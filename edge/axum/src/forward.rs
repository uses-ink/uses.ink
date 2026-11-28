//! Where a request really came from, behind a proxy.
//!
//! Behind a reverse proxy every connection comes from the proxy, so a limit by source would count
//! the whole world as one client. [`Forwarded`] says how many proxies the operator runs in front of
//! the server; the client is then the address that many entries from the end of `X-Forwarded-For`,
//! which is the last one a proxy of ours wrote. Entries further left were written by the client or
//! by a proxy of someone else, and are not read. It is off by default, because on a server that is
//! reached directly the header is whatever the client sends.

use std::fmt;
use std::net::IpAddr;
use std::num::NonZeroU8;

use axum::http::HeaderMap;

/// How the client's address is found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Forwarded {
    /// The address of the connection. The default; headers are ignored.
    #[default]
    Off,
    /// `X-Forwarded-For`, with this many proxies of the operator's in front of the server. The
    /// connection is the last of them.
    Hops(NonZeroU8),
}

/// Why the header could not give a client. The request is refused, not served as if it came from
/// the proxy: a misplaced proxy setting is the operator's to see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// The setting is on and the request has no `X-Forwarded-For`.
    Absent,
    /// The header is not text, or has fewer entries than the proxies there should be.
    Short {
        /// The entries found.
        found: usize,
        /// The proxies trusted.
        trusted: u8,
    },
    /// The entry that names the client is not an address.
    Entry {
        /// Its place, counted from the end, starting at the number of trusted proxies.
        from_end: u8,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => out.write_str("the request has no X-Forwarded-For header and a proxy was trusted"),
            Self::Short { found, trusted } => {
                write!(out, "X-Forwarded-For has {found} entries and {trusted} proxies are trusted")
            }
            Self::Entry { from_end } => write!(out, "entry {from_end} from the end of X-Forwarded-For is no address"),
        }
    }
}

impl std::error::Error for Refused {}

impl Refused {
    /// The reply body: the diag code, what happened and what to do about it. It holds no part
    /// of the header, which a client wrote.
    pub fn body(self) -> Vec<u8> {
        let fix = "check that the proxy sets X-Forwarded-For and that the trusted hops count matches the proxies in front of the server";
        format!(r#"{{"problem":"auth.forwarded.refused","message":"{self}","causes":[],"fix":"{fix}"}}"#).into_bytes()
    }
}

impl Forwarded {
    /// The client of a request that came from `peer` with `headers`.
    pub fn client(self, peer: Option<IpAddr>, headers: &HeaderMap) -> Result<Option<IpAddr>, Refused> {
        let Self::Hops(trusted) = self else { return Ok(peer) };
        let lines: Vec<&str> =
            headers.get_all("x-forwarded-for").iter().filter_map(|line| line.to_str().ok()).collect();
        if lines.is_empty() {
            return Err(Refused::Absent);
        }
        let entries: Vec<&str> = lines.iter().flat_map(|line| line.split(',')).map(str::trim).collect();
        let trusted = trusted.get();
        let at =
            entries.len().checked_sub(usize::from(trusted)).ok_or(Refused::Short { found: entries.len(), trusted })?;
        let entry = entries.get(at).ok_or(Refused::Short { found: entries.len(), trusted })?;
        entry
            .parse::<IpAddr>()
            .map(|address| Some(address.to_canonical()))
            .map_err(|_| Refused::Entry { from_end: trusted })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
mod test {
    use super::*;

    fn headers(lines: &[&str]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for line in lines {
            map.append("x-forwarded-for", line.parse().unwrap());
        }
        map
    }

    fn hops(count: u8) -> Forwarded {
        Forwarded::Hops(NonZeroU8::new(count).unwrap())
    }

    #[test]
    fn it_is_off_by_default_and_then_the_header_is_ignored() {
        assert_eq!(Forwarded::default(), Forwarded::Off);
        let peer = Some("10.0.0.1".parse().unwrap());
        assert_eq!(Forwarded::Off.client(peer, &headers(&["1.2.3.4"])), Ok(peer));
    }

    #[test]
    fn the_client_is_the_entry_the_last_trusted_proxy_wrote() {
        // a client may send its own entries first: with one proxy only the last is ours
        let lines = headers(&["6.6.6.6, 7.7.7.7", "203.0.113.9"]);
        assert_eq!(hops(1).client(None, &lines), Ok(Some("203.0.113.9".parse().unwrap())));
        assert_eq!(hops(2).client(None, &lines), Ok(Some("7.7.7.7".parse().unwrap())));
        assert_eq!(hops(1).client(None, &headers(&["::ffff:1.2.3.4"])), Ok(Some("1.2.3.4".parse().unwrap())));
    }

    #[test]
    fn a_short_missing_or_garbled_header_is_refused_and_never_trusted_or_ignored() {
        assert_eq!(hops(1).client(None, &HeaderMap::new()), Err(Refused::Absent));
        assert_eq!(hops(3).client(None, &headers(&["1.1.1.1, 2.2.2.2"])), Err(Refused::Short { found: 2, trusted: 3 }));
        assert_eq!(hops(1).client(None, &headers(&["1.1.1.1, nobody"])), Err(Refused::Entry { from_end: 1 }));
        let refused = Refused::Short { found: 2, trusted: 3 }.body();
        let text = String::from_utf8(refused).unwrap();
        assert!(text.contains("auth.forwarded.refused") && !text.contains("1.1.1.1"));
    }
}
