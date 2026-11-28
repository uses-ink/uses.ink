//! The resolver that asks a real DNS server for the TXT records of a host (RFC 1035): a question
//! over UDP from a random port with a random id and a random case of the name's letters, and over
//! TCP when the answer was cut. DNSSEC is not checked: an answer is trusted as far as the path to
//! the nameserver is, so the operator names nameservers it trusts, and may name a second resolver
//! that must give the same answer ([`Chosen::agreeing`](crate::Chosen)).
//!
//! The nameservers are the ones of the system (`/etc/resolv.conf`, read at each lookup so a change
//! shows) or the operator's own list. A lookup is bounded: each nameserver has its share of the
//! limit, so a dead one cannot take the time of the rest. An answer that is not to this question,
//! from this address, with this id, is not read: a datagram that is not the answer is waited past,
//! so what a stranger sends to the port cannot make a proof.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clock::Draw;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpStream, UdpSocket};

use super::failed::{Failed, Unlisted};
use super::wire::{Malformed, Query, Reply};
use crate::{Host, Resolver};

/// Where the nameservers are named.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Servers {
    /// These addresses, in this order.
    Listed(Vec<SocketAddr>),
    /// The `nameserver` lines of this file.
    File(PathBuf),
}

impl Servers {
    /// The file the system keeps its nameservers in.
    pub const SYSTEM: &'static str = "/etc/resolv.conf";

    /// The port a nameserver listens on when its address has none.
    pub const PORT: u16 = 53;

    /// The system's own nameservers.
    pub fn system() -> Self {
        Self::File(PathBuf::from(Self::SYSTEM))
    }

    /// The nameservers a file's `text` names: one `nameserver <address>` a line; other lines and
    /// comments are not read.
    pub fn read(text: &str, path: &Path) -> Result<Vec<SocketAddr>, Unlisted> {
        let (mut found, mut skipped) = (Vec::new(), 0);
        for line in text.lines() {
            let line = line.split(['#', ';']).next().unwrap_or_default();
            let mut words = line.split_whitespace();
            if words.next() != Some("nameserver") {
                continue;
            }
            match words.next().and_then(|word| word.parse::<IpAddr>().ok()) {
                Some(address) => found.push(SocketAddr::new(address, Self::PORT)),
                None => skipped += 1,
            }
        }
        if found.is_empty() { Err(Unlisted::None { path: path.to_path_buf(), skipped }) } else { Ok(found) }
    }

    /// The addresses to ask, now.
    fn addresses(&self) -> Result<Vec<SocketAddr>, Unlisted> {
        match self {
            Self::Listed(listed) => Ok(listed.clone()),
            Self::File(path) => {
                let text =
                    std::fs::read_to_string(path).map_err(|source| Unlisted::Read { path: path.clone(), source })?;
                Self::read(&text, path)
            }
        }
    }
}

/// A nameserver as an operator writes it: an address, with a port or with the default one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nameserver(pub SocketAddr);

impl std::str::FromStr for Nameserver {
    type Err = std::net::AddrParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse::<SocketAddr>()
            .or_else(|_| text.parse::<IpAddr>().map(|address| SocketAddr::new(address, Servers::PORT)))
            .map(Self)
    }
}

/// A resolver that asks nameservers, within a limit.
#[derive(Clone, Debug)]
pub struct System<D> {
    servers: Servers,
    limit: Duration,
    draw: D,
}

impl<D: Draw> System<D> {
    /// The longest a lookup takes when nothing says: five seconds.
    pub const LIMIT: Duration = Duration::from_secs(5);

    /// A resolver over `servers` that gives a lookup at most `limit` in all, with ids from `draw`.
    pub const fn new(servers: Servers, limit: Duration, draw: D) -> Self {
        Self { servers, limit, draw }
    }

    /// The limit.
    pub const fn limit(&self) -> Duration {
        self.limit
    }

    /// The question for `host`: a random id and a random case for each letter of its name, from
    /// one draw.
    fn question(&self, host: &Host) -> Result<Query, Failed> {
        let bytes = self.draw.bytes().map_err(|source| Failed::Draw(Box::new(source)))?;
        let [first, second, mask @ ..] = bytes;
        Ok(Query::cased(u16::from_be_bytes([first, second]), &host.to_string(), &mask))
    }

