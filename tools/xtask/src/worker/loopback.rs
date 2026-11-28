//! A tiny HTTP client for a Worker on this machine: a call with a body of known or unknown
//! length, and the reply read whole. `curl` cannot send a chunked body with chosen chunk
//! boundaries, which is the point of the R2 spike, so the bytes are written by hand.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::fault::{Fault, Wire};

/// The bytes a chunked call sends: a size, cut in pieces, from a seed, so both ends can hash them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stream {
    /// How many bytes in all.
    pub(crate) total: usize,
    /// How many bytes in one chunk.
    pub(crate) piece: usize,
    /// What the bytes derive from.
    pub(crate) seed: u64,
}

impl Stream {
    /// The bytes, in order, as one buffer.
    pub(crate) fn bytes(&self) -> Vec<u8> {
        let mut state = self.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut bytes = Vec::with_capacity(self.total);
        while bytes.len() < self.total {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.extend_from_slice(&state.to_le_bytes());
        }
        bytes.truncate(self.total);
        bytes
    }

    /// The BLAKE3 hash of the bytes, in hex.
    pub(crate) fn hash(&self) -> String {
        blake3::hash(&self.bytes()).to_hex().to_string()
    }
}

/// What a call carries.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Body {
    /// Nothing, with `content-length: 0`.
    Empty,
    /// A stream sent as `transfer-encoding: chunked`, so its length is not known to the receiver.
    Chunked(Stream),
    /// The same bytes with a `content-length`.
    Fixed(Stream),
}

/// One request.
#[derive(Debug, Clone)]
pub(crate) struct Call {
    pub(crate) method: &'static str,
    pub(crate) path: String,
    pub(crate) body: Body,
}

impl Call {
    /// A call with no body.
    pub(crate) fn get(path: impl Into<String>) -> Self {
        Self { method: "GET", path: path.into(), body: Body::Empty }
    }

    /// A call that posts nothing.
    pub(crate) fn post(path: impl Into<String>) -> Self {
        Self { method: "POST", path: path.into(), body: Body::Empty }
    }

    /// A call that puts a body.
    pub(crate) fn put(path: impl Into<String>, body: Body) -> Self {
        Self { method: "PUT", path: path.into(), body }
    }
}

/// What came back.
#[derive(Debug, Clone)]
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
}

impl Reply {
    /// The body as text, with what is not UTF-8 replaced.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Reads a whole HTTP/1.1 response, `chunked` or not.
    fn parse(raw: &[u8]) -> Result<Self, Wire> {
        let split = raw.windows(4).position(|window| window == b"\r\n\r\n").ok_or(Wire::Head)?;
        let (head, rest) = raw.split_at(split);
        let rest = rest.get(4..).unwrap_or_default();
        let head = String::from_utf8_lossy(head);
        let mut lines = head.lines();
        let status = lines
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or(Wire::Status)?;
        let chunked = lines.any(|line| {
            let lower = line.to_ascii_lowercase();
            lower.starts_with("transfer-encoding:") && lower.contains("chunked")
        });
        let body = if chunked { Self::unchunk(rest)? } else { rest.to_vec() };
        Ok(Self { status, body })
    }

    /// The payload of a chunked body.
    fn unchunk(mut rest: &[u8]) -> Result<Vec<u8>, Wire> {
        let mut body = Vec::new();
        loop {
            let end = rest.windows(2).position(|window| window == b"\r\n").ok_or(Wire::Chunk)?;
            let size = rest.get(..end).ok_or(Wire::Chunk)?;
            let size = std::str::from_utf8(size).map_err(|_| Wire::Chunk)?;
            let size = usize::from_str_radix(size.split(';').next().unwrap_or_default().trim(), 16)
                .map_err(|_| Wire::Chunk)?;
            rest = rest.get(end + 2..).ok_or(Wire::Chunk)?;
            if size == 0 {
                return Ok(body);
            }
            body.extend_from_slice(rest.get(..size).ok_or(Wire::Chunk)?);
            rest = rest.get(size + 2..).ok_or(Wire::Chunk)?;
        }
    }
}

/// The Worker's address.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Loopback {
    pub(crate) port: u16,
}

impl Loopback {
    /// How long a call may take, whole.
    const WITHIN: Duration = Duration::from_secs(300);

    /// Sends `call` and reads the reply.
    pub(crate) fn send(&self, call: &Call) -> Result<Reply, Fault> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).map_err(Fault::Connect)?;
        stream.set_read_timeout(Some(Self::WITHIN)).map_err(Fault::Connect)?;
        stream.set_write_timeout(Some(Self::WITHIN)).map_err(Fault::Connect)?;
        let framing = match call.body {
            Body::Empty => "content-length: 0".to_owned(),
            Body::Chunked(_) => "transfer-encoding: chunked".to_owned(),
            Body::Fixed(stream) => format!("content-length: {}", stream.total),
        };
        let head = format!(
            "{} {} HTTP/1.1\r\nhost: 127.0.0.1:{}\r\nconnection: close\r\n{framing}\r\n\r\n",
            call.method, call.path, self.port
        );
        // A Worker may answer, and close, before the whole body is sent; its answer is the result then.
        let sent = Self::write(&mut stream, &head, call.body);
        let mut raw = Vec::new();
        let read = stream.read_to_end(&mut raw);
        match (Reply::parse(&raw), sent, read) {
            (Ok(reply), _, _) => Ok(reply),
            (Err(wire), Err(error), _) => Err(Fault::Send { path: call.path.clone(), wire, source: error }),
            (Err(wire), Ok(()), Err(error)) => Err(Fault::Send { path: call.path.clone(), wire, source: error }),
            (Err(wire), Ok(()), Ok(_)) => Err(Fault::Wire { path: call.path.clone(), wire }),
        }
    }

    fn write(stream: &mut TcpStream, head: &str, body: Body) -> std::io::Result<()> {
        stream.write_all(head.as_bytes())?;
        match body {
            Body::Empty => Ok(()),
            Body::Fixed(source) => stream.write_all(&source.bytes()),
            Body::Chunked(source) => {
                for piece in source.bytes().chunks(source.piece.max(1)) {
                    stream.write_all(format!("{:x}\r\n", piece.len()).as_bytes())?;
                    stream.write_all(piece)?;
                    stream.write_all(b"\r\n")?;
                }
                stream.write_all(b"0\r\n\r\n")
            }
        }
    }
}
