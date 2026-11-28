//! The edge serves a deploy: conditional, ranged, compressed, moved, missing.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]
extern crate uses_ink_edge as edge;

mod support;

use edge::{Encoding, Ignored, Outcome, Ranging, Span};
use model::Hash;
use support::{PAGE_CSP, Site, answer, get};

const HOST: (&str, &str) = ("host", "notes.example");

fn tag(bytes: &[u8]) -> String {
    format!("\"{}\"", Hash::of(bytes))
}

#[test]
fn serves_a_page_with_its_headers_and_an_etag() {
    let edge = Site::new().edge();
    let (got, outcome) = answer(&edge, &get("GET", "/tea", &[HOST]));
    assert_eq!(got.status, 200);
    assert_eq!(got.text(), "<h1>tea</h1>");
    assert_eq!(got.field("content-type"), Some("text/html; charset=utf-8"));
    assert_eq!(got.field("content-length"), Some("12"));
    assert_eq!(got.field("etag"), Some(tag(b"<h1>tea</h1>").as_str()));
    assert_eq!(got.field("content-security-policy"), Some(PAGE_CSP), "the manifest's policy wins");
    assert_eq!(got.field("x-content-type-options"), Some("nosniff"), "the edge fills what the entry lacks");
    assert_eq!(got.field("cross-origin-resource-policy"), Some("same-origin"));
    assert_eq!(got.field("cache-control"), Some("public, max-age=0, must-revalidate"));
    assert_eq!(got.field("accept-ranges"), Some("bytes"));
    assert_eq!(got.field("strict-transport-security"), None, "plain HTTP");
    assert!(matches!(outcome, Outcome::Served { ranging: Ranging::Whole, encoding: None, .. }));
}

#[test]
fn serves_the_home_and_a_trailing_slash() {
    let edge = Site::new().edge();
    assert_eq!(answer(&edge, &get("GET", "/", &[HOST])).0.text(), "<h1>home</h1>");
    assert_eq!(answer(&edge, &get("GET", "/tea/?utm=x", &[HOST])).0.text(), "<h1>tea</h1>");
    assert_eq!(answer(&edge, &get("GET", "http://Notes.Example/tea", &[HOST])).0.status, 200);
    assert_eq!(answer(&edge, &get("GET", "/tea", &[("Host", "NOTES.example.:8080")])).0.status, 200);
}

#[test]
fn a_manifest_header_beats_the_default() {
    let edge = Site::new().edge();
    let (got, _) = answer(&edge, &get("GET", "/style.css", &[HOST]));
    assert_eq!(got.field("cache-control"), Some("public, max-age=31536000, immutable"));
    assert_eq!(got.field("content-type"), Some("text/css; charset=utf-8"));
}

#[test]
fn head_says_what_get_would_send() {
    let edge = Site::new().edge();
    let (get_, _) = answer(&edge, &get("GET", "/clip.png", &[HOST]));
    let (head, _) = answer(&edge, &get("HEAD", "/clip.png", &[HOST]));
    assert_eq!(head.body, b"");
    assert_eq!(head.status, 200);
    assert_eq!(head.field("content-length"), Some("1000"));
    for name in ["content-length", "content-type", "etag", "accept-ranges", "content-security-policy"] {
        assert_eq!(head.field(name), get_.field(name), "{name}");
    }
    let (head, _) = answer(&edge, &get("HEAD", "/clip.png", &[HOST, ("range", "bytes=0-9")]));
    assert_eq!(head.status, 200, "HEAD ignores a range");
    assert_eq!(head.field("content-length"), Some("1000"));
}

