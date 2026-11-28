//! `ink preview` serves a deploy with the manifest's statuses and headers.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_cli as cli;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

fn get(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn serves_pages_with_their_headers_and_404() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/notes");
    let preview = cli::Preview::of(&dir, "http://127.0.0.1:4000").unwrap().unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || preview.serve(listener));
    let tea = get(port, "/tea").to_ascii_lowercase();
    assert!(tea.starts_with("http/1.1 200"), "{tea}");
    assert!(tea.contains("content-type: text/html; charset=utf-8"), "{tea}");
    assert!(tea.contains("content-security-policy: default-src 'none'"), "{tea}");
    assert!(tea.contains("<h1"), "{tea}");
    let missing = get(port, "/no/such/page");
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    assert!(missing.contains("Page not found"), "{missing}");
    assert!(get(port, "/style.css").contains("text/css"));
}

#[test]
fn serves_a_route_that_is_not_ascii() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-preview/unicode");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.md"), "# Home\n").unwrap();
    std::fs::write(dir.join("thé.md"), "# Thé vert\n").unwrap();
    let preview = cli::Preview::of(&dir, "http://127.0.0.1:4000").unwrap().unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || preview.serve(listener));
    let tea = get(port, "/th%C3%A9");
    assert!(tea.starts_with("HTTP/1.1 200"), "{tea}");
    assert!(tea.contains("Thé vert"), "{tea}");
    assert!(get(port, "/th%C3%A9/").starts_with("HTTP/1.1 200"));
    assert!(get(port, "/th%C3").starts_with("HTTP/1.1 404"), "not UTF-8");
    assert!(get(port, "/th%zz").starts_with("HTTP/1.1 404"), "no escape");
}

/// Stdout sent to the test while `ink preview` runs in its own thread.
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

#[test]
fn preview_on_port_zero_prints_the_port_it_bound() {
    use clap::Parser;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/notes");
    let cli = cli::Cli::try_parse_from(["ink", "preview", "-C", dir.to_str().unwrap(), "--port", "0"]).unwrap();
    let (sender, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || cli.run(&mut Sent(sender), &mut std::io::sink()));
    let mut printed = String::new();
    while !printed.ends_with('\n') {
        let bytes = received.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        printed.push_str(std::str::from_utf8(&bytes).unwrap());
    }
    let port: u16 = printed.trim().strip_prefix("http://127.0.0.1:").unwrap().trim_end_matches('/').parse().unwrap();
    assert_ne!(port, 0, "{printed}");
    assert!(get(port, "/tea").starts_with("HTTP/1.1 200"));
}
