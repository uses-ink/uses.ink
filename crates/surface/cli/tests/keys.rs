//! A machine's key from login to refusal through the real binary: `ink serve --data`, `ink login
//! --data`, a signed `ink deploy list`, a replayed request refused, and a revoked key refused.

#![allow(clippy::unwrap_used, clippy::panic, clippy::too_many_lines, reason = "tests stop at the first surprise")]

mod guard;
mod keyed;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// `ink <args>` with a config folder of its own and no key in the environment: exit code, stdout
/// and stderr.
fn ink(home: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ink"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// One raw request to the API host, read to the end.
fn raw(port: u16, (method, target): (&str, &str), authorization: &str, body: &[u8]) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let head = format!(
        "{method} {target} HTTP/1.1\r\nHost: api.localhost:{port}\r\nConnection: close\r\nAuthorization: {authorization}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn a_key_made_by_login_deploys_is_not_replayable_and_ends_when_removed() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-keys");
    std::fs::remove_dir_all(&root).ok();
    let (data, home, site) = (root.join("data"), root.join("home"), root.join("site"));
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(site.join("index.html"), "hi").unwrap();
    // the audience names the port, so the port is chosen first; a port that another process took in
    // between ends the server at once, and another one is tried
    let (_running, port) = (0..5)
        .find_map(|_| {
            let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
            let (addr, audience) = (format!("127.0.0.1:{port}"), format!("api.localhost:{port}"));
            let mut running = guard::Guard::spawn(
                Command::new(env!("CARGO_BIN_EXE_ink"))
                    .args(["serve", site.to_str().unwrap(), "--addr", &addr, "--data", data.to_str().unwrap()])
                    .args(["--audience", &audience, "--auth-skew", "5"]),
            );
            let received = running.lines();
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                // the lines end when the child does, which is a start that did not happen
                let line = received.recv_timeout(deadline.saturating_duration_since(Instant::now())).ok()?;
                if line.contains("backing off") {
                    return Some((running, port));
                }
            }
        })
        .unwrap_or_else(|| panic!("the server ended or was silent five times before it started"));
    let (audience, api) = (format!("api.localhost:{port}"), format!("http://api.localhost:{port}"));
    assert!(data.join("operator-keys").exists(), "serve made no operator-keys file");
    // a site to list deploys of, made by a second operator key
    let helper = keyed::Keyed::operator(&data, &audience);
    let barrier = Instant::now() + Duration::from_secs(30);
    let mut restarting = false;
    let made = loop {
        let body = br#"{"handle":"diary","source":{"kind":"upload"}}"#;
        let got = raw(port, ("POST", "/v1/sites"), &helper.sign("POST", "/v1/sites", body), body);
        if !got.contains("auth.server.restarting") {
            break got;
        }
        // a signed request before the boot barrier ends is told so, by its code
        restarting = true;
        assert!(Instant::now() < barrier, "the boot barrier did not end: {got}");
        std::thread::sleep(Duration::from_millis(250));
    };
    assert!(
        restarting,
        "a signed request right after the start is turned away until the barrier ends (skew 5, so 11 s)"
    );
    assert!(made.starts_with("HTTP/1.1 201"), "{made}");
    let id = made.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap().to_owned();

    // login with the data folder, no browser
    let (code, out, err) = ink(&home, &["login", "--data", data.to_str().unwrap(), "--api", &api, "--label", "e2e"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("enrolled"), "{out}");
    assert!(!out.contains("ink-ed25519-1:") && !err.contains("ink-ed25519-1:"), "a private key was printed");
    let operators = std::fs::read_to_string(data.join("operator-keys")).unwrap();
    assert!(operators.lines().any(|line| line.ends_with(" e2e")), "{operators}");

    // a signed call, through the CLI
    let list = ["deploy", "list", "--api", &api, "--site", &id];
    let mut listed = ink(&home, &list);
    for _ in 0..20 {
        if !listed.2.contains("auth.server.restarting") {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        listed = ink(&home, &list);
    }
    assert_eq!(listed.0, 0, "{}{}", listed.1, listed.2);
    let (code, who, err) = ink(&home, &["whoami", "--api", &api]);
    assert_eq!(code, 0, "{who}{err}");

    // a key file that the group or the world can read is refused, and works again at 0600
    let key = std::fs::read_dir(home.join("config").join("ink").join("keys"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "key"))
        .unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    let (code, out, err) = ink(&home, &["whoami", "--api", &api]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("chmod 600"), "{err}");
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let (code, out, err) = ink(&home, &["whoami", "--api", &api]);
    assert_eq!(code, 0, "{out}{err}");

    // a replayed request is refused: the same header twice
    let target = "/v1/whoami";
    let header = helper.sign("GET", target, b"");
    let first = raw(port, ("GET", target), &header, b"");
    assert!(first.starts_with("HTTP/1.1 200"), "{first}");
    let again = raw(port, ("GET", target), &header, b"");
    assert!(again.starts_with("HTTP/1.1 401") && again.contains("auth.signature.replayed"), "{again}");

    // a key whose line is removed is refused
    let kept = operators.lines().filter(|line| !line.ends_with(" e2e")).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::write(data.join("operator-keys"), kept).unwrap();
    let mut refused = ink(&home, &["whoami", "--api", &api]);
    for _ in 0..20 {
        if refused.0 == 3 {
            break;
        }
        // the file is rechecked by its modification time, which may not have moved yet
        std::thread::sleep(Duration::from_millis(250));
        refused = ink(&home, &["whoami", "--api", &api]);
    }
    assert_eq!(refused.0, 3, "{}{}", refused.1, refused.2);
}
