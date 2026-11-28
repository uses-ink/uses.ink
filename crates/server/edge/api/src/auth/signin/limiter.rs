//! Rate limits on asking for a sign-in link, per address and per source, in memory.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use control::signin::{Ttl, TtlError};
use jiff::Timestamp;
use mail::Address;

/// At most `count` in each `window`, counted back from now (a sliding window).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    /// How many are let through.
    pub count: u32,
    /// How far back they are counted.
    pub window: Ttl,
}

/// Text that is no rate.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RateError {
    /// Not `<count>/<window>`, as `5/1h`.
    Shape {
        /// What was written.
        text: String,
    },
    /// The count is not a number of at least one.
    Count {
        /// What was written.
        text: String,
    },
    /// The window is no lifetime.
    Window(TtlError),
}

impl fmt::Display for RateError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape { text } => write!(out, "{text:?} is not a rate: write <count>/<window>, as 5/1h"),
            Self::Count { text } => write!(out, "{text:?} is not a count of at least one"),
            Self::Window(_) => out.write_str("the window of the rate is not a lifetime"),
        }
    }
}

impl std::error::Error for RateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Window(source) => Some(source),
            _ => None,
        }
    }
}

impl FromStr for Rate {
    type Err = RateError;

    fn from_str(text: &str) -> Result<Self, RateError> {
        let shown = || text.chars().take(40).collect::<String>();
        let (count, window) = text.split_once('/').ok_or_else(|| RateError::Shape { text: shown() })?;
        let count: u32 =
            count.parse().ok().filter(|count| *count >= 1).ok_or_else(|| RateError::Count { text: shown() })?;
        Ok(Self { count, window: window.parse().map_err(RateError::Window)? })
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}/{}", self.count, self.window)
    }
}

/// Every limit on asking for links. Each is the operator's to set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates {
    /// Requests for one address. Default 5 an hour.
    pub address: Rate,
    /// Requests from one source (an IPv4 address, or an IPv6 /64). Default 20 an hour.
    pub source: Rate,
    /// How many addresses and sources are remembered at once. Default 10 000.
    pub tracked: usize,
}

impl Default for Rates {
    fn default() -> Self {
        let hour = Ttl::secs(3600);
        Self { address: Rate { count: 5, window: hour }, source: Rate { count: 20, window: hour }, tracked: 10_000 }
    }
}

/// What was asked for a link, as far as a limit is concerned.
#[derive(Clone, Copy, Debug)]
pub struct Who<'a> {
    /// The address a link was asked for.
    pub address: &'a Address,
    /// Where the request came from.
    pub source: Option<IpAddr>,
}

/// Which limit was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Scope {
    /// Too many requests for this address.
    Address,
    /// Too many requests from this source.
    Source,
}

/// A request turned away, and when to try again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limited {
    /// Which limit.
    pub scope: Scope,
    /// How long until a request would be let through, rounded up to a second.
    pub retry: Duration,
}

impl fmt::Display for Limited {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "too many requests: try again in {} seconds", self.retry.as_secs())
    }
}

impl std::error::Error for Limited {}

/// What a count is kept for. An address is kept as its digest, so the limiter holds no address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key {
    Address([u8; 32]),
    V4(u32),
    V6(u64),
    /// Every request whose source is not known, as one bucket, so a missing address is not a way round.
    Unknown,
}

impl Key {
    fn of(address: &Address) -> Self {
        Self::Address(*blake3::hash(address.as_str().as_bytes()).as_bytes())
    }

    fn network(source: IpAddr) -> Self {
        match source {
            IpAddr::V4(ip) => Self::V4(u32::from(ip)),
            IpAddr::V6(ip) => Self::V6(u64::try_from(u128::from(ip) >> 64).unwrap_or_default()),
        }
    }
}

/// The times of the requests counted for each key, newest last, as milliseconds.
#[derive(Debug, Default)]
struct Counts(HashMap<Key, VecDeque<i64>>);

impl Counts {
    /// Makes room for the keys of `wanted` that are not held yet, within `tracked` keys: forgets
    /// those whose last request is older than `longest`, then, if that is not enough, the ones
    /// whose last request is oldest, so a flood of new sources forgets old counts (which only
    /// makes the limiter more generous) and never turns a new source away. Keys in `wanted` stay.
    fn make_room(&mut self, wanted: &[Key], (tracked, longest): (usize, Duration), now: i64) {
        let fresh = wanted.iter().filter(|key| !self.0.contains_key(key)).count();
        if self.0.len().saturating_add(fresh) <= tracked {
            return;
        }
        let cut = now.saturating_sub(i64::try_from(longest.as_millis()).unwrap_or(i64::MAX));
        self.0.retain(|key, hits| wanted.contains(key) || hits.back().is_some_and(|last| *last > cut));
        let mut extra = self.0.len().saturating_add(fresh).saturating_sub(tracked);
        if extra > 0 {
            let mut order: Vec<(i64, Key)> = self
                .0
                .iter()
                .filter(|(key, _)| !wanted.contains(key))
                .map(|(key, hits)| (hits.back().copied().unwrap_or(i64::MIN), *key))
                .collect();
            order.sort_unstable_by_key(|(last, _)| *last);
            for (_, key) in order {
                if extra == 0 {
                    break;
                }
                self.0.remove(&key);
                extra -= 1;
            }
        }
    }

