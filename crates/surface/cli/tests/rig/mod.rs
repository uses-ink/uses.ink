//! A self-hosted `ink serve --data` and the people of its tests: a browser that speaks raw HTTP
//! with a cookie, and a machine that runs the real `ink` with a home of its own. Shared by the
//! tests that start from the first-run link, as a stranger would.

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "shared by several test crates; tests stop at the first surprise"
)]

use std::fmt::Write as _;
use std::io::{Read, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// What one run of `ink` said.
pub struct Ran {
    pub code: i32,
    pub out: String,
    pub err: String,
}

impl Ran {
    /// stdout and stderr together, for a failure message.
    pub fn said(&self) -> String {
        format!("{}{}", self.out, self.err)
    }

    /// The JSON document of a `--json` run: its last line.
    pub fn json(&self) -> Value {
        serde_json::from_str(self.out.lines().last().unwrap()).unwrap_or_else(|error| panic!("{error}: {}", self.out))
    }
}

/// One HTTP answer, read to the end.
pub struct Got {
    pub text: String,
}

impl Got {
    pub fn status(&self) -> &str {
        self.text.split(' ').nth(1).unwrap_or_default()
    }

    pub fn body(&self) -> &str {
        self.text.split_once("\r\n\r\n").map_or("", |(_, body)| body)
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(self.body()).unwrap_or_else(|error| panic!("{error}: {}", self.text))
    }

    /// The value of the first header called `name`, whatever its case.
    pub fn header(&self, name: &str) -> Option<&str> {
        let head = self.text.split_once("\r\n\r\n").map_or(self.text.as_str(), |(head, _)| head);
        head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }

    /// Every `Set-Cookie` header, whole.
    pub fn set_cookies(&self) -> Vec<String> {
        let head = self.text.split_once("\r\n\r\n").map_or(self.text.as_str(), |(head, _)| head);
        head.lines()
            .skip(1)
            .filter_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("set-cookie").then(|| value.trim().to_owned())
            })
            .collect()
    }

    /// The `Set-Cookie` header whole, for its attributes.
    pub fn set_cookie(&self) -> String {
        self.header("set-cookie").unwrap_or_default().to_owned()
    }

    /// Whether the reply sets no cookie.
    pub fn sets_none(&self) -> bool {
        self.set_cookies().is_empty()
    }

    /// The problem code of an error body.
    pub fn problem(&self) -> String {
        serde_json::from_str::<Value>(self.body())
            .ok()
            .and_then(|body| body["problem"].as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

/// What a browser sends with a request that changes state: the dashboard's own origin.
pub const SAME: (&str, &str) = ("Sec-Fetch-Site", "same-origin");

/// A running `ink serve --data` with a folder of its own, stopped when dropped.
pub struct Server {
    child: std::sync::Mutex<Option<Child>>,
    pub root: PathBuf,
    pub data: PathBuf,
    pub port: u16,
    pub api: String,
    pub audience: String,
    log: PathBuf,
    extra: Vec<String>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Server {
    /// How long a line may take to come, on a loaded machine.
    pub const PATIENCE: Duration = Duration::from_secs(60);

    /// Starts a server on a fresh folder named for the test, with `extra` flags.
    pub fn start(name: &str, extra: &[&str]) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-rig-{name}"));
        std::fs::remove_dir_all(&root).ok();
        let data = root.join("data");
        std::fs::create_dir_all(root.join("served")).unwrap();
        std::fs::write(root.join("served/index.html"), "served").unwrap();
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let server = Self {
            child: std::sync::Mutex::new(None),
            root: root.clone(),
            data,
            port,
            api: format!("http://api.localhost:{port}"),
            audience: format!("api.localhost:{port}"),
            log: root.join("serve.log"),
            extra: extra.iter().map(|flag| (*flag).to_owned()).collect(),
        };
        server.spawn();
        server
    }

    /// Starts the process on the same data folder, port and log, and waits until it is serving.
    fn spawn(&self) {
        let sink = std::fs::OpenOptions::new().create(true).append(true).open(&self.log).unwrap();
        let before = self.said().len();
        let child = Command::new(env!("CARGO_BIN_EXE_ink"))
            .args(["serve", self.root.join("served").to_str().unwrap()])
            .args(["--addr", &format!("127.0.0.1:{}", self.port)])
            .args(["--data", self.data.to_str().unwrap()])
            .args(["--audience", &self.audience, "--auth-skew", "10"])
            // the dashboard's origin is what passkeys are made for: this server's own port
            .args(["--origin", &format!("http://localhost:{}", self.port)])
            .args(&self.extra)
            .stdin(Stdio::null())
            .stdout(sink.try_clone().unwrap())
            .stderr(sink)
            .spawn()
            .unwrap();
        *self.child.lock().unwrap() = Some(child);
        let started = Instant::now();
        while !self.said()[before..].contains("backing off") {
            assert!(started.elapsed() < Self::PATIENCE, "the server did not start: {}", self.said());
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Everything the server has printed, over all its runs.
    pub fn said(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// Stops the process, keeping the data folder.
    pub fn stop(&self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
            child.kill().ok();
            child.wait().ok();
        }
    }

    /// Stops and starts it again on the same data folder.
    pub fn restart(&self) {
        self.stop();
        self.spawn();
    }

    /// Every sign-in link the server has printed, first to last: the first-run link, then mail.
    pub fn links(&self) -> Vec<String> {
        self.said()
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| matches!(c, ',' | ')' | '(' | ';' | '<' | '>' | '"')))
            // the request log names the route too, with the token redacted
            .filter(|word| word.starts_with("http") && word.contains("/v1/login/"))
            .map(str::to_owned)
            .collect()
    }

    /// One raw request with a `Host` of the API, answered in full. A server that is still booting
    /// says so and is asked again.
    pub fn http(&self, (method, target): (&str, &str), headers: &[(&str, &str)], body: &str) -> Got {
        let started = Instant::now();
        loop {
            let got = self.once((method, target), headers, body);
            // a read that gets no bytes at all (a connection a busy host dropped) is asked again
            let dropped = got.text.is_empty() && method.eq_ignore_ascii_case("get");
            if !(dropped || got.text.contains("auth.server.restarting")) || started.elapsed() > Self::PATIENCE {
                return got;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn once(&self, (method, target): (&str, &str), headers: &[(&str, &str)], body: &str) -> Got {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut head = format!(
            "{} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            method.to_ascii_uppercase(),
            self.audience
        );
        for (name, value) in headers {
            write!(head, "{name}: {value}\r\n").unwrap();
        }
        write!(head, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).ok();
        Got { text: String::from_utf8_lossy(&bytes).into_owned() }
    }

    /// A page of a site by its handle.
    pub fn page(&self, handle: &str, path: &str) -> Got {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: {handle}.localhost\r\nConnection: close\r\n\r\n").unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).ok();
        Got { text: String::from_utf8_lossy(&bytes).into_owned() }
    }

    /// A person's browser with no session yet.
    pub fn browser(&self) -> Browser<'_> {
        Browser { server: self, jar: std::collections::BTreeMap::new() }
    }

    /// A machine of someone, with a home of its own and no key.
    pub fn machine(&self, name: &str) -> Machine<'_> {
        let home = self.root.join(format!("home-{name}"));
        std::fs::create_dir_all(&home).unwrap();
        Machine { server: self, home, dir: self.root.clone(), env: Vec::new() }
    }
}

/// A browser: raw requests that carry the cookies the server has set.
pub struct Browser<'a> {
    server: &'a Server,
    jar: std::collections::BTreeMap<String, String>,
}

impl Browser<'_> {
    /// A request as a page of the dashboard makes it: the same origin, and the cookies.
    pub fn send(&self, route: (&str, &str), body: &str) -> Got {
        self.send_with(route, &[SAME], body)
    }

    /// A request with the headers of the caller's choice instead of the same origin's.
    pub fn send_with(&self, route: (&str, &str), headers: &[(&str, &str)], body: &str) -> Got {
        let cookie = self.jar.iter().map(|(name, value)| format!("{name}={value}")).collect::<Vec<_>>().join("; ");
        let mut all = headers.to_vec();
        all.push(("Content-Type", "application/json"));
        if !cookie.is_empty() {
            all.push(("Cookie", &cookie));
        }
        self.server.http(route, &all, body)
    }

    /// Takes the cookies a reply sets and drops the ones it ends.
    pub fn keep(&mut self, got: &Got) {
        for header in got.set_cookies() {
            let (name, value) = header.split(';').next().unwrap().split_once('=').unwrap();
            if header.contains("Max-Age=0") {
                self.jar.remove(name);
            } else {
                self.jar.insert(name.to_owned(), value.to_owned());
            }
        }
    }

    /// The value of the cookie `name`, when the browser holds it.
    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.jar.get(name).map(String::as_str)
    }

    /// Forgets every cookie: a browser with no session.
    pub fn clear(&mut self) {
        self.jar.clear();
    }
}

