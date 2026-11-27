//! Conformance: a table of hostile requests, each with the typed refusal the edge answers with.
//!
//! Every row names the request as it reaches the edge (the adapter has passed it on exactly as
//! received) and the `Refusal` it must be answered with. Every refusal is also checked for its
//! status, for never reaching a site, and for the headers every response carries.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]
extern crate uses_ink_edge as edge;

mod support;

use edge::{BodyError, ConditionError, HeaderError, HostnameError, Outcome, Refusal, Singular, TargetError};
use support::{Site, answer, block, get};

const HOST: (&str, &str) = ("host", "notes.example");

/// One hostile request and the refusal it gets.
struct Row {
    name: &'static str,
    method: &'static str,
    target: String,
    headers: Vec<(String, Vec<u8>)>,
    refusal: Refusal,
    status: u16,
}

impl Row {
    fn new(name: &'static str, target: &str, refusal: impl Into<Refusal>, status: u16) -> Self {
        let headers = vec![("host".to_owned(), b"notes.example".to_vec())];
        Self { name, method: "GET", target: target.to_owned(), headers, refusal: refusal.into(), status }
    }

    fn method(mut self, method: &'static str) -> Self {
        self.method = method;
        self
    }

    /// Replaces every header with these.
    fn headers(mut self, headers: &[(&str, &[u8])]) -> Self {
        self.headers = headers.iter().map(|(name, value)| ((*name).to_owned(), value.to_vec())).collect();
        self
    }

    /// Adds a header after `Host`.
    fn with(mut self, name: &str, value: &[u8]) -> Self {
        self.headers.push((name.to_owned(), value.to_vec()));
        self
    }
}

