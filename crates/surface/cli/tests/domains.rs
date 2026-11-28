//! Custom domains through the real binary (decision 0093): `ink domain add|list|prove|remove`
//! and the Settings forms, against a server that asks a real nameserver (a UDP socket of the
//! test) for the proof record, and against one with no resolver.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

mod rig;

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rig::{Browser, Got, Machine, Server};

/// A nameserver on a UDP socket that answers every TXT question with the one record it is given.
struct Zone {
    record: Arc<Mutex<Option<String>>>,
    address: String,
}

impl Zone {
    fn start() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap().to_string();
        let record: Arc<Mutex<Option<String>>> = Arc::default();
        let shared = Arc::clone(&record);
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 512];
            while let Ok((length, from)) = socket.recv_from(&mut buffer) {
                let asked = &buffer[..length];
                let end = 12 + asked[12..].iter().position(|byte| *byte == 0).unwrap() + 5;
                let said = shared.lock().unwrap().clone();
                let mut answer = vec![asked[0], asked[1], 0x81, 0x80, 0, 1, 0, u8::from(said.is_some()), 0, 0, 0, 0];
                answer.extend_from_slice(&asked[12..end]);
                if let Some(text) = said {
                    answer.extend_from_slice(&[0xc0, 12, 0, 16, 0, 1, 0, 0, 0, 60]);
                    answer.extend_from_slice(&u16::try_from(text.len() + 1).unwrap().to_be_bytes());
                    answer.push(u8::try_from(text.len()).unwrap());
                    answer.extend_from_slice(text.as_bytes());
                }
                socket.send_to(&answer, from).ok();
            }
        });
        Self { record, address }
    }

    /// Publishes `value` as the TXT record of every name.
    fn publish(&self, value: &str) {
        *self.record.lock().unwrap() = Some(value.to_owned());
    }
}

/// A form body of one field.
fn field(name: &str, value: &str) -> String {
    format!("{name}={value}")
}

/// A page of the server asked for under another host, as a browser follows a custom domain.
fn under(server: &Server, host: &str) -> Got {
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(stream, "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).ok();
    Got { text: String::from_utf8_lossy(&bytes).into_owned() }
}

/// A machine holding the operator's key, in a folder that is pushed as the site `handle`.
fn publisher<'a>(server: &'a Server, handle: &str) -> Machine<'a> {
    let mut machine = server.machine("publisher");
    machine.dir = server.root.join("site");
    std::fs::create_dir_all(&machine.dir).unwrap();
    let logged = machine.ink(&["login", "--data", server.data.to_str().unwrap(), "--label", "operator"]);
    assert_eq!(logged.code, 0, "{}", logged.said());
    machine.write("index.md", "# Garden\n\nHello.\n");
    let init = machine.ink(&["init", "--handle", handle, "--json"]);
    assert_eq!(init.code, 0, "{}", init.said());
    let pushed = machine.ink(&["push", "--wait", "30"]);
    assert_eq!(pushed.code, 0, "{}", pushed.said());
    machine
}

/// Signs the first-run link in: the owner of the server's one account.
fn owner(server: &Server) -> Browser<'_> {
    let link = server.links().pop().unwrap();
    let path = link[link.find("/login/").unwrap()..].to_owned();
    let mut browser = server.browser();
    let entered = browser.send(("POST", &path), "");
    assert_eq!(entered.status(), "303", "{}", entered.text);
    browser.keep(&entered);
    browser
}

#[test]
fn a_site_proves_a_name_through_a_nameserver_and_the_name_serves_it() {
    let zone = Zone::start();
    let server = Server::start("domains-cli", &["--site-ttl", "0", "--resolver-server", &zone.address]);
    let machine = publisher(&server, "garden");

    let added = machine.ink(&["domain", "add", "www.example.org", "--json"]);
    assert_eq!(added.code, 0, "{}", added.said());
    let (host, value) = (
        added.json()["record"]["host"].as_str().unwrap().to_owned(),
        added.json()["record"]["value"].as_str().unwrap().to_owned(),
    );
    assert_eq!(host, "_uses-ink.www.example.org");
    assert!(value.starts_with("uses-ink-domain="), "{value}");
    let plain = machine.ink(&["domain", "list"]);
    assert!(plain.out.contains("TXT") && plain.out.contains("waiting for proof"), "{}", plain.said());
    assert_eq!(under(&server, "www.example.org").status(), "404", "a claim serves nothing");

    // no record yet: the name is refused, with what to do
    let early = machine.ink(&["domain", "prove", "www.example.org"]);
    assert_ne!(early.code, 0, "{}", early.said());
    assert!(early.err.contains("publish the TXT record"), "{}", early.err);

    // a record that carries another token proves nothing
    zone.publish("uses-ink-domain=00000000000000000000000000000000");
    assert_ne!(machine.ink(&["domain", "prove", "www.example.org"]).code, 0);

    zone.publish(&value);
    let proved = machine.ink(&["domain", "prove", "www.example.org", "--json"]);
    assert_eq!(proved.code, 0, "{}", proved.said());
    assert_eq!(proved.json()["state"], "proven");
    let page = under(&server, "www.example.org");
    assert_eq!(page.status(), "200", "the proved name serves the site: {}", page.text);
    assert!(page.body().contains("Hello."), "{}", page.text);

    let listed = machine.ink(&["domain", "list", "--json"]);
    assert_eq!(listed.code, 0, "{}", listed.said());
    assert_eq!(listed.json()["domains"][0]["name"], "www.example.org");
    assert_eq!(listed.json()["domains"][0]["state"], "proven");
    assert!(machine.ink(&["domain", "list"]).out.contains("serving"));

    let removed = machine.ink(&["domain", "remove", "www.example.org"]);
    assert_eq!(removed.code, 0, "{}", removed.said());
    assert_eq!(under(&server, "www.example.org").status(), "404", "a removed name serves nothing");
    assert!(machine.ink(&["domain", "list"]).out.contains("no custom domains"));
    let again = machine.ink(&["domain", "remove", "www.example.org"]);
    assert_ne!(again.code, 0, "{}", again.said());

    // what is not a name is refused before any call
    let bad = machine.ink(&["domain", "add", "not a name"]);
    assert_eq!(bad.code, 2, "{}", bad.said());
}