    /// Whether `key` may make another request under `rate`, or how long until it may. The key
    /// is not counted: [`Counts::record`] does that once every check has passed.
    fn check(&mut self, key: Key, rate: Rate, now: i64) -> Result<(), Duration> {
        let window = i64::try_from(rate.window.duration().as_millis()).unwrap_or(i64::MAX);
        let hits = self.0.entry(key).or_default();
        while hits.front().is_some_and(|first| first.saturating_add(window) <= now) {
            hits.pop_front();
        }
        if hits.len() < usize::try_from(rate.count).unwrap_or(usize::MAX) {
            return Ok(());
        }
        let first = hits.front().copied().unwrap_or(now);
        let wait = first.saturating_add(window).saturating_sub(now).max(0);
        Err(Duration::from_secs(u64::try_from(wait).unwrap_or(0).div_ceil(1000).max(1)))
    }

    fn record(&mut self, key: Key, now: i64) {
        self.0.entry(key).or_default().push_back(now);
    }
}

/// Counts requests in memory, by address and by source, and turns away those over a limit. It
/// is told the time and reads no clock. A request that is turned away is not counted, and
/// nothing is stored for an address except its digest. What it knows is lost on a restart, which
/// only makes it more generous once.
#[derive(Debug, Default)]
pub struct Limiter {
    seen: Mutex<Counts>,
}

impl Limiter {
    /// A limiter that has seen nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lets the request through and counts it, or says which limit stops it.
    pub fn admit(&self, rates: &Rates, who: Who<'_>, now: Timestamp) -> Result<(), Limited> {
        let now = now.as_millisecond();
        let mut checks = vec![(Key::of(who.address), rates.address, Scope::Address)];
        checks.extend(who.source.map(|source| (Key::network(source), rates.source, Scope::Source)));
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        let wanted: Vec<Key> = checks.iter().map(|(key, ..)| *key).collect();
        let longest = rates.address.window.duration().max(rates.source.window.duration());
        seen.make_room(&wanted, (rates.tracked, longest), now);
        for (key, rate, scope) in &checks {
            seen.check(*key, *rate, now).map_err(|retry| Limited { scope: *scope, retry })?;
        }
        for (key, ..) in &checks {
            seen.record(*key, now);
        }
        Ok(())
    }
}

/// One limit on one kind of request per source, with the same sources as [`Limiter`] (an IPv4
/// address, an IPv6 /64) and a bucket of its own for requests whose source is not known. The
/// passkey challenge uses it: it takes no address, so only the source can be counted. Behind a
/// reverse proxy every request has the proxy's source until the server is told which forwarded
/// header to trust, and then the limit is one for everybody.
#[derive(Debug)]
pub struct Quota {
    rate: Rate,
    tracked: usize,
    seen: Mutex<Counts>,
}

impl Quota {
    /// What starting a passkey sign-in is held to by default: 60 an hour from one source. A
    /// challenge costs a row and names no one, so the limit bounds rows and noise; it is not a
    /// guard against guessing.
    pub const CHALLENGES: Rate = Rate { count: 60, window: Ttl::secs(3600) };

    /// What spending a recovery code is held to by default: ten an hour from one source. A code is
    /// 128 random bits, so the limit is not what stops a guess; it bounds the work a stranger can
    /// make the server do.
    pub const RECOVERIES: Rate = Rate { count: 10, window: Ttl::secs(3600) };

    /// A quota of `rate` for each source, remembering at most `tracked` sources.
    pub fn new(rate: Rate, tracked: usize) -> Self {
        Self { rate, tracked, seen: Mutex::default() }
    }

    /// The rate.
    pub const fn rate(&self) -> Rate {
        self.rate
    }

    /// Lets the request through and counts it, or says when it would be let through. A turned away
    /// request is not counted.
    pub fn admit(&self, source: Option<IpAddr>, now: Timestamp) -> Result<(), Limited> {
        let now = now.as_millisecond();
        let key = source.map_or(Key::Unknown, Key::network);
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.make_room(&[key], (self.tracked, self.rate.window.duration()), now);
        seen.check(key, self.rate, now).map_err(|retry| Limited { scope: Scope::Source, retry })?;
        seen.record(key, now);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_second(1_800_000_000 + seconds).unwrap()
    }

    fn rates(address: &str, source: &str) -> Rates {
        Rates { address: address.parse().unwrap(), source: source.parse().unwrap(), tracked: 100 }
    }