#[test]
fn answers_304_to_a_matching_tag() {
    let edge = Site::new().edge();
    let etag = tag(b"<h1>tea</h1>");
    for header in [etag.clone(), format!("W/{etag}"), format!("\"x\", {etag}"), "*".to_owned()] {
        let (got, outcome) = answer(&edge, &get("GET", "/tea", &[HOST, ("if-none-match", &header)]));
        assert_eq!(got.status, 304, "{header}");
        assert_eq!(got.body, b"");
        assert_eq!(got.field("etag"), Some(etag.as_str()));
        assert_eq!(got.field("x-content-type-options"), Some("nosniff"));
        assert!(matches!(outcome, Outcome::Unchanged { .. }));
    }
    let (got, _) = answer(&edge, &get("GET", "/tea", &[HOST, ("if-none-match", "\"other\"")]));
    assert_eq!(got.status, 200);
    let (got, _) = answer(&edge, &get("HEAD", "/tea", &[HOST, ("if-none-match", &etag)]));
    assert_eq!(got.status, 304);
}

#[test]
fn a_tag_never_matches_another_file() {
    let edge = Site::new().edge();
    let (got, _) = answer(&edge, &get("GET", "/", &[HOST, ("if-none-match", &tag(b"<h1>tea</h1>"))]));
    assert_eq!(got.status, 200);
}

#[test]
fn serves_a_range_of_a_media_file() {
    let edge = Site::new().edge();
    let media = support::media();
    let range = |header: &str| answer(&edge, &get("GET", "/clip.png", &[HOST, ("range", header)]));
    let (got, outcome) = range("bytes=10-19");
    assert_eq!(got.status, 206);
    assert_eq!(got.body, media[10..20]);
    assert_eq!(got.field("content-range"), Some("bytes 10-19/1000"));
    assert_eq!(got.field("content-length"), Some("10"));
    assert!(matches!(outcome, Outcome::Served { ranging: Ranging::Span(Span { start: 10, length: 10 }), .. }));
    let (got, _) = range("bytes=990-");
    assert_eq!((got.status, got.body), (206, media[990..].to_vec()));
    let (got, _) = range("bytes=-5");
    assert_eq!((got.status, got.field("content-range")), (206, Some("bytes 995-999/1000")));
    let (got, _) = range("bytes=900-5000");
    assert_eq!(got.field("content-range"), Some("bytes 900-999/1000"));
}

#[test]
fn says_when_a_range_is_past_the_end() {
    let edge = Site::new().edge();
    for header in ["bytes=1000-", "bytes=5000-6000", "bytes=-0"] {
        let (got, outcome) = answer(&edge, &get("GET", "/clip.png", &[HOST, ("range", header)]));
        assert_eq!(got.status, 416, "{header}");
        assert_eq!(got.field("content-range"), Some("bytes */1000"));
        assert!(matches!(outcome, Outcome::Refused(edge::Refusal::Unsatisfiable { length: 1000 })));
    }
    let (got, _) = answer(&edge, &get("GET", "/empty.png", &[HOST, ("range", "bytes=0-")]));
    assert_eq!(got.status, 416);
}

#[test]
fn follows_if_range_and_ignores_what_it_does_not_serve() {
    let edge = Site::new().edge();
    let etag = tag(&support::media());
    let (got, _) = answer(&edge, &get("GET", "/clip.png", &[HOST, ("range", "bytes=0-9"), ("if-range", &etag)]));
    assert_eq!(got.status, 206);
    let (got, outcome) =
        answer(&edge, &get("GET", "/clip.png", &[HOST, ("range", "bytes=0-9"), ("if-range", "\"stale\"")]));
    assert_eq!((got.status, got.body.len()), (200, 1000));
    assert!(matches!(outcome, Outcome::Served { ranging: Ranging::Ignored(Ignored::Validator), .. }));
    let (got, _) = answer(
        &edge,
        &get("GET", "/clip.png", &[HOST, ("range", "bytes=0-9"), ("if-range", "Wed, 21 Oct 2015 07:28:00 GMT")]),
    );
    assert_eq!(got.status, 200);
    let (got, outcome) = answer(&edge, &get("GET", "/clip.png", &[HOST, ("range", "bytes=0-1,5-6")]));
    assert_eq!(got.status, 200);
    assert!(matches!(outcome, Outcome::Served { ranging: Ranging::Ignored(Ignored::Multiple), .. }));
    let (got, outcome) = answer(&edge, &get("GET", "/clip.png", &[HOST, ("range", "pages=1-2")]));
    assert_eq!(got.status, 200);
    assert!(matches!(outcome, Outcome::Served { ranging: Ranging::Ignored(Ignored::Unit), .. }));
}