    /// One nameserver's answer, within `share`.
    async fn once(&self, server: SocketAddr, host: &Host, share: Duration) -> Result<Vec<String>, Failed> {
        let query = self.question(host)?;
        let asked = async {
            let reply = match Self::datagram(server, &query).await? {
                reply if reply.cut => Self::stream(server, &query).await?,
                reply => reply,
            };
            match reply.code {
                0 => Ok(reply.texts),
                // the name has no records at all: an answer, and an empty one
                3 => Ok(Vec::new()),
                code => Err(Failed::Rcode { server, code }),
            }
        };
        tokio::time::timeout(share, asked).await.unwrap_or(Err(Failed::Timeout { server, after: share }))
    }

    /// The question over UDP from a port of the system's choosing. Datagrams that are not the
    /// answer to it are waited past, until the caller's time runs out.
    async fn datagram(server: SocketAddr, query: &Query) -> Result<Reply, Failed> {
        let io = |source| Failed::Io { server, source };
        let message = query.encode().map_err(Failed::Question)?;
        let local = if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        let socket = UdpSocket::bind(local).await.map_err(io)?;
        socket.connect(server).await.map_err(io)?;
        socket.send(&message).await.map_err(io)?;
        let mut buffer = [0_u8; 4096];
        loop {
            let length = socket.recv(&mut buffer).await.map_err(io)?;
            match query.read(buffer.get(..length).unwrap_or_default()) {
                Ok(reply) => return Ok(reply),
                Err(Malformed::Id { .. } | Malformed::Question | Malformed::Echo) => {}
                Err(source) => return Err(Failed::Malformed { server, source }),
            }
        }
    }

    /// The question over TCP, for an answer that did not fit a datagram.
    async fn stream(server: SocketAddr, query: &Query) -> Result<Reply, Failed> {
        let io = |source| Failed::Io { server, source };
        let message = query.encode().map_err(Failed::Question)?;
        let size =
            u16::try_from(message.len()).map_err(|_| Failed::Question(Malformed::Host { host: query.host.clone() }))?;
        let mut stream = TcpStream::connect(server).await.map_err(io)?;
        stream.write_all(&size.to_be_bytes()).await.map_err(io)?;
        stream.write_all(&message).await.map_err(io)?;
        let mut head = [0_u8; 2];
        stream.read_exact(&mut head).await.map_err(io)?;
        let mut answer = vec![0_u8; usize::from(u16::from_be_bytes(head))];
        stream.read_exact(&mut answer).await.map_err(io)?;
        query.read(&answer).map_err(|source| Failed::Malformed { server, source })
    }
}

impl<D: Draw> Resolver for System<D> {
    type Error = Failed;

