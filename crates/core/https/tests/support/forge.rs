//! A forge on the loopback for the tests of API calls: a certificate authority made here, a server
//! certificate it signs for the names given, and canned answers by method and path. Each test
//! target that needs one includes this file. No network is touched, and nothing is a git server.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests stop at the first surprise")]
#![allow(dead_code, reason = "each test target uses the part it needs")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

    /// The authority's certificate as a PEM file's text, for a client that reads a file.
    pub(crate) fn pem(&self) -> String {
        self.cert.pem()
    }

    fn server(&self, names: &[&str]) -> Arc<rustls::ServerConfig> {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        for name in names {
            params.subject_alt_names.push(SanType::DnsName((*name).try_into().unwrap()));
        }
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

/// What the forge does with a request for a path.
#[derive(Clone, Debug)]
pub(crate) enum Reply {
    /// Answers with this status and body.
    Status(u16, Vec<u8>),
    /// Answers with a redirect to this address.
    Redirect(String),
    /// Reads the request and never answers.
    Stall,
}

/// A request the forge read.
#[derive(Clone, Debug)]
pub(crate) struct Seen {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Seen {
    /// The first value of the header `name`.
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(have, _)| have.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

type Canned = Arc<Mutex<Vec<(String, String, Reply)>>>;

/// A running forge: it stops when dropped.
pub(crate) struct Forge {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    canned: Canned,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Forge {
    /// A forge whose certificate is for `names`, signed by `authority`, answering `404` for a
    /// path nothing was told about.
    pub(crate) fn start(authority: &Authority, names: &[&str]) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, canned, seen) = (Arc::new(AtomicBool::new(false)), Canned::default(), Arc::default());
        let (config, flag) = (authority.server(names), Arc::clone(&stop));
        let (routes, log) = (Arc::clone(&canned), Arc::clone(&seen));
        std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (config, routes, log) = (Arc::clone(&config), Arc::clone(&routes), Arc::clone(&log));
                        std::thread::spawn(move || {
                            // a client that hangs up or fails verification is the test's to see
                            Self::serve(stream, config, &routes, &log).ok();
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self { address, stop, canned, seen }
    }

    /// Where it listens.
    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    /// Answers `method` `path` (with its query) with `reply` from now on.
    pub(crate) fn reply(&self, method: &str, path: &str, reply: Reply) {
        let mut canned = self.canned.lock().unwrap();
        canned.retain(|(have, at, _)| !(have == method && at == path));
        canned.push((method.to_owned(), path.to_owned(), reply));
    }

    /// Every request read so far, oldest first.
    pub(crate) fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    fn serve(
        stream: TcpStream,
        config: Arc<rustls::ServerConfig>,
        canned: &Canned,
        log: &Arc<Mutex<Vec<Seen>>>,
    ) -> std::io::Result<()> {
        stream.set_nonblocking(false)?;
        let connection = rustls::ServerConnection::new(config).map_err(std::io::Error::other)?;
        let mut tls = rustls::StreamOwned::new(connection, stream);
        let asked = Self::read(&mut tls)?;
        let reply = canned
            .lock()
            .unwrap()
            .iter()
            .find(|(method, path, _)| *method == asked.method && *path == asked.path)
            .map_or_else(|| Reply::Status(404, b"{}".to_vec()), |(_, _, reply)| reply.clone());
        log.lock().unwrap().push(asked);
        match reply {
            Reply::Stall => {
                std::thread::sleep(Duration::from_secs(30));
                Ok(())
            }
            Reply::Redirect(to) => {
                write!(tls, "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
                tls.flush()
            }
            Reply::Status(status, body) => {
                write!(
                    tls,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )?;
                tls.write_all(&body)?;
                tls.conn.send_close_notify();
                tls.flush()
            }
        }
    }

    fn read(tls: &mut impl Read) -> std::io::Result<Seen> {
        let mut reader = BufReader::new(tls);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let mut parts = line.split_whitespace();
        let (method, path) = (parts.next().unwrap_or_default().to_owned(), parts.next().unwrap_or_default().to_owned());
        let (mut headers, mut length) = (Vec::new(), 0_usize);
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            let Some((name, value)) = header.split_once(':') else { continue };
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
            headers.push((name.to_owned(), value.trim().to_owned()));
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        Ok(Seen { method, path, headers, body })
    }
}

impl Drop for Forge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
