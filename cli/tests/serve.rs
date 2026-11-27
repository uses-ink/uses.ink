//! `ink serve` serves what `ink build` wrote, through the edge: pages with the manifest's
//! headers, conditional and range requests, the site's 404, hostile paths refused, a folder of
//! deploys by host.

#![allow(clippy::unwrap_used, clippy::too_many_lines, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use clap::Parser;

/// Stdout sent to the test while `ink serve` runs in its own thread.
struct Sent(std::sync::mpsc::Sender<Vec<u8>>);

impl Write for Sent {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.send(bytes.to_vec()).map_err(|_| std::io::ErrorKind::BrokenPipe)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&str]) -> (cli::Exit, String, String) {
    let cli = cli::Cli::try_parse_from(std::iter::once("ink").chain(args.iter().copied())).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = cli.run(&mut out, &mut err);
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// A scratch folder for `name`, empty.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-serve").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Builds a two-page site into `<root>/out` and returns `out`.
fn build(root: &Path) -> PathBuf {
    let site = root.join("site");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(site.join("index.md"), "# Home\n").unwrap();
    std::fs::write(site.join("tea.md"), "# Tea\n\nA pot of tea.\n").unwrap();
    let out = root.join("out");
    let (exit, _, err) = ink(&["build", "-C", site.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(exit, cli::Exit::Done, "{err}");
    out
}

/// Copies the folder `from` into `to`, whole.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Runs `ink serve <args>` in a thread and returns the site it serves.
fn serve(args: &[&str]) -> Site {
    let mut full = vec!["ink", "serve"];
    full.extend(args);
    full.extend(["--addr", "127.0.0.1:0"]);
    let cli = cli::Cli::try_parse_from(full).unwrap();
    let (sender, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || cli.run(&mut Sent(sender), &mut std::io::sink()));
    // the receiver stays alive with the site: `ink serve` prints more after the address, and a
    // closed stdout fails that write, ends the run and drops the listener under the test
    let mut printed = String::new();
    while !printed.ends_with('\n') {
        printed
            .push_str(std::str::from_utf8(&received.recv_timeout(std::time::Duration::from_secs(5)).unwrap()).unwrap());
    }
    Site { port: printed.trim().strip_prefix("http://127.0.0.1:").unwrap().parse().unwrap(), _stdout: received }
}

/// One response.
struct Got {
    text: String,
}

impl Got {
    fn status(&self) -> &str {
        self.text.split(' ').nth(1).unwrap_or_default()
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.text.lines().take_while(|line| !line.is_empty()).find_map(|line| {
            let (field, value) = line.split_once(": ")?;
            field.eq_ignore_ascii_case(name).then_some(value)
        })
    }

    fn body(&self) -> &str {
        self.text.split_once("\r\n\r\n").map_or("", |(_, body)| body)
    }
}

/// A running `ink serve`, by its port, and the stdout it keeps writing to.
struct Site {
    port: u16,
    _stdout: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl Site {
    /// Sends `method target` with `Host: host` and these headers, and reads to the end.
    fn fetch(&self, (method, target): (&str, &str), host: &str, headers: &[&str]) -> Got {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let extra: String = headers.iter().map(|line| [*line, "\r\n"].concat()).collect();
        write!(stream, "{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{extra}\r\n").unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        Got { text: String::from_utf8_lossy(&bytes).into_owned() }
    }

    fn get(&self, target: &str, headers: &[&str]) -> Got {
        self.fetch(("GET", target), "localhost", headers)
    }
}

/// Stdout that fails every write, as a receiver already dropped.
struct Shut;

impl Write for Shut {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A stdout that cannot be written ends the run before it serves: the address is written once the
/// listener is bound, and when that write fails the run ends and the listener closes, so no run
/// serves with nobody told where.
#[test]
fn a_closed_stdout_ends_the_run_and_closes_the_listener() {
    let root = scratch("shut");
    let out = build(&root);
    let cli = cli::Cli::try_parse_from(["ink", "serve", out.to_str().unwrap(), "--addr", "127.0.0.1:0"]).unwrap();
    let exit = std::thread::spawn(move || cli.run(&mut Shut, &mut std::io::sink())).join().unwrap();
    assert_ne!(exit, cli::Exit::Done, "the failed write ends the run");
}

#[test]
fn serves_a_built_deploy_with_the_edge_behaviour() {
    let root = scratch("deploy");
    let out = build(&root);
    let site = serve(&[out.to_str().unwrap()]);

    let tea = site.get("/tea", &[]);
    assert_eq!(tea.status(), "200", "{}", tea.text);
    assert_eq!(tea.header("content-type"), Some("text/html; charset=utf-8"));
    assert!(tea.header("content-security-policy").unwrap().starts_with("default-src 'none'; img-src"));
    assert_eq!(tea.header("x-content-type-options"), Some("nosniff"));
    assert!(tea.body().contains("A pot of tea."));
    let etag = tea.header("etag").unwrap();
    assert_eq!(site.get("/tea", &[&format!("If-None-Match: {etag}")]).status(), "304");
}

#[test]
fn answers_conditions_and_ranges_like_the_engine_headers_say() {
    let root = scratch("ranges");
    let site = serve(&[build(&root).to_str().unwrap()]);
    let css = site.get("/style.css", &[]);
    assert_eq!(css.status(), "200");
    let length: usize = css.header("content-length").unwrap().parse().unwrap();
    let ranged = site.get("/style.css", &["Range: bytes=0-9"]);
    assert_eq!(ranged.status(), "206");
    assert_eq!(ranged.header("content-range"), Some(format!("bytes 0-9/{length}").as_str()));
    assert_eq!(ranged.body(), &css.body()[..10]);
    assert_eq!(site.get("/style.css", &[&format!("Range: bytes={length}-")]).status(), "416");
    assert_eq!(
        site.fetch(("HEAD", "/style.css"), "localhost", &[]).header("content-length"),
        css.header("content-length")
    );
}

#[test]
fn refuses_hostile_requests_and_serves_the_sites_404() {
    let root = scratch("hostile");
    let site = serve(&[build(&root).to_str().unwrap()]);
    let tea = site.get("/tea", &[]);
    let missing = site.get("/no/such/page", &[]);
    assert_eq!(missing.status(), "404");
    assert!(missing.body().contains("Page not found"), "{}", missing.text);
    assert_eq!(missing.header("content-security-policy"), tea.header("content-security-policy"));

    for target in [
        "/../etc/passwd",
        "/%2e%2e/%2e%2e/etc/passwd",
        "/..%2f..%2fetc/passwd",
        "//evil.example/",
        "/%252e%252e/",
        "/a%00b",
    ] {
        let refused = site.get(target, &[]);
        assert_eq!(refused.status(), "400", "{target}: {}", refused.text);
        assert!(!refused.body().contains("root:"));
    }
    for target in ["/.ink-manifest.json", "/%2Eink-manifest.json", "/.ink", "/tea.html"] {
        let hidden = site.get(target, &[]);
        assert_eq!(hidden.status(), "404", "{target}");
        assert!(!hidden.body().contains("uses.manifest"), "the manifest is never served: {target}");
    }
    assert_eq!(site.fetch(("POST", "/"), "localhost", &["Content-Length: 0"]).status(), "405");
    assert_eq!(site.fetch(("GET", "/"), "evil.example/x", &[]).status(), "400");
}

#[test]
fn the_edges_own_headers_match_what_the_engine_writes() {
    let root = scratch("policy");
    let out = build(&root);
    let site = serve(&[out.to_str().unwrap()]);
    let manifest = site.get("/style.css", &[]);
    let refusal = site.fetch(("POST", "/"), "localhost", &["Content-Length: 0"]);
    for name in ["content-security-policy", "cross-origin-resource-policy", "x-content-type-options"] {
        assert!(manifest.header(name).is_some(), "{name}");
        assert_eq!(
            refusal.header(name),
            manifest.header(name),
            "{name}: the edge's copy drifted from the engine's Policy"
        );
    }
}

#[test]
fn serves_a_folder_of_deploys_by_host() {
    let root = scratch("hosts");
    let out = build(&root);
    let hosts = root.join("hosts");
    let one = hosts.join("notes.example");
    std::fs::create_dir_all(&one).unwrap();
    copy(&out, &one);
    let site = serve(&[hosts.to_str().unwrap(), "--host-mode", "folders"]);
    assert_eq!(site.fetch(("GET", "/tea"), "notes.example", &[]).status(), "200");
    assert_eq!(site.fetch(("GET", "/tea"), "NOTES.example:8080", &[]).status(), "200");
    assert_eq!(site.fetch(("GET", "/tea"), "other.example", &[]).status(), "404");
    assert_eq!(site.fetch(("GET", "/tea"), "..", &[]).status(), "400");
    assert_eq!(site.fetch(("GET", "/tea"), "../notes.example", &[]).status(), "400");
    assert_eq!(site.fetch(("GET", "/healthz"), "10.1.2.3", &[]).status(), "200");
}

#[test]
fn a_folder_that_is_no_deploy_is_refused() {
    let root = scratch("empty");
    let (exit, _, err) = ink(&["serve", root.to_str().unwrap(), "--host-mode", "single", "--addr", "127.0.0.1:0"]);
    assert_eq!(exit, cli::Exit::Usage);
    assert!(err.contains("is not a deploy `ink serve` can serve"), "{err}");
    let missing = root.join("nothing-here");
    let (exit, _, err) = ink(&["serve", missing.to_str().unwrap(), "--addr", "127.0.0.1:0"]);
    assert_eq!(exit, cli::Exit::Usage, "a folder that does not exist serves nothing, said aloud: {err}");
    assert!(err.contains("is not a deploy `ink serve` can serve"), "{err}");
}

/// Every file under `root`, as a tar archive of paths relative to it.
fn archive(root: &Path) -> Vec<u8> {
    fn walk(dir: &Path, root: &Path, writer: &mut edge_api::tar::Writer) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, writer);
            } else {
                let name = path.strip_prefix(root).unwrap().to_str().unwrap();
                writer.file(name, &std::fs::read(&path).unwrap()).unwrap();
            }
        }
    }
    let mut writer = edge_api::tar::Writer::new(1 << 30);
    walk(root, root, &mut writer);
    writer.finish()
}

impl Site {
    /// An API call with a bearer token and a body, as raw bytes on the wire.
    fn call(&self, (method, target): (&str, &str), token: &str, body: &[u8]) -> Got {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let head = format!(
            "{method} {target} HTTP/1.1\r\nHost: api.localhost:8080\r\nConnection: close\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        Got { text: String::from_utf8_lossy(&bytes).into_owned() }
    }
}

#[test]
fn the_api_takes_a_site_and_a_built_folder_when_given_a_data_folder() {
    let root = scratch("api");
    let out = build(&root);
    let data = root.join("data");
    let cli = cli::Cli::try_parse_from([
        "ink",
        "serve",
        out.to_str().unwrap(),
        "--data",
        data.to_str().unwrap(),
        "--addr",
        "127.0.0.1:0",
    ])
    .unwrap();
    let (sender, received) = std::sync::mpsc::channel();
    let (said, heard) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || cli.run(&mut Sent(sender), &mut Sent(said)));
    let mut printed = String::new();
    while !printed.ends_with('\n') {
        let part = received.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        printed.push_str(std::str::from_utf8(&part).unwrap());
    }
    // the API is up once its last setting line is written (to stderr): the owner token file is
    // written before
    let mut settings = String::new();
    while !settings.contains("backing off") {
        let part = heard.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        settings.push_str(std::str::from_utf8(&part).unwrap());
    }
    let first = printed.lines().next().unwrap();
    let site = Site { port: first.strip_prefix("http://127.0.0.1:").unwrap().parse().unwrap(), _stdout: received };
    let file = data.join("owner-token");
    assert!(!handle.is_finished(), "ink serve ended before the API was used");
    let token = std::fs::read_to_string(&file).unwrap();
    let denied = site.call(("GET", "/v1/sites"), "wrong", b"");
    assert_eq!(denied.status(), "401", "{}", denied.text);
    assert_eq!(denied.header("www-authenticate"), Some("Bearer"));
    let added = site.call(("POST", "/v1/sites"), &token, br#"{"handle":"diary","source":{"kind":"upload"}}"#);
    assert_eq!(added.status(), "201", "{}", added.text);
    let id = added.body().split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap().to_owned();
    let put = site.call(("PUT", &format!("/v1/sites/{id}/deploy")), &token, &archive(&out));
    assert_eq!(put.status(), "201", "{}", put.text);
    assert!(put.body().contains("\"published\":\"live\""), "{}", put.text);
    let exported = site.call(("GET", &format!("/v1/sites/{id}/export")), &token, b"");
    assert_eq!((exported.status(), exported.header("content-type")), ("200", Some("application/x-tar")));
    // the folder is still served on every other host
    assert_eq!(site.get("/tea", &[]).status(), "200");
}