    async fn txt(&self, host: &Host) -> Result<Vec<String>, Failed> {
        let servers = self.servers.addresses().map_err(Failed::Servers)?;
        let count = u32::try_from(servers.len()).unwrap_or(u32::MAX).max(1);
        let share = self.limit / count;
        let mut last = None;
        for server in &servers {
            match self.once(*server, host, share).await {
                Ok(texts) => return Ok(texts),
                Err(failed) => last = Some(failed),
            }
        }
        match last {
            Some(last) if servers.len() == 1 => Err(last),
            Some(last) => Err(Failed::Exhausted { tried: servers.len(), last: Box::new(last) }),
            None => Err(Failed::Servers(Unlisted::None { path: PathBuf::from(Servers::SYSTEM), skipped: 0 })),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::sync::atomic::{AtomicU8, Ordering};

    use clock::Draw;
    use tokio::net::TcpListener;

    /// A source of ids that counts up, for a test that must know the id.
    #[derive(Debug)]
    struct Counting(AtomicU8);

    impl Draw for Counting {
        type Error = std::convert::Infallible;

        fn bytes(&self) -> Result<[u8; 16], Self::Error> {
            let next = self.0.fetch_add(1, Ordering::Relaxed);
            Ok([0, next, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        }
    }

    fn resolver(servers: Vec<SocketAddr>, limit: Duration) -> System<Counting> {
        System::new(Servers::Listed(servers), limit, Counting(AtomicU8::new(1)))
    }

    fn host() -> Host {
        Host::of(&"example.org".parse().unwrap())
    }

    /// The question the resolver sends first: id 1.
    fn first() -> Query {
        Query { id: 1, host: host().to_string() }
    }

    /// A nameserver on a UDP socket that sends `replies` to the first question it reads.
    async fn serving(replies: Vec<Vec<u8>>) -> SocketAddr {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buffer = [0_u8; 512];
            let (_, from) = socket.recv_from(&mut buffer).await.unwrap();
            for reply in replies {
                socket.send_to(&reply, from).await.unwrap();
            }
        });
        address
    }

    #[tokio::test]
    async fn the_records_of_the_host_are_what_the_nameserver_answers() {
        let query = first();
        let server = serving(vec![query.answered(&[&["uses-ink-domain=", "abc"]], 0)]).await;
        let found = resolver(vec![server], Duration::from_millis(500)).txt(&host()).await.unwrap();
        assert_eq!(found, ["uses-ink-domain=abc"]);
    }

    #[tokio::test]
    async fn a_datagram_that_is_not_the_answer_is_waited_past() {
        let query = first();
        let stranger = Query { id: 99, ..query.clone() }.answered(&[&["uses-ink-domain=forged"]], 0);
        let elsewhere = Query { host: "_uses-ink.other.org".to_owned(), ..query.clone() }.answered(&[&["x"]], 0);
        let server = serving(vec![stranger, elsewhere, query.answered(&[&["uses-ink-domain=real"]], 0)]).await;
        let found = resolver(vec![server], Duration::from_millis(500)).txt(&host()).await.unwrap();
        assert_eq!(found, ["uses-ink-domain=real"]);
    }

    /// A source whose every byte after the id is set, so every letter of the name is upper case.
    #[derive(Debug)]
    struct Loud;

    impl Draw for Loud {
        type Error = std::convert::Infallible;

        fn bytes(&self) -> Result<[u8; 16], Self::Error> {
            Ok([0, 1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
        }
    }

    #[tokio::test]
    async fn only_an_answer_that_echoes_the_random_case_is_read() {
        let asked = Query::cased(1, &host().to_string(), &[0xff; 14]);
        assert_ne!(asked.host, host().to_string(), "the question is not the plain name");
        let folded = Query { host: host().to_string(), ..asked.clone() }.answered(&[&["uses-ink-domain=forged"]], 0);
        let server = serving(vec![folded, asked.answered(&[&["uses-ink-domain=real"]], 0)]).await;
        let loud = System::new(Servers::Listed(vec![server]), Duration::from_millis(500), Loud);
        assert_eq!(loud.txt(&host()).await.unwrap(), ["uses-ink-domain=real"]);
    }

    #[tokio::test]
    async fn a_name_with_no_records_is_an_empty_answer_and_a_server_failure_is_an_error() {
        let query = first();
        let missing = serving(vec![query.answered(&[], 3)]).await;
        assert_eq!(
            resolver(vec![missing], Duration::from_millis(500)).txt(&host()).await.unwrap(),
            Vec::<String>::new()
        );
        let broken = serving(vec![query.answered(&[], 2)]).await;
        let failed = resolver(vec![broken], Duration::from_millis(500)).txt(&host()).await.unwrap_err();
        assert!(matches!(failed, Failed::Rcode { code: 2, .. }), "{failed:?}");
    }

    #[tokio::test]
    async fn two_resolvers_that_must_agree_answer_only_what_both_do() {
        let same = |records: &'static str| async move {
            let query = first();
            serving(vec![query.answered(&[&[records]], 0)]).await
        };
        let (a, b) = (same("uses-ink-domain=abc").await, same("uses-ink-domain=abc").await);
        let both = |first, second| {
            super::super::Agreeing::new(
                resolver(vec![first], Duration::from_millis(500)),
                resolver(vec![second], Duration::from_millis(500)),
            )
        };
        assert_eq!(both(a, b).txt(&host()).await.unwrap(), ["uses-ink-domain=abc"]);
        let (a, c) = (same("uses-ink-domain=abc").await, same("uses-ink-domain=forged").await);
        let failed = both(a, c).txt(&host()).await.unwrap_err();
        assert!(matches!(failed, Failed::Disagree { first: 1, second: 1 }), "{failed:?}");
        // a second path that is down is not agreement
        let silent = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let a = same("uses-ink-domain=abc").await;
        let down = super::super::Agreeing::new(
            resolver(vec![a], Duration::from_millis(500)),
            resolver(vec![silent.local_addr().unwrap()], Duration::from_millis(20)),
        );
        assert!(matches!(down.txt(&host()).await.unwrap_err(), Failed::Timeout { .. }));
    }

    #[tokio::test]
    async fn a_silent_nameserver_ends_at_the_limit_and_the_next_is_asked() {
        let silent = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let query = first();
        let answers = serving(vec![Query { id: 2, ..query }.answered(&[&["uses-ink-domain=abc"]], 0)]).await;
        let both = resolver(vec![silent.local_addr().unwrap(), answers], Duration::from_millis(80));
        assert_eq!(both.txt(&host()).await.unwrap(), ["uses-ink-domain=abc"]);
        let alone = resolver(vec![silent.local_addr().unwrap()], Duration::from_millis(20));
        let failed = alone.txt(&host()).await.unwrap_err();
        assert!(matches!(failed, Failed::Timeout { after, .. } if after == Duration::from_millis(20)), "{failed:?}");
        let all = resolver(vec![silent.local_addr().unwrap(), silent.local_addr().unwrap()], Duration::from_millis(20));
        assert!(matches!(all.txt(&host()).await.unwrap_err(), Failed::Exhausted { tried: 2, .. }));
    }

    #[tokio::test]
    async fn a_cut_answer_is_asked_again_over_a_stream() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let udp = UdpSocket::bind(address).await.unwrap();
        let query = first();
        let cut = query.answered(&[], 0x0200);
        let whole = query.answered(&[&["uses-ink-domain=long"]], 0);
        tokio::spawn(async move {
            let mut buffer = [0_u8; 512];
            let (_, from) = udp.recv_from(&mut buffer).await.unwrap();
            udp.send_to(&cut, from).await.unwrap();
        });
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut head = [0_u8; 2];
            stream.read_exact(&mut head).await.unwrap();
            let mut asked = vec![0_u8; usize::from(u16::from_be_bytes(head))];
            stream.read_exact(&mut asked).await.unwrap();
            stream.write_all(&u16::try_from(whole.len()).unwrap().to_be_bytes()).await.unwrap();
            stream.write_all(&whole).await.unwrap();
        });
        let found = resolver(vec![address], Duration::from_millis(500)).txt(&host()).await.unwrap();
        assert_eq!(found, ["uses-ink-domain=long"]);
    }