#[test]
fn serves_the_sites_404_page_with_404() {
    let edge = Site::new().edge();
    for target in ["/no/such/page", "/Tea", "/tea.html", "/%E2%82%AC"] {
        let (got, outcome) = answer(&edge, &get("GET", target, &[HOST]));
        assert_eq!(got.status, 404, "{target}");
        assert_eq!(got.text(), "<h1>nothing here</h1>");
        assert_eq!(got.field("content-security-policy"), Some(PAGE_CSP));
        assert_eq!(got.field("etag"), None);
        assert!(matches!(outcome, Outcome::Missing { .. }));
    }
    let (got, _) = answer(&edge, &get("GET", "/404", &[HOST]));
    assert_eq!(got.status, 404, "the page itself is a 404 too");
    let (got, _) = answer(&edge, &get("HEAD", "/nope", &[HOST]));
    assert_eq!((got.status, got.body.len(), got.field("content-length")), (404, 0, Some("21")));
}

#[test]
fn a_site_without_a_404_page_gets_a_line_of_text() {
    let edge = Site::new().edge();
    let (got, _) = answer(&edge, &get("GET", "/nope", &[("host", "bare.example")]));
    assert_eq!((got.status, got.text().as_str()), (404, "not found"));
    assert_eq!(got.field("content-security-policy"), Some("default-src 'none'; frame-ancestors 'none'; sandbox"));
    assert_eq!(got.field("x-content-type-options"), Some("nosniff"));
}

#[test]
fn redirects_a_moved_route_with_an_ascii_location() {
    let edge = Site::new().edge();
    let (got, outcome) = answer(&edge, &get("GET", "/old", &[HOST]));
    assert_eq!(got.status, 301);
    assert_eq!(got.field("location"), Some("/th%C3%A9"));
    assert_eq!(got.body, b"");
    assert!(matches!(outcome, Outcome::Moved { .. }));
    assert_eq!(answer(&edge, &get("GET", "/old/", &[HOST])).0.status, 301);
    let (got, _) = answer(&edge, &get("GET", "/old", &[("host", "bare.example")]));
    assert_eq!(got.status, 404, "a move belongs to its deploy");
}

#[test]
fn serves_a_variant_only_when_the_deploy_has_it() {
    let edge = Site::new().edge();
    let css = |accept: &'static str| {
        let headers = [HOST, ("accept-encoding", accept)];
        answer(&edge, &get("GET", "/style.css", &headers))
    };
    let (got, outcome) = css("gzip, br");
    assert_eq!((got.status, got.text().as_str()), (200, "br bytes"));
    assert_eq!(got.field("content-encoding"), Some("br"));
    assert_eq!(got.field("vary"), Some("accept-encoding"));
    assert_eq!(got.field("accept-ranges"), None);
    assert_eq!(got.field("etag"), Some(format!("\"{}-br\"", Hash::of(b"body { color: black }")).as_str()));
    assert!(matches!(outcome, Outcome::Served { encoding: Some(Encoding::Brotli), .. }));
    assert_eq!(css("gzip").0.text(), "gzip bytes");
    let (got, _) = css("identity");
    assert_eq!(
        (got.text().as_str(), got.field("content-encoding"), got.field("vary")),
        ("body { color: black }", None, Some("accept-encoding"))
    );
    let (got, _) = answer(&edge, &get("GET", "/style.css", &[HOST, ("accept-encoding", "br"), ("range", "bytes=0-3")]));
    assert_eq!((got.status, got.text().as_str(), got.field("content-encoding")), (206, "body", None));
    let (got, _) = answer(&edge, &get("GET", "/tea", &[HOST, ("accept-encoding", "br")]));
    assert_eq!((got.field("content-encoding"), got.field("vary")), (None, None), "no variant, no promise");
    let br = format!("\"{}-br\"", Hash::of(b"body { color: black }"));
    let (got, _) = answer(&edge, &get("GET", "/style.css", &[HOST, ("accept-encoding", "br"), ("if-none-match", &br)]));
    assert_eq!(got.status, 304);
}

