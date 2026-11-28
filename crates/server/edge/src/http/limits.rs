//! The operator's budgets for one request and one response (decision 0044, bucket 2).

use std::time::Duration;

use model::Bytes;

/// What a request and its response may cost. Every field is the operator's to set; a request over
/// a size budget is refused with the number and the limit, and the times are enforced by the
/// adapter that owns the sockets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The request target, path and query as received.
    pub target: Bytes,
    /// Segments in a path.
    pub segments: usize,
    /// Headers in a request.
    pub headers: usize,
    /// One header's value.
    pub header: Bytes,
    /// Every header's name and value together.
    pub head: Bytes,
    /// Entries in an `If-None-Match` list.
    pub tags: usize,
    /// The largest file served.
    pub file: Bytes,
    /// Time for a client to send its request head.
    pub read: Duration,
    /// Time to resolve a host and open a blob.
    pub open: Duration,
    /// Time a response may wait for its reader to take the next chunk.
    pub idle: Duration,
    /// Time for a client to send the body of an API request, which a push makes large.
    pub body: Duration,
    /// Time for one whole response, from the request head to its last byte.
    pub send: Duration,
    /// Time open connections get to finish when the server is told to stop.
    pub drain: Duration,
    /// Requests in flight at once, from a head read to the last byte sent. A request over it is
    /// refused with a 503.
    pub requests: usize,
    /// Connections open at once, including ones that have sent no head yet. The accept loop takes
    /// a slot before it takes a connection, so a flood waits in the system's backlog instead of
    /// using descriptors. Keep it under the process's descriptor limit, with room for files.
    pub connections: usize,
}

impl Default for Limits {
    fn default() -> Self {
        const KIB: u64 = 1024;
        Self {
            target: Bytes(4 * KIB),
            segments: 32,
            headers: 64,
            header: Bytes(8 * KIB),
            head: Bytes(32 * KIB),
            tags: 16,
            file: Bytes(1024 * 1024 * KIB),
            read: Duration::from_secs(10),
            open: Duration::from_secs(5),
            idle: Duration::from_secs(30),
            body: Duration::from_secs(600),
            send: Duration::from_secs(600),
            drain: Duration::from_secs(30),
            requests: 1024,
            connections: 1024,
        }
    }
}