    #[test]
    fn a_nameserver_is_an_address_with_or_without_a_port() {
        assert_eq!("192.0.2.1".parse::<Nameserver>().unwrap().0, "192.0.2.1:53".parse().unwrap());
        assert_eq!("[2001:db8::1]:5353".parse::<Nameserver>().unwrap().0, "[2001:db8::1]:5353".parse().unwrap());
        assert_eq!("2001:db8::1".parse::<Nameserver>().unwrap().0.port(), 53);
        assert!("dns.example".parse::<Nameserver>().is_err());
    }

    #[test]
    fn the_nameservers_of_a_file_are_its_addresses_and_none_is_an_error() {
        let text = "# generated\nsearch lan\nnameserver 192.0.2.1\nnameserver 2001:db8::1 ; v6\nnameserver fe80::1%eth0\noptions edns0\n";
        let read = Servers::read(text, Path::new("resolv.conf")).unwrap();
        assert_eq!(read, ["192.0.2.1:53".parse::<SocketAddr>().unwrap(), "[2001:db8::1]:53".parse().unwrap()]);
        let none = Servers::read("search lan\nnameserver fe80::1%eth0\n", Path::new("resolv.conf")).unwrap_err();
        assert!(matches!(none, Unlisted::None { skipped: 1, .. }), "{none:?}");
    }

    #[tokio::test]
    async fn a_file_that_is_missing_says_which() {
        let system = System::new(
            Servers::File(PathBuf::from("/nonexistent/resolv.conf")),
            Duration::from_millis(50),
            Counting(AtomicU8::new(1)),
        );
        let failed = system.txt(&host()).await.unwrap_err();
        assert!(matches!(failed, Failed::Servers(Unlisted::Read { .. })), "{failed:?}");
    }
}
