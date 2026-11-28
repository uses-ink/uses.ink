//! The product Worker end to end, in workerd (decision 0201): the real `ink` signs the calls
//! that make a site, a built folder is put through intake, and the serve path of the same
//! Worker answers for it. A replayed request is refused, a suspended site answers 403, and what
//! the Worker does not have yet answers 501 with its code.
//!
//! It needs wrangler, workerd and a built Worker, so it is ignored until `cargo xtask worker app`
//! runs it with `HOSTED_APP` (the Worker's folder), `HOSTED_STATE` (where its local D1 is, with the
//! migrations applied) and `HOSTED_PORT`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

mod keyed;

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use edge_api::tar::Writer;
use serde_json::{Value, json};
use sig::{Seed, Signer};

/// What the test is told by `cargo xtask worker app`.
struct Told {
    app: PathBuf,
    state: PathBuf,
    port: u16,
}

impl Told {
    fn read() -> Self {
        let var = |name: &str| {
            std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set: run `cargo xtask worker app`"))
        };
        Self {
            app: var("HOSTED_APP").into(),
            state: var("HOSTED_STATE").into(),
            port: var("HOSTED_PORT").parse().unwrap(),
        }
    }
}

/// `wrangler dev --local` with the Worker, stopped with its whole process group when dropped.
struct Dev {
    child: Child,
    log: PathBuf,
}

impl Dev {
    /// Starts the Worker with `operators` as the operator's keys, and waits until it listens.
    fn start(told: &Told, operators: &str, log: &Path) -> Self {
        let port = told.port;
        let file = std::fs::File::create(log).unwrap();
        let vars = [
            format!("ORIGIN:http://localhost:{port}"),
            format!("API_HOST:api.localhost:{port}"),
            format!("OPERATOR_KEYS:{operators}"),
            "AUTH_SKEW:5".to_owned(),
            "LOOKUP_TTL:0".to_owned(),
            // a key for this run only: nothing sealed under it is worth keeping
            "SEALING_KEY:6a09e667f3bcc908b2fb1366ea957d3e3adec17512775099da2f59b4e9e8b3a5".to_owned(),
        ];
        let mut command = Command::new("wrangler");
        command
            .current_dir(&told.app)
            .args(["dev", "--local", "--ip", "127.0.0.1", "--port"])
            .arg(port.to_string())
            .arg("--inspector-port")
            .arg((u32::from(port) + 1000).to_string())
            .arg("--persist-to")
            .arg(&told.state)
            .env("CI", "1")
            .env("WRANGLER_SEND_METRICS", "false")
            .stdin(Stdio::null())
            .stdout(file.try_clone().unwrap())
            .stderr(file)
            .process_group(0);
        for var in &vars {
            command.arg("--var").arg(var);
        }
        let child = command.spawn().unwrap();
        let dev = Self { child, log: log.to_path_buf() };
        let started = Instant::now();
        while !dev.said().contains("Ready on") {
            assert!(started.elapsed() < Duration::from_secs(180), "wrangler did not start:\n{}", dev.said());
            std::thread::sleep(Duration::from_millis(200));
        }
        dev
    }

    fn said(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

impl Drop for Dev {
    fn drop(&mut self) {
        let group = format!("-{}", self.child.id());
        drop(Command::new("kill").args(["-TERM", "--", &group]).status());
        for _ in 0..50 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        drop(Command::new("kill").args(["-KILL", "--", &group]).status());
        drop(self.child.wait());
    }
}

/// What a raw HTTP call got back.
struct Got {
    status: u16,
    head: String,
    body: Vec<u8>,
    millis: u128,
}

impl Got {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&self.body)))
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.head
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(found, _)| found.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim())
    }
}

/// A raw HTTP/1.1 client: the Worker is reached by address and named by `Host`.
struct Wire {
    port: u16,
}

impl Wire {
    /// One call, answered in full.
    fn send(&self, host: &str, (method, target): (&str, &str), (extra, body): (&[(&str, &str)], &[u8])) -> Got {
        let started = Instant::now();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let mut head = format!(
            "{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in extra {
            write!(head, "{name}: {value}\r\n").unwrap();
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let split =
            bytes.windows(4).position(|window| window == b"\r\n\r\n").unwrap_or_else(|| panic!("no head in {bytes:?}"));
        let head = String::from_utf8_lossy(&bytes[..split]).into_owned();
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        let mut body = bytes[split + 4..].to_vec();
        if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
            body = Self::unchunk(&body);
        }
        Got { status, head, body, millis: started.elapsed().as_millis() }
    }

