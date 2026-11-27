//! A git remote over TLS on the loopback, for the tests of https (decision 0072): a certificate
//! authority made here, a server certificate it signs for `127.0.0.1`, and a smart HTTP handler
//! that answers `git upload-pack` with the machine's `git`, so `git` must be on the path. Included
//! by each test target that needs one. No network is touched.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests stop at the first surprise")]
#![allow(dead_code, reason = "each test target uses the part it needs")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// A certificate authority and the key it signs server certificates with.
pub(crate) struct Authority {
    cert: rcgen::Certificate,
    issuer: Issuer<'static, KeyPair>,
}

impl Authority {
    pub(crate) fn new() -> Self {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let cert = params.self_signed(&key).unwrap();
        Self { cert, issuer: Issuer::new(params, key) }
    }

    /// The authority's certificate, which a client trusts to trust what it signed.
    pub(crate) fn root(&self) -> CertificateDer<'static> {
        self.cert.der().clone()
    }

    /// The TLS server configuration of a certificate for `127.0.0.1` signed by this authority.
    fn server(&self) -> Arc<rustls::ServerConfig> {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params.subject_alt_names.push(SanType::IpAddress(Ipv4Addr::LOCALHOST.into()));
        let leaf = params.signed_by(&key, &self.issuer).unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![leaf.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
            )
            .unwrap();
        Arc::new(config)
    }
}

/// What the server does with a request for the repository's references.
#[derive(Clone, Debug)]
pub(crate) enum Mode {
    /// Answer as `git http-backend` does.
    Serve,
    /// Answer every request with a redirect to this address.
    Redirect(String),
    /// Read the request and never answer.
    Stall,
}

/// A running server: it stops when dropped.
pub(crate) struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
}

impl Server {
    /// Serves the repository at `repo` over TLS signed by `authority`.
    pub(crate) fn start(authority: &Authority, repo: &Path, mode: Mode) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (stop, requests) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
        let (config, repo) = (authority.server(), repo.to_path_buf());
        let (flag, count) = (Arc::clone(&stop), Arc::clone(&requests));
        std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (config, repo, mode, count) =
                            (Arc::clone(&config), repo.clone(), mode.clone(), Arc::clone(&count));
                        std::thread::spawn(move || {
                            // a client that hangs up or fails verification is the test's to see
                            Handler { repo, mode, count }.run(stream, config).ok();
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self { port, stop, requests }
    }

    /// The address of the repository served, as a git url.
    pub(crate) fn url(&self) -> String {
        format!("https://127.0.0.1:{}/site.git", self.port)
    }

    /// How many requests were read.
    pub(crate) fn requests(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A plain TCP listener that counts connections: what a downgrade redirect would reach.
pub(crate) struct Plain {
    port: u16,
    stop: Arc<AtomicBool>,
    accepted: Arc<AtomicUsize>,
}

impl Plain {
    pub(crate) fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (stop, accepted) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
        let (flag, count) = (Arc::clone(&stop), Arc::clone(&accepted));
        std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                if listener.accept().is_ok() {
                    count.fetch_add(1, Ordering::Relaxed);
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });
        Self { port, stop, accepted }
    }

    pub(crate) fn url(&self) -> String {
        format!("http://127.0.0.1:{}/site.git", self.port)
    }

    pub(crate) fn accepted(&self) -> usize {
        self.accepted.load(Ordering::Relaxed)
    }
}

impl Drop for Plain {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// One request, read and answered.
struct Handler {
    repo: PathBuf,
    mode: Mode,
    count: Arc<AtomicUsize>,
}

/// The head and body of a request.
struct Asked {
    line: String,
    protocol: Option<String>,
    body: Vec<u8>,
}

impl Handler {
    fn run(&self, stream: TcpStream, config: Arc<rustls::ServerConfig>) -> std::io::Result<()> {
        stream.set_nonblocking(false)?;
        let connection = rustls::ServerConnection::new(config).map_err(std::io::Error::other)?;
        let mut tls = rustls::StreamOwned::new(connection, stream);
        let asked = Self::read(&mut tls)?;
        self.count.fetch_add(1, Ordering::Relaxed);
        match &self.mode {
            Mode::Stall => {
                std::thread::sleep(Duration::from_secs(30));
                Ok(())
            }
            Mode::Redirect(to) => {
                write!(tls, "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
                tls.flush()
            }
            Mode::Serve => {
                self.serve(&mut tls, &asked)?;
                tls.conn.send_close_notify();
                tls.flush()
            }
        }
    }

    fn read(tls: &mut impl Read) -> std::io::Result<Asked> {
        let mut reader = BufReader::new(tls);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let (mut length, mut chunked, mut protocol) = (0_usize, false, None);
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim_end().to_owned();
            if header.is_empty() {
                break;
            }
            let Some((name, value)) = header.split_once(':') else { continue };
            let (name, value) = (name.to_ascii_lowercase(), value.trim().to_owned());
            match name.as_str() {
                "content-length" => length = value.parse().unwrap_or(0),
                "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
                "git-protocol" => protocol = Some(value),
                _ => {}
            }
        }
        let mut body = Vec::new();
        if chunked {
            loop {
                let mut size = String::new();
                reader.read_line(&mut size)?;
                let size = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
                if size == 0 {
                    break;
                }
                let mut chunk = vec![0; size + 2];
                reader.read_exact(&mut chunk)?;
                chunk.truncate(size);
                body.extend(chunk);
            }
        } else {
            body.resize(length, 0);
            reader.read_exact(&mut body)?;
        }
        Ok(Asked { line, protocol, body })
    }

    fn serve(&self, tls: &mut impl Write, asked: &Asked) -> std::io::Result<()> {
        let advertise = asked.line.starts_with("GET") && asked.line.contains("/info/refs?service=git-upload-pack");
        let mut command = Command::new("git");
        command.arg("upload-pack").arg("--stateless-rpc");
        if advertise {
            command.arg("--advertise-refs");
        }
        command.arg(&self.repo).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Some(protocol) = &asked.protocol {
            command.env("GIT_PROTOCOL", protocol);
        }
        let mut child = command.spawn()?;
        let mut stdin = child.stdin.take().expect("piped");
        let body = asked.body.clone();
        let writer = std::thread::spawn(move || stdin.write_all(&body).ok());
        let output = child.wait_with_output()?;
        writer.join().ok();
        let mut answer = Vec::new();
        let version_two = asked.protocol.as_deref().is_some_and(|protocol| protocol.contains("version=2"));
        let kind = if advertise {
            if !version_two {
                answer.extend_from_slice(b"001e# service=git-upload-pack\n0000");
            }
            "advertisement"
        } else {
            "result"
        };
        answer.extend_from_slice(&output.stdout);
        write!(
            tls,
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-{kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            answer.len()
        )?;
        tls.write_all(&answer)?;
        tls.flush()
    }
}
