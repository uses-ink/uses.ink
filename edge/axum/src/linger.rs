//! Closing a connection so the peer can still read the reply.
//!
//! A socket closed while the peer's bytes are unread makes the system send a reset, and a reset
//! can discard a reply the peer has not read yet. A refused request is exactly that case: the
//! edge answers a head and closes without reading what the client went on to send. So the close is
//! lingering: the reply is flushed, the write side is shut (the peer sees the end of the reply),
//! and what the peer still sends is read and dropped, for as long as a client may take to send a
//! head and for as many bytes as a head may hold, then the socket is closed.

use std::time::Duration;

use edge::Limits;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use trace::Field as _;

/// How many bytes one read of the dropped input takes.
const SCRATCH: usize = 4096;

/// A second handle on a connection's socket, kept after hyper has dropped its own: the socket is
/// not closed, and no reset is sent, until this one is.
#[derive(Debug)]
pub(crate) struct Linger {
    twin: std::net::TcpStream,
    bound: u64,
    deadline: Duration,
}

impl Linger {
    /// The stream hyper serves and the handle that outlives it. The bound is [`Limits::head`], the
    /// most a request may send before it is refused, and the deadline is [`Limits::read`], the
    /// time it has to send it.
    pub(crate) fn new(stream: TcpStream, limits: &Limits) -> Result<(TcpStream, Self), std::io::Error> {
        let plain = stream.into_std()?;
        let twin = plain.try_clone()?;
        let linger = Self { twin, bound: limits.head.count(), deadline: limits.read };
        Ok((TcpStream::from_std(plain)?, linger))
    }

    /// Ends the reply and reads out what the peer still sends. A failure here is the peer's
    /// going away, with the reply already written, so there is nothing left to report.
    pub(crate) async fn close(self) {
        let Ok(mut stream) = TcpStream::from_std(self.twin) else { return };
        if stream.shutdown().await.is_err() {
            return;
        }
        let mut seen = 0_u64;
        let mut scratch = [0_u8; SCRATCH];
        let drained = async {
            while seen < self.bound {
                match stream.read(&mut scratch).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => seen += read as u64,
                }
            }
        };
        let finished = tokio::time::timeout(self.deadline, drained).await.is_ok();
        trace::debug!(
            name: trace::event::CONNECTION_LINGERED,
            drained = finished,
            bytes = trace::Size(seen).field(),
        );
    }
}
