//! The people and the data of the dashboard pictures: a real `ink serve --data` that is given a site
//! with deploys, a domain claim and an organization by the real `ink`, as the end-to-end tests do,
//! and the sign-in links the browser uses. Nothing here prints a secret: a link is handed to the
//! driver in the plan and is never written to the report.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::super::fault::Fault;
use crate::process::Process;

/// What `ink org create --json` answers, as far as the run needs it.
#[derive(Debug, Deserialize)]
struct Made {
    id: String,
}

/// What `GET /v1/session` answers, as far as the run needs it.
#[derive(Debug, Deserialize)]
struct Seat {
    account: String,
}

/// One answer of the server.
#[derive(Debug)]
struct Reply {
    status: String,
    head: String,
    body: String,
}

/// The files of the pictured site: a repository's worth of notes, with the decisions that are
/// worth a look (a file nothing links to, a post without a title, a folder of drafts).
#[derive(Debug, Clone, Copy)]
struct Content;

impl Content {
    /// The first push.
    const FIRST: [(&'static str, &'static str); 6] = [
        ("README.md", "# Notes\n\nWhat I am reading and building.\n"),
        ("about.md", "# About\n\nOne person, one garden, one shelf of books.\n"),
        ("posts/shelves.md", "---\ndate: 2026-09-02\n---\n# Shelves\n\nHow the shelves went up.\n"),
        ("posts/old/x.md", "Some words with no heading and no title.\n"),
        ("notes/2023.md", "# Notes of 2023\n\nNothing links here.\n"),
        ("drafts/next.md", "---\ndraft: true\n---\n# Next\n\nNot yet.\n"),
    ];

    /// The second push adds a post.
    const SECOND: (&'static str, &'static str) =
        ("posts/rust-traits.md", "---\ndate: 2026-10-11\n---\n# Rust traits\n\nA note on traits.\n");

    /// The third push changes the home page.
    const THIRD: (&'static str, &'static str) =
        ("README.md", "# Notes\n\nWhat I am reading and building, and what I broke.\n");
}

/// A run of the real `ink` and of raw HTTP against one server, with a home of its own.
#[derive(Debug)]
pub(crate) struct Rig {
    ink: PathBuf,
    home: PathBuf,
    port: u16,
    log: PathBuf,
}

impl Rig {
    /// How long a mail gets to be printed.
    const PATIENCE: Duration = Duration::from_secs(30);

    /// The rig of the server on `port` that logs to `log`, run from `home`.
    pub(crate) fn new(ink: &Path, home: &Path, port: u16, log: &Path) -> Self {
        Self { ink: ink.to_path_buf(), home: home.to_path_buf(), port, log: log.to_path_buf() }
    }

    /// The flags that give the server what the pictures need: open sign-up, no site expiry, and
    /// an audience that the real `ink` can reach by name.
    pub(crate) fn flags(&self) -> Vec<String> {
        ["--signup", "open", "--site-ttl", "0", "--auth-skew", "10", "--audience"]
            .into_iter()
            .map(str::to_owned)
            .chain([self.audience()])
            .collect()
    }

    /// The API's name and port.
    fn audience(&self) -> String {
        format!("api.localhost:{}", self.port)
    }

    /// `ink <args>` in `dir`, as a machine of its own, answering its output.
    fn run(&self, dir: &Path, args: &[&str]) -> Result<String, Fault> {
        let mut command = Command::new(&self.ink);
        command
            .arg("-C")
            .arg(dir)
            .args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .env("INK_API", format!("http://{}", self.audience()))
            .env("INK_TIMEOUT", "20")
            .env("PATH", std::env::var_os("PATH").unwrap_or_default());
        let line = format!("ink {}", args.join(" "));
        Process::from(command).text(&[]).map_err(|source| Fault::Command { line, source })
    }