#[test]
fn the_settings_screen_claims_proves_and_removes_a_name() {
    let zone = Zone::start();
    let server = Server::start("domains-web", &["--site-ttl", "0", "--resolver-server", &zone.address]);
    let browser = owner(&server);
    let _machine = publisher(&server, "diary");

    let empty = browser.send(("GET", "/diary/settings"), "");
    assert_eq!(empty.status(), "200", "{}", empty.text);
    assert!(empty.body().contains("action=\"/diary/claim\""), "the claim form: {}", empty.body());

    let bad = browser.send(("POST", "/diary/claim"), &field("name", "not+a+name"));
    assert_eq!(bad.status(), "400", "{}", bad.text);
    assert!(bad.body().contains("form.field.invalid"), "{}", bad.body());
    let foreign = browser.send_with(
        ("POST", "/diary/claim"),
        &[("Origin", "http://evil.example")],
        &field("name", "a.example.org"),
    );
    assert_eq!(foreign.status(), "403", "a post from another origin changes nothing: {}", foreign.text);

    let claimed = browser.send(("POST", "/diary/claim"), &field("name", "www.example.org"));
    assert_eq!(claimed.status(), "303", "{}", claimed.text);
    assert_eq!(claimed.header("location"), Some("/diary/settings"));
    let pending = browser.send(("GET", "/diary/settings"), "");
    let body = pending.body();
    assert!(body.contains("_uses-ink.www.example.org"), "the record's host: {body}");
    assert!(body.contains("uses-ink-domain="), "the record's value: {body}");
    assert!(body.contains("action=\"/diary/prove\""), "{body}");
    let value = body[body.find("uses-ink-domain=").unwrap()..].split('<').next().unwrap().to_owned();

    // no record yet: the page says so, and the claim stays
    let early = browser.send(("POST", "/diary/prove"), &field("name", "www.example.org"));
    assert_eq!(early.status(), "409", "{}", early.text);
    assert!(early.body().contains("Fix"), "{}", early.body());

    zone.publish(&value);
    let proved = browser.send(("POST", "/diary/prove"), &field("name", "www.example.org"));
    assert_eq!(proved.status(), "303", "{}{}", proved.text, server.said());
    let serving = browser.send(("GET", "/diary/settings"), "");
    assert!(serving.body().contains("Serves this site since"), "{}", serving.body());
    assert_eq!(under(&server, "www.example.org").status(), "200");

    let removed = browser.send(("POST", "/diary/unclaim"), &field("name", "www.example.org"));
    assert_eq!(removed.status(), "303", "{}", removed.text);
    let after = browser.send(("GET", "/diary/settings"), "");
    assert!(!after.body().contains(">www.example.org</code>"), "{}", after.body());
    assert_eq!(under(&server, "www.example.org").status(), "404");
}

#[test]
fn a_server_with_no_resolver_says_why_a_proof_cannot_be_checked() {
    let server = Server::start("domains-offline", &["--site-ttl", "0", "--resolver", "offline"]);
    let browser = owner(&server);
    let machine = publisher(&server, "garden");
    assert_eq!(machine.ink(&["domain", "add", "www.example.org"]).code, 0);
    let proved = machine.ink(&["domain", "prove", "www.example.org"]);
    assert_ne!(proved.code, 0, "{}", proved.said());
    assert!(proved.err.contains("could not be looked up"), "{}", proved.err);
    assert!(proved.err.contains("run `ink domain prove` again"), "{}", proved.err);
    // and the claim is still there to be proved
    assert!(machine.ink(&["domain", "list"]).out.contains("waiting for proof"));

    let page = browser.send(("POST", "/garden/prove"), &field("name", "www.example.org"));
    assert_eq!(page.status(), "503", "{}", page.text);
    assert!(page.body().contains("domain.lookup.failed"), "{}", page.body());
    assert!(page.body().contains("no DNS resolver"), "the cause is on the page: {}", page.body());
}

#[test]
fn a_resolver_setting_out_of_range_stops_the_server_at_start() {
    for (flag, value) in [("--resolver-timeout", "0"), ("--resolver-timeout", "61")] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_ink"))
            .args(["serve", ".", "--addr", "127.0.0.1:0", "--data"])
            .arg(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-rig-domains-flag"))
            .args([flag, value])
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(!out.status.success());
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(said.contains("--resolver-timeout"), "{said}");
    }
}