    fn who<'a>(address: &'a Address, source: &str) -> Who<'a> {
        Who { address, source: Some(source.parse().unwrap()) }
    }

    #[test]
    fn rates_read_as_a_count_and_a_window() {
        assert_eq!("5/1h".parse::<Rate>().unwrap().to_string(), "5/1h");
        for text in ["5", "0/1h", "x/1h", "5/1w", "/1h"] {
            assert!(text.parse::<Rate>().is_err(), "{text}");
        }
    }

    #[test]
    fn an_address_is_limited_and_freed_by_the_clock() {
        let limiter = Limiter::new();
        let rates = rates("2/10m", "100/10m");
        let me: Address = "me@example.org".parse().unwrap();
        assert!(limiter.admit(&rates, who(&me, "10.0.0.1"), at(0)).is_ok());
        assert!(limiter.admit(&rates, who(&me, "10.0.0.2"), at(60)).is_ok());
        let over = limiter.admit(&rates, who(&me, "10.0.0.3"), at(120)).unwrap_err();
        assert_eq!(over, Limited { scope: Scope::Address, retry: Duration::from_mins(8) });
        // turned away, so not counted: the first hit frees a place at 10 minutes
        assert!(limiter.admit(&rates, who(&me, "10.0.0.3"), at(600)).is_ok());
        let other: Address = "you@example.org".parse().unwrap();
        assert!(limiter.admit(&rates, who(&other, "10.0.0.3"), at(601)).is_ok());
    }

    #[test]
    fn a_source_is_limited_across_addresses() {
        let limiter = Limiter::new();
        let rates = rates("100/1h", "2/1h");
        for (n, name) in ["a", "b"].iter().enumerate() {
            let address: Address = format!("{name}@example.org").parse().unwrap();
            assert!(limiter.admit(&rates, who(&address, "10.0.0.1"), at(i64::try_from(n).unwrap())).is_ok());
        }
        let third: Address = "c@example.org".parse().unwrap();
        let over = limiter.admit(&rates, who(&third, "10.0.0.1"), at(5)).unwrap_err();
        assert_eq!(over.scope, Scope::Source);
        assert!(limiter.admit(&rates, who(&third, "10.0.0.9"), at(5)).is_ok());
    }

    #[test]
    fn an_ipv6_source_is_its_64_bit_prefix() {
        let limiter = Limiter::new();
        let rates = rates("100/1h", "1/1h");
        let me: Address = "me@example.org".parse().unwrap();
        assert!(limiter.admit(&rates, who(&me, "2001:db8:1:2::1"), at(0)).is_ok());
        assert!(limiter.admit(&rates, who(&me, "2001:db8:1:2:ffff::9"), at(1)).is_err());
        assert!(limiter.admit(&rates, who(&me, "2001:db8:1:3::1"), at(1)).is_ok());
    }

    #[test]
    fn a_full_table_forgets_the_oldest_and_turns_no_one_away() {
        let limiter = Limiter::new();
        let rates = Rates { tracked: 4, ..rates("5/1h", "5/1h") };
        for n in 0..20 {
            let address: Address = format!("a{n}@example.org").parse().unwrap();
            assert!(limiter.admit(&rates, who(&address, &format!("10.0.0.{n}")), at(n)).is_ok(), "{n}");
            assert!(limiter.seen.lock().unwrap().0.len() <= 4);
        }
        let late: Address = "late@example.org".parse().unwrap();
        assert!(limiter.admit(&rates, who(&late, "10.0.1.1"), at(30)).is_ok());
    }

    #[test]
    fn a_quota_counts_a_source_and_one_bucket_for_no_source() {
        let quota = Quota::new("2/10m".parse().unwrap(), 100);
        let one: Option<IpAddr> = Some("10.0.0.1".parse().unwrap());
        assert!(quota.admit(one, at(0)).is_ok());
        assert!(quota.admit(one, at(1)).is_ok());
        let over = quota.admit(one, at(2)).unwrap_err();
        assert_eq!(over, Limited { scope: Scope::Source, retry: Duration::from_secs(598) });
        assert!(quota.admit(Some("10.0.0.2".parse().unwrap()), at(2)).is_ok());
        // a request with no known source is one bucket, not a way round
        assert!(quota.admit(None, at(3)).is_ok());
        assert!(quota.admit(None, at(4)).is_ok());
        assert_eq!(quota.admit(None, at(5)).unwrap_err().scope, Scope::Source);
        // the window frees it
        assert!(quota.admit(one, at(601)).is_ok());
    }

    #[test]
    fn a_quota_remembers_a_bounded_number_of_sources_and_turns_no_new_one_away() {
        let quota = Quota::new("5/1h".parse().unwrap(), 2);
        for n in 1..=50 {
            assert!(quota.admit(Some(format!("10.0.0.{n}").parse().unwrap()), at(n)).is_ok(), "{n}");
            assert!(quota.seen.lock().unwrap().0.len() <= 2);
        }
    }
}