    /// Writes `text` at `name` under `dir`.
    fn write(dir: &Path, (name, text): (&str, &str)) -> Result<(), Fault> {
        let at = dir.join(name);
        let file = |source| Fault::File { path: at.clone(), source };
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).map_err(file)?;
        }
        std::fs::write(&at, text).map_err(file)
    }

    /// Every sign-in link the server has printed so far, in order, as a path.
    fn links(&self) -> Vec<String> {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        text.split_whitespace()
            .map(|word| word.trim_matches(|c: char| matches!(c, ',' | ')' | '(' | ';' | '<' | '>' | '"')))
            .filter(|word| word.starts_with("http") && word.contains("/login/"))
            .map(str::to_owned)
            .collect()
    }

    /// The `count`-th sign-in link (from 1), waited for.
    fn link(&self, count: usize) -> Result<String, Fault> {
        let start = Instant::now();
        loop {
            if let Some(link) = self.links().into_iter().nth(count - 1) {
                return Ok(link);
            }
            if start.elapsed() > Self::PATIENCE {
                let log = std::fs::read_to_string(&self.log).unwrap_or_default();
                return Err(Fault::Mail { count, seconds: Self::PATIENCE.as_secs(), log });
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// One request, as the dashboard's own page makes it, answered in full.
    fn send(&self, (method, target): (&str, &str), cookie: Option<&str>, body: &str) -> Result<Reply, Fault> {
        let call = format!("{method} {target}");
        let io = |source| Fault::Http { call: call.clone(), source };
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).map_err(io)?;
        stream.set_read_timeout(Some(Duration::from_secs(10))).map_err(io)?;
        let jar = cookie.map_or_else(String::new, |cookie| format!("Cookie: {cookie}\r\n"));
        let head = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nSec-Fetch-Site: same-origin\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{jar}\r\n",
            self.audience(),
            body.len()
        );
        stream.write_all(head.as_bytes()).map_err(io)?;
        stream.write_all(body.as_bytes()).map_err(io)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).map_err(io)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
        let status = head.split(' ').nth(1).unwrap_or_default().to_owned();
        Ok(Reply { status, head: head.to_owned(), body: body.to_owned() })
    }

    /// A reply that has the status `wanted`, or the fault that says what came instead.
    fn expect(call: &str, wanted: &str, reply: Reply) -> Result<Reply, Fault> {
        if reply.status == wanted {
            Ok(reply)
        } else {
            Err(Fault::Reply { call: call.to_owned(), status: reply.status, body: reply.body })
        }
    }

    /// Asks for a sign-in link for `address`, and returns its path once the mail is printed.
    /// `count` is how many links the server has printed once this one is among them.
    fn ask(&self, address: &str, count: usize) -> Result<String, Fault> {
        let body = format!("{{\"address\":\"{address}\"}}");
        Self::expect("POST /v1/login", "202", self.send(("POST", "/v1/login"), None, &body)?)?;
        self.link(count)
    }

    /// Signs `link` in over the API, and answers the account id.
    fn enter(&self, link: &str) -> Result<String, Fault> {
        let at = link.find("/login/").and_then(|at| link.get(at..)).unwrap_or(link);
        let target = format!("/v1{at}");
        let entered = Self::expect("POST the sign-in link", "200", self.send(("POST", &target), None, "")?)?;
        let cookie = entered
            .head
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
            .and_then(|(_, value)| value.split(';').next())
            .map(|pair| pair.trim().to_owned())
            .ok_or_else(|| Fault::Reply {
                call: "POST the sign-in link".to_owned(),
                status: entered.status.clone(),
                body: "no session cookie came".to_owned(),
            })?;
        let seen = Self::expect("GET /v1/session", "200", self.send(("GET", "/v1/session"), Some(&cookie), "")?)?;
        let seat: Seat = serde_json::from_str(&seen.body).map_err(|source| Fault::Json { what: "session", source })?;
        Ok(seat.account)
    }

    /// Plants the data: an owner with a site of three deploys, a domain claim and an organization
    /// with a member, and a newcomer who has nothing. Returns the owner's and the newcomer's
    /// sign-in paths, unspent.
    pub(crate) fn plant(&self, data: &Path) -> Result<Planted, Fault> {
        let owner = self.link(1)?;
        let dir = self.home.join("diary");
        std::fs::create_dir_all(&dir).map_err(|source| Fault::File { path: dir.clone(), source })?;
        let data = data.to_string_lossy().into_owned();
        self.run(&dir, &["login", "--data", &data, "--label", "shoot"])?;
        for file in Content::FIRST {
            Self::write(&dir, file)?;
        }
        self.run(&dir, &["init", "--handle", "diary"])?;
        self.run(&dir, &["push", "--wait", "30"])?;
        Self::write(&dir, Content::SECOND)?;
        self.run(&dir, &["push", "--wait", "30"])?;
        Self::write(&dir, Content::THIRD)?;
        self.run(&dir, &["push", "--wait", "30"])?;
        self.run(&dir, &["domain", "add", "www.example.org"])?;
        let ann = self.enter(&self.ask("ann@example.org", 2)?)?;
        let made = self.run(&dir, &["org", "create", "Garden club", "--json"])?;
        let org: Made = serde_json::from_str(made.lines().last().unwrap_or_default())
            .map_err(|source| Fault::Json { what: "org", source })?;
        self.run(&dir, &["org", "member", "add", &org.id, &ann, "--role", "writer"])?;
        let newcomer = self.ask("bo@example.org", 3)?;
        Ok(Planted { owner, newcomer })
    }
}

/// The sign-in paths the planted data leaves for the browser.
#[derive(Debug)]
pub(crate) struct Planted {
    /// The owner's: the first-run link.
    pub(crate) owner: String,
    /// The newcomer's: a mail link of an account with no site.
    pub(crate) newcomer: String,
}