fn table() -> Vec<Row> {
    let long_target = format!("/{}", "a".repeat(5000));
    let deep = "/a".repeat(40);
    let big_value = vec![b'a'; 9000];
    let mut rows = vec![
        // path traversal, every spelling
        Row::new("dot dot", "/../etc/passwd", TargetError::Dot { index: 0 }, 400),
        Row::new("dot dot inside", "/a/../../b", TargetError::Dot { index: 1 }, 400),
        Row::new("single dot", "/a/./b", TargetError::Dot { index: 1 }, 400),
        Row::new("escaped dots", "/%2e%2e/secret", TargetError::Dot { index: 0 }, 400),
        Row::new("mixed escaped dots", "/.%2E/secret", TargetError::Dot { index: 0 }, 400),
        Row::new("dot dot then escaped slash", "/..%2fsecret", TargetError::Separator { at: 3 }, 400),
        Row::new("escaped dots and slash", "/%2e%2e%2fsecret", TargetError::Separator { at: 7 }, 400),
        Row::new("escaped backslash", "/a%5Cb", TargetError::Separator { at: 2 }, 400),
        Row::new("backslash", "/a\\b", TargetError::Byte { at: 2, found: b'\\' }, 400),
        Row::new("double encoded dots", "/%252e%252e/", TargetError::Double { at: 1 }, 400),
        Row::new("double encoded slash", "/a%252fb", TargetError::Double { at: 1 }, 400),
        Row::new("overlong utf-8 dot", "/%C0%AE%C0%AE/", TargetError::Encoding, 400),
        Row::new("lone continuation byte", "/%80", TargetError::Encoding, 400),
        Row::new("escaped nul", "/a%00b", TargetError::Control { at: 2 }, 400),
        Row::new("escaped newline", "/a%0Ab", TargetError::Control { at: 2 }, 400),
        Row::new("escaped delete", "/a%7f", TargetError::Control { at: 2 }, 400),
        Row::new("raw nul", "/a\0b", TargetError::Byte { at: 2, found: 0 }, 400),
        Row::new("raw newline", "/a\nb", TargetError::Byte { at: 2, found: b'\n' }, 400),
        Row::new("raw space", "/a b", TargetError::Byte { at: 2, found: b' ' }, 400),
        Row::new("raw tab", "/a\tb", TargetError::Byte { at: 2, found: b'\t' }, 400),
        Row::new("raw non-ascii", "/thé", TargetError::Byte { at: 3, found: 0xc3 }, 400),
        Row::new("fragment", "/a#b", TargetError::Byte { at: 2, found: b'#' }, 400),
        Row::new("truncated escape", "/%", TargetError::Escape { at: 1 }, 400),
        Row::new("non-hex escape", "/%zz", TargetError::Escape { at: 1 }, 400),
        Row::new("short escape", "/a%4", TargetError::Escape { at: 2 }, 400),
        // open redirects and shapes that name no path
        Row::new("protocol-relative", "//evil.com/", TargetError::Empty { index: 0 }, 400),
        Row::new("empty segment", "/a//b", TargetError::Empty { index: 1 }, 400),
        Row::new("only slashes", "///", TargetError::Empty { index: 0 }, 400),
        Row::new("asterisk", "*", TargetError::Form, 400),
        Row::new("no leading slash", "a/b", TargetError::Form, 400),
        Row::new("empty target", "", TargetError::Form, 400),
        Row::new("other scheme", "ftp://notes.example/", TargetError::Form, 400),
        Row::new("authority form", "notes.example:443", TargetError::Form, 400),
        // limits
        Row::new("overlong target", &long_target, TargetError::TooLong { length: 5001, limit: 4096 }, 414),
        Row::new("too many segments", &deep, TargetError::Deep { count: 40, limit: 32 }, 400),
        // host confusion
        Row::new(
            "absolute form for another host",
            "http://evil.example/",
            Refusal::Authority { header: "notes.example".parse_hostname(), target: "evil.example".parse_hostname() },
            400,
        ),
        Row::new(
            "userinfo in the target",
            "http://notes.example@evil.example/",
            TargetError::Authority(HostnameError::Character { at: 13, found: b'@' }),
            400,
        ),
        Row::new("no host header", "/", HostnameError::Missing, 400).headers(&[]),
        Row::new("empty host header", "/", HostnameError::Missing, 400).headers(&[("host", b"")]),
        Row::new("two host headers", "/", HeaderError::Repeated { name: Singular::Host }, 400)
            .with("host", b"evil.example"),
        Row::new("host list", "/", HostnameError::Character { at: 13, found: b',' }, 400)
            .headers(&[("host", b"notes.example,evil.example")]),
        Row::new("host with path", "/", HostnameError::Character { at: 13, found: b'/' }, 400)
            .headers(&[("host", b"notes.example/..")]),
        Row::new("host with userinfo", "/", HostnameError::Character { at: 4, found: b'@' }, 400)
            .headers(&[("host", b"evil@notes.example")]),
        Row::new("host with space", "/", HostnameError::Character { at: 5, found: b' ' }, 400)
            .headers(&[("host", b"notes example")]),
        Row::new("host dot dot", "/", HostnameError::Label { index: 0 }, 400).headers(&[("host", b"..")]),
        Row::new("host with empty label", "/", HostnameError::Label { index: 1 }, 400).headers(&[("host", b"a..b")]),
        Row::new("host with unicode", "/", HostnameError::Character { at: 0, found: 0xc3 }, 400)
            .headers(&[("host", "é.example".as_bytes())]),
        Row::new("host port too big", "/", HostnameError::Port { at: 14 }, 400)
            .headers(&[("host", b"notes.example:99999")]),
        Row::new("host empty port", "/", HostnameError::Port { at: 14 }, 400).headers(&[("host", b"notes.example:")]),
        Row::new("host broken literal", "/", HostnameError::Literal, 400).headers(&[("host", b"[::1")]),
        Row::new("host too long", "/", HostnameError::TooLong { length: 300, limit: 259 }, 400)
            .headers(&[("host", "a".repeat(300).as_bytes())]),
        Row::new("host with line break", "/", HeaderError::Byte { index: 0, at: 13 }, 400)
            .headers(&[("host", b"notes.example\r\nx: y")]),
        Row::new("host with nul", "/", HeaderError::Byte { index: 0, at: 5 }, 400)
            .headers(&[("host", b"notes\0.example")]),
        // methods
        Row::new("post", "/", Refusal::Method { method: "POST".to_owned() }, 405).method("POST"),
        Row::new("lower-case get", "/", Refusal::Method { method: "get".to_owned() }, 405).method("get"),
        Row::new("options", "/", Refusal::Method { method: "OPTIONS".to_owned() }, 405).method("OPTIONS"),
        Row::new("connect", "/", Refusal::Method { method: "CONNECT".to_owned() }, 405).method("CONNECT"),
        Row::new("trace", "/", Refusal::Method { method: "TRACE".to_owned() }, 405).method("TRACE"),
        Row::new("method with a line break", "/", Refusal::Method { method: "GET\r\nHost: x".to_owned() }, 405)
            .method("GET\r\nHost: x"),
        // request smuggling shapes
        Row::new("body on get", "/", BodyError::Declared { length: 5 }, 400).with("content-length", b"5"),
        Row::new("two lengths", "/", BodyError::Declared { length: 5 }, 400)
            .with("content-length", b"0")
            .with("content-length", b"5"),
        Row::new("signed length", "/", BodyError::Invalid, 400).with("content-length", b"+5"),
        Row::new("list of lengths", "/", BodyError::Invalid, 400).with("content-length", b"5, 5"),
        Row::new("empty length", "/", BodyError::Invalid, 400).with("content-length", b""),
        Row::new("chunked", "/", BodyError::Chunked, 400).with("transfer-encoding", b"chunked"),
        Row::new("obfuscated chunked", "/", BodyError::Chunked, 400).with("Transfer-Encoding", b"\tchunked"),
        Row::new("length and chunked", "/", BodyError::Framing, 400)
            .with("content-length", b"0")
            .with("transfer-encoding", b"chunked"),
        // header limits and line breaks
        Row::new("header name with a space", "/", HeaderError::Name { index: 1 }, 400).with("x y", b"1"),
        Row::new("empty header name", "/", HeaderError::Name { index: 1 }, 400).with("", b"1"),
        Row::new("header name with a colon", "/", HeaderError::Name { index: 1 }, 400).with("x:y", b"1"),
        Row::new("line break in a value", "/", HeaderError::Byte { index: 1, at: 1 }, 400)
            .with("x", b"a\r\nhost: evil.example"),
        Row::new("nul in a value", "/", HeaderError::Byte { index: 1, at: 1 }, 400).with("x", b"a\0b"),
        Row::new("huge value", "/", HeaderError::Size { index: 1, length: 9000, limit: 8192 }, 431)
            .with("x", &big_value),
        // conditions
        Row::new("junk if-none-match", "/", ConditionError::IfNoneMatch { at: 0 }, 400).with("if-none-match", b"junk"),
        Row::new("unterminated tag", "/", ConditionError::IfNoneMatch { at: 3 }, 400).with("if-none-match", b"\"ab"),
        Row::new("star in a list", "/", ConditionError::IfNoneMatch { at: 1 }, 400).with("if-none-match", b"*, \"a\""),
        Row::new("two if-none-match", "/", HeaderError::Repeated { name: Singular::IfNoneMatch }, 400)
            .with("if-none-match", b"\"a\"")
            .with("if-none-match", b"\"b\""),
        Row::new("range not numbers", "/", ConditionError::Range { at: 6 }, 400).with("range", b"bytes=a-b"),
        Row::new("range backwards", "/", ConditionError::Range { at: 6 }, 400).with("range", b"bytes=5-1"),
        Row::new("range past u64", "/", ConditionError::Range { at: 6 }, 400)
            .with("range", b"bytes=18446744073709551616-"),
        Row::new("range without equals", "/", ConditionError::Range { at: 0 }, 400).with("range", b"bytes"),
        Row::new("range with trailing comma", "/", ConditionError::Range { at: 10 }, 400).with("range", b"bytes=0-1,"),
        Row::new("two ranges headers", "/", HeaderError::Repeated { name: Singular::Range }, 400)
            .with("range", b"bytes=0-1")
            .with("range", b"bytes=2-3"),
        Row::new("bad if-range", "/", ConditionError::IfRange { at: 0 }, 400).with("if-range", b""),
        Row::new("bad accept-encoding weight", "/", ConditionError::AcceptEncoding { at: 0 }, 400)
            .with("accept-encoding", b"gzip;q=9"),
        Row::new("bad accept-encoding token", "/", ConditionError::AcceptEncoding { at: 5 }, 400)
            .with("accept-encoding", b"gzip,g zip"),
        // a range past the end, and a host that is no site
        Row::new("range past the end", "/clip.png", Refusal::Unsatisfiable { length: 1000 }, 416)
            .with("range", b"bytes=1000-"),
        Row::new("unknown host", "/", Refusal::Unserved { hostname: "other.example".parse_hostname() }, 404)
            .headers(&[("host", b"Other.Example")]),
    ];
    let many: Vec<_> = (0..64).map(|n| (format!("x{n}"), b"1".to_vec())).collect();
    let mut flooded = Row::new("too many headers", "/", HeaderError::Count { count: 65, limit: 64 }, 431);
    flooded.headers.extend(many);
    rows.push(flooded);
    let mut heavy =
        Row::new("headers too large together", "/", HeaderError::Total { length: 35_027, limit: 32_768 }, 431);
    heavy.headers.extend((0..5).map(|n| (format!("x{n}"), vec![b'a'; 7000])));
    rows.push(heavy);
    let tags = (0..17).map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(",");
    rows.push(
        Row::new("too many tags", "/", ConditionError::Tags { count: 17, limit: 16 }, 400)
            .with("if-none-match", tags.as_bytes()),
    );
    rows
}