#[test]
fn sends_hsts_over_tls_only() {
    let edge = Site::new().edge().with_hsts(edge::Hsts::default()).unwrap();
    let mut request = edge::Request::new("GET", "/tea", edge::Scheme::Tls);
    request.extend([(b"host".as_slice(), b"notes.example".as_slice())]);
    let (got, _) = support::block(async { support::read(edge.respond(&request).await).await });
    assert_eq!(got.field("strict-transport-security"), Some("max-age=31536000"));
    let mut refused = edge::Request::new("POST", "/", edge::Scheme::Tls);
    refused.extend([(b"host".as_slice(), b"notes.example".as_slice())]);
    let (got, _) = support::block(async { support::read(edge.respond(&refused).await).await });
    assert_eq!(got.field("strict-transport-security"), Some("max-age=31536000"), "refusals too");
    assert_eq!(answer(&edge, &get("GET", "/tea", &[HOST])).0.field("strict-transport-security"), None);
}

#[test]
fn probes_by_address_and_never_hides_a_page() {
    let edge = Site::new().edge();
    let (got, outcome) = answer(&edge, &get("GET", "/healthz", &[("host", "10.0.0.7:8080")]));
    assert_eq!((got.status, got.text().as_str()), (200, "ok"));
    assert!(matches!(outcome, Outcome::Probed));
    assert_eq!(got.field("x-content-type-options"), Some("nosniff"));
    assert_eq!(answer(&edge, &get("GET", "/healthz", &[HOST])).0.status, 404, "a site's own route wins");
    let (got, outcome) = answer(&edge, &get("GET", "/tea", &[("host", "10.0.0.7")]));
    assert_eq!(got.status, 404);
    assert!(matches!(outcome, Outcome::Refused(edge::Refusal::Unserved { .. })));
}

#[test]
fn a_store_that_fails_is_a_500_that_shows_nothing() {
    let mut site = Site::new();
    site.store.0.clear();
    let edge = site.edge();
    let (got, outcome) = answer(&edge, &get("GET", "/tea", &[HOST]));
    assert_eq!(got.status, 500);
    assert_eq!(got.text(), "the edge could not serve this request");
    assert_eq!(got.field("x-content-type-options"), Some("nosniff"));
    assert!(matches!(outcome, Outcome::Failed(edge::Fault::Blob(edge::BlobError::Missing { .. }))));

    let mut site = Site::new();
    site.hosts.1 = true;
    let (got, outcome) = answer(&site.edge(), &get("GET", "/tea", &[HOST]));
    assert_eq!(got.status, 500);
    assert!(matches!(outcome, Outcome::Failed(edge::Fault::Sites(support::Broken))));
}

#[test]
fn refuses_to_serve_a_file_over_the_limit() {
    let site = Site::new();
    let limits = edge::Limits { file: model::Bytes(500), ..edge::Limits::default() };
    let edge = edge::Edge::new(site.hosts, site.store, limits).unwrap();
    let (got, outcome) = answer(&edge, &get("GET", "/clip.png", &[HOST]));
    assert_eq!(got.status, 500);
    assert!(matches!(outcome, Outcome::Failed(edge::Fault::Large { length: 1000, limit: 500, .. })));
    assert_eq!(answer(&edge, &get("GET", "/tea", &[HOST])).0.status, 200);
}