/// A machine: the real `ink`, run with its own home and no environment of the test runner's.
pub struct Machine<'a> {
    pub server: &'a Server,
    pub home: PathBuf,
    pub dir: PathBuf,
    /// Variables set for every run, such as `INK_KEY_FILE`.
    pub env: Vec<(String, String)>,
}

impl Machine<'_> {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ink"));
        command
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .env("INK_API", &self.server.api)
            .env("INK_TIMEOUT", "20")
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .envs(self.env.iter().map(|(name, value)| (name, value)));
        command
    }

    /// Runs `ink <args>` to its end, again while the server is still booting.
    pub fn ink(&self, args: &[&str]) -> Ran {
        let mut ran = self.run(args);
        for _ in 0..30 {
            if !ran.err.contains("restarting") && !ran.err.contains("api.slow") {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
            ran = self.run(args);
        }
        ran
    }

    fn run(&self, args: &[&str]) -> Ran {
        let out = self.command(args).output().unwrap();
        Ran {
            code: out.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Starts `ink <args>` and leaves it running, with its output read line by line.
    pub fn start(&self, args: &[&str]) -> Waiting {
        let child =
            self.command(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        Waiting::new(child)
    }

    /// Writes `text` at `name` under this machine's folder.
    pub fn write(&self, name: &str, text: &str) {
        let at = self.dir.join(name);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
}

/// A command that waits for a person: its lines, and its end.
pub struct Waiting {
    child: Child,
    lines: std::sync::mpsc::Receiver<String>,
}

impl Waiting {
    fn new(mut child: Child) -> Self {
        let (into, lines) = std::sync::mpsc::channel();
        for stream in [
            Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
            Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
        ] {
            let into = into.clone();
            std::thread::spawn(move || {
                use std::io::BufRead as _;
                for line in std::io::BufReader::new(stream).lines().map_while(Result::ok) {
                    if into.send(line).is_err() {
                        return;
                    }
                }
            });
        }
        Self { child, lines }
    }

    /// The next line that holds `needle`.
    pub fn until(&self, needle: &str) -> String {
        let mut seen = Vec::new();
        loop {
            match self.lines.recv_timeout(Server::PATIENCE) {
                Ok(line) if line.contains(needle) => return line,
                Ok(line) => seen.push(line),
                Err(error) => panic!("no line with `{needle}` came ({error}); saw {seen:#?}"),
            }
        }
    }

    /// Waits for the command to end, and every line it printed.
    pub fn finish(mut self) -> (i32, String) {
        let started = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(started.elapsed() < Server::PATIENCE, "the command did not end");
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut rest = Vec::new();
        while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(300)) {
            rest.push(line);
        }
        (status.code().unwrap_or(-1), rest.join("\n"))
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}