trait ParseHostname {
    fn parse_hostname(&self) -> edge::Hostname;
}

impl ParseHostname for str {
    fn parse_hostname(&self) -> edge::Hostname {
        edge::Authority::try_from(self.as_bytes()).unwrap().hostname
    }
}

#[test]
fn every_hostile_request_gets_its_typed_refusal() {
    let edge = Site::new().edge();
    let mut wrong = Vec::new();
    for row in table() {
        let mut request = edge::Request::new(row.method, &row.target, edge::Scheme::Plain);
        request.extend(row.headers.iter().map(|(name, value)| (name.as_bytes(), value.as_slice())));
        let (got, outcome) = answer(&edge, &request);
        let Outcome::Refused(refusal) = outcome else {
            wrong.push(format!("{}: not refused: {outcome:?}", row.name));
            continue;
        };
        let hardened = got.field("x-content-type-options") == Some("nosniff")
            && got.field("content-security-policy").is_some_and(|csp| csp.contains("default-src 'none'"))
            && got.field("cache-control") == Some("no-store")
            && !got.text().contains("<h1>");
        if refusal != row.refusal || got.status != row.status || refusal.status().code() != row.status || !hardened {
            wrong.push(format!(
                "{}: got {refusal:?} with {}, wanted {:?} with {}",
                row.name, got.status, row.refusal, row.status
            ));
        }
    }
    assert!(wrong.is_empty(), "{} rows wrong:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn a_refusal_says_why_in_text() {
    let edge = Site::new().edge();
    let (got, _) = answer(&edge, &get("GET", "/a/../b", &[HOST]));
    assert_eq!(got.text(), "the request target is refused: segment 2 of the path is a dot segment");
    assert_eq!(got.field("content-type"), Some("text/plain; charset=utf-8"));
    assert_eq!(got.field("content-length"), Some(got.body.len().to_string().as_str()));
}

#[test]
fn a_method_refusal_lists_what_is_allowed() {
    let edge = Site::new().edge();
    let (got, _) = answer(&edge, &get("DELETE", "/tea", &[HOST]));
    assert_eq!((got.status, got.field("allow")), (405, Some("GET, HEAD")));
}

#[test]
fn a_refused_request_never_reaches_the_sites() {
    let mut site = Site::new();
    site.hosts.1 = true;
    let edge = site.edge();
    let (got, outcome) = answer(&edge, &get("GET", "/../x", &[HOST]));
    assert_eq!(got.status, 400, "refused before the broken site lookup");
    assert!(matches!(outcome, Outcome::Refused(_)));
}

#[test]
fn the_same_table_holds_for_head() {
    let edge = Site::new().edge();
    for row in table().into_iter().filter(|row| row.method == "GET" && row.name != "range past the end") {
        let mut request = edge::Request::new("HEAD", &row.target, edge::Scheme::Plain);
        request.extend(row.headers.iter().map(|(name, value)| (name.as_bytes(), value.as_slice())));
        let (got, outcome) = block(async { support::read(edge.respond(&request).await).await });
        let Outcome::Refused(refusal) = outcome else { panic!("{}: not refused", row.name) };
        assert_eq!(refusal, row.refusal, "{}", row.name);
        assert_eq!(got.status, row.status, "{}", row.name);
    }
}