    /// The body of a chunked answer.
    fn unchunk(mut rest: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let line_end = rest.windows(2).position(|window| window == b"\r\n").unwrap();
            let size = usize::from_str_radix(
                std::str::from_utf8(&rest[..line_end]).unwrap().split(';').next().unwrap().trim(),
                16,
            )
            .unwrap();
            rest = &rest[line_end + 2..];
            if size == 0 {
                return out;
            }
            out.extend_from_slice(&rest[..size]);
            rest = &rest[size + 2..];
        }
    }
}

/// What a run of `ink` said.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

/// The tar of every file below `dir`, named by its path from `dir`.
fn tarred(dir: &Path) -> Vec<u8> {
    fn walk(dir: &Path, below: &Path, writer: &mut Writer) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, below, writer);
            } else {
                let name = path.strip_prefix(below).unwrap().to_string_lossy().replace('\\', "/");
                writer.file(&name, &std::fs::read(&path).unwrap()).unwrap();
            }
        }
    }
    let mut writer = Writer::new(64 << 20);
    walk(dir, dir, &mut writer);
    writer.finish()
}

#[test]
#[ignore = "needs wrangler and workerd: run it with `cargo xtask worker app`"]
fn the_product_worker_makes_a_site_serves_it_refuses_a_replay_and_suspends_it() {
    let told = Told::read();
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-hosted");
    std::fs::remove_dir_all(&root).ok();
    let (data, home, site) = (root.join("data"), root.join("home"), root.join("site"));
    for dir in [&data, &home, &site] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let port = told.port;
    let (audience, api) = (format!("api.localhost:{port}"), format!("http://api.localhost:{port}"));
    let once = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_ink"))
            .arg("-C")
            .arg(&site)
            .args(args)
            .env_clear()
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("INK_API", &api)
            .env("INK_TIMEOUT", "30")
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .unwrap();
        Ran {
            code: out.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    };
    // a nonce object that was just built refuses a key for one window, as the boot rule says
    let ink = |args: &[&str]| {
        let mut ran = once(args);
        for _ in 0..40 {
            if !ran.err.contains("auth.server.restarting") && !ran.out.contains("auth.server.restarting") {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
            ran = once(args);
        }
        ran
    };

    // the operator's keys: the key `ink` keeps, and one this test signs with by hand
    let login = ink(&["login", "--data", data.to_str().unwrap(), "--label", "e2e"]);
    assert_eq!(login.code, 0, "{}{}", login.out, login.err);
    let hand = keyed::Keyed::operator(&data, &audience);
    let operators = std::fs::read_to_string(data.join("operator-keys")).unwrap();
    let dev = Dev::start(&told, operators.trim(), &root.join("worker.log"));
    let wire = Wire { port };
    let sign = |method: &str, target: &str, body: &[u8]| hand.sign(method, target, body);
    // the same wait for the hand-signed key, with a fresh signature each time
    let signed = |route: (&str, &str), body: &[u8]| {
        let mut got = wire.send(&audience, route, (&[("Authorization", &sign(route.0, route.1, body))], body));
        for _ in 0..40 {
            if got.status != 503 || !got.text().contains("auth.server.restarting") {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
            got = wire.send(&audience, route, (&[("Authorization", &sign(route.0, route.1, body))], body));
        }
        got
    };
    let mut said = Vec::new();

    // nothing is taken unsigned, and what the Worker lacks says so
    let unsigned = wire.send(&audience, ("GET", "/v1/sites"), (&[], b""));
    assert_eq!(unsigned.status, 400, "{}\n{}", unsigned.text(), dev.said());
    assert_eq!(unsigned.json()["problem"], "auth.signature.malformed");
    let pending = wire.send(&audience, ("GET", "/"), (&[], b""));
    assert_eq!(
        (pending.status, pending.json()["problem"].clone()),
        (501, json!("worker.route.pending")),
        "{}",
        pending.text()
    );
    let session = wire.send(&audience, ("POST", "/v1/passkeys/start"), (&[], b"{}"));
    assert_eq!(session.status, 501, "{}", session.text());
    said.push(format!("e2e not-yet route answered 501 in {} ms", pending.millis));

    // a key asks to be enrolled: it waits for a person, who cannot sign in on the Worker yet
    let device = Signer::from_seed(&Seed::new([42; 32]));
    let ask = json!({ "key": device.public().to_string(), "label": "phone", "purpose": "device" }).to_string();
    let asked = wire.send(&audience, ("POST", "/v1/enrol"), (&[], ask.as_bytes()));
    assert_eq!(asked.status, 201, "{}\n{}", asked.text(), dev.said());
    assert!(asked.json()["user_code"].as_str().is_some_and(|code| code.len() == 9), "{}", asked.text());
    said.push(format!("e2e enrol answered 201 in {} ms", asked.millis));

    // the real ink makes the site, signing as the operator
    let init = ink(&["init", "--handle", "diary", "--json"]);
    assert_eq!(init.code, 0, "{}{}\n{}", init.out, init.err, dev.said());
    let linked: Value = serde_json::from_str(init.out.lines().last().unwrap()).unwrap();
    let id = linked["site"].as_str().unwrap().to_owned();

    // the real ink builds the folder, and the archive of it is the intake
    std::fs::write(site.join("index.md"), "# Hosted on a Worker\n").unwrap();
    std::fs::write(site.join("about.md"), "# About\n\nA second page.\n").unwrap();
    let out = site.join("out");
    let built = ink(&["build", "-o", out.to_str().unwrap()]);
    assert_eq!(built.code, 0, "{}{}", built.out, built.err);
    let archive = tarred(&out);
    let path = format!("/v1/sites/{id}/deploy");
    let put = signed(("PUT", &path), &archive);
    assert_eq!(put.status, 201, "{}\n{}", put.text(), dev.said());
    assert_eq!(put.json()["published"], "live", "{}", put.text());
    said.push(format!("e2e intake of {} bytes answered 201 in {} ms", archive.len(), put.millis));

    // the serve path of the same Worker answers for it
    let host = format!("diary.localhost:{port}");
    let page = wire.send(&host, ("GET", "/"), (&[], b""));
    assert_eq!(page.status, 200, "{}\n{}", page.text(), dev.said());
    assert!(page.text().contains("Hosted on a Worker"), "{}", page.text());
    let again = wire.send(&host, ("GET", "/"), (&[], b""));
    said.push(format!("e2e page served in {} ms, then {} ms", page.millis, again.millis));
    let about = wire.send(&host, ("GET", "/about"), (&[], b""));
    assert_eq!(about.status, 200, "{}", about.text());
    let ranged = wire.send(&host, ("GET", "/"), (&[("Range", "bytes=0-3")], b""));
    assert_eq!(ranged.status, 206, "{}", ranged.text());
    assert_eq!(ranged.body.len(), 4);
    assert!(ranged.header("content-range").is_some_and(|value| value.starts_with("bytes 0-3/")), "{}", ranged.head);
    assert_eq!(wire.send(&host, ("GET", "/nothing-here"), (&[], b"")).status, 404);
    assert_eq!(wire.send(&format!("nobody.localhost:{port}"), ("GET", "/"), (&[], b"")).status, 404);

    // the listing is signed, and the same signature twice is taken once
    let listed = sign("GET", "/v1/sites", b"");
    let first = wire.send(&audience, ("GET", "/v1/sites"), (&[("Authorization", &listed)], b""));
    assert_eq!(first.status, 200, "{}\n{}", first.text(), dev.said());
    assert!(first.text().contains("diary"), "{}", first.text());
    let replay = wire.send(&audience, ("GET", "/v1/sites"), (&[("Authorization", &listed)], b""));
    assert_ne!(replay.status, 200, "a replayed signature was taken: {}", replay.text());
    assert!(replay.status == 401 || replay.status == 409, "{} {}", replay.status, replay.text());
    said.push(format!(
        "e2e signed call {} ms, replay refused with {} in {} ms",
        first.millis, replay.status, replay.millis
    ));

    // a suspended site answers 403 with none of its bytes, and serves again once it is reinstated
    let suspend = format!("/v1/sites/{id}/suspend");
    let body = br#"{"kind":"spam"}"#;
    let done = signed(("POST", &suspend), body);
    assert_eq!(done.status, 200, "{}", done.text());
    let held = wire.send(&host, ("GET", "/"), (&[], b""));
    assert_eq!(held.status, 403, "{}", held.text());
    assert!(!held.text().contains("Hosted on a Worker"), "a suspended site served its bytes");
    let reinstate = format!("/v1/sites/{id}/reinstate");
    let back = signed(("POST", &reinstate), b"");
    assert_eq!(back.status, 200, "{}", back.text());
    assert_eq!(wire.send(&host, ("GET", "/"), (&[], b"")).status, 200);

    for line in &said {
        writeln!(std::io::stdout(), "{line}").unwrap();
    }
    // `cargo xtask worker app` reads the numbers from here and puts them in its report
    if let Ok(path) = std::env::var("HOSTED_REPORT") {
        std::fs::write(path, said.join("\n")).unwrap();
    }
}
