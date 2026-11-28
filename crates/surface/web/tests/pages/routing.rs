//! The dashboard's routing, redirects and stream over a fake of what the edge provides.

use super::Fixtures;
use super::structure::Structure;
use build::{BuildId, Event};
use control::{AccountKind, Handle, Id};
use uses_ink_web::view::{Account, Deploys, Live, Serving, Settings, Site, Understood, Why};
use uses_ink_web::{
    Action, Asked, Body, Dashboard, Download, Failure, Follow, Form, Logged, Method, Only, Reply, Request, Screens,
    Status, Token,
};

/// What the edge provides, over fixtures.
struct Fake;

struct Replay(std::vec::IntoIter<Event>);

impl Follow for Replay {
    fn next(&mut self) -> impl Future<Output = Result<Option<Event>, Failure>> {
        std::future::ready(Ok(self.0.next()))
    }
}

impl Screens for Fake {
    type Follow = Replay;

    fn limits(&self) -> control::Limits {
        control::Limits::DEFAULT
    }
    fn site(&self, _: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Site, Failure>> {
        std::future::ready(Ok(Site::new(handle.clone(), Fixtures::address(), Serving::Nothing)))
    }

    fn understood(
        &self,
        _: Id<AccountKind>,
        handle: &Handle,
        _: Only,
    ) -> impl Future<Output = Result<Understood, Failure>> {
        std::future::ready(Ok(Understood::new(handle.clone(), Fixtures::source(), vec![])))
    }

    fn why(&self, _: Id<AccountKind>, _: &Handle, _: &Asked) -> impl Future<Output = Result<Why, Failure>> {
        std::future::ready(Ok(Fixtures::why()))
    }

    fn deploys(&self, _: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Deploys, Failure>> {
        std::future::ready(Ok(Deploys::new(handle.clone(), Serving::Nothing, vec![])))
    }

    fn settings(&self, _: Id<AccountKind>, _: &Handle) -> impl Future<Output = Result<Settings, Failure>> {
        std::future::ready(Err(Failure::NoSite { handle: Fixtures::handle() }))
    }

    fn end_session(
        &self,
        _: Id<AccountKind>,
        id: Id<control::SessionKind>,
    ) -> impl Future<Output = Result<(), Failure>> {
        std::future::ready(if id.to_string() == "0a".repeat(16) {
            Ok(())
        } else {
            Err(Failure::Unrouted { source: uses_ink_web::Unrouted::Path })
        })
    }

    fn account(&self, _: Id<AccountKind>) -> impl Future<Output = Result<Account, Failure>> {
        std::future::ready(Ok(Account::new(Fixtures::at("2026-11-01T12:00:00Z"), vec![])))
    }

    fn ask(&self, address: &str) -> impl Future<Output = Result<(), Failure>> {
        std::future::ready(if address.contains('@') {
            Ok(())
        } else {
            Err(Failure::Auth {
                status: Status::BadRequest,
                problem: "body".to_owned(),
                source: uses_ink_web::Cause::of(&std::io::Error::other("not an address")),
            })
        })
    }

    fn enter(&self, token: &Token, _: Option<&str>) -> impl Future<Output = Result<(), Failure>> {
        std::future::ready(if token.as_str() == "good" {
            Ok(())
        } else {
            Err(Failure::Auth {
                status: Status::NotFound,
                problem: "login".to_owned(),
                source: uses_ink_web::Cause::of(&std::io::Error::other("no such link")),
            })
        })
    }

    fn sign_out(&self, _: Id<AccountKind>) -> impl Future<Output = Result<(), Failure>> {
        std::future::ready(Ok(()))
    }

    fn log(
        &self,
        _: Id<AccountKind>,
        handle: &Handle,
        build: BuildId,
    ) -> impl Future<Output = Result<Logged<Replay>, Failure>> {
        std::future::ready(Ok(Logged::Running(
            Live::new(handle.clone(), build),
            Replay(Fixtures::events().into_iter()),
        )))
    }

    fn act(&self, _: Id<AccountKind>, _: &Handle, _: &Action) -> impl Future<Output = Result<(), Failure>> {
        std::future::ready(Ok(()))
    }

    fn export(&self, _: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Download, Failure>> {
        std::future::ready(Ok(Download { of: handle.clone(), bytes: vec![1, 2, 3] }))
    }
}

struct Ask;

impl Ask {
    fn get(path: &str) -> Request<'_> {
        Request { method: Method::Get, path, query: Form::default(), body: Form::default() }
    }

    fn post<'a>(path: &'a str, body: &str) -> Request<'a> {
        Request { method: Method::Post, path, query: Form::default(), body: Form::from(body) }
    }

    fn who() -> Id<AccountKind> {
        Id::from_bytes([9; 16])
    }
}

fn reply(request: &Request<'_>, who: Option<Id<AccountKind>>) -> Reply<Replay> {
    pollster::block_on(Dashboard(Fake).respond(who, request))
}

#[test]
fn a_form_is_answered_by_a_redirect_to_the_page_it_changed() {
    for (body_path, body, goes) in [
        ("/ada-notes/build", "", "/ada-notes/deploys"),
        ("/ada-notes/resume", "", "/ada-notes"),
        ("/ada-notes/rollback", &format!("deploy={}", "a".repeat(64)), "/ada-notes/deploys"),
        ("/ada-notes/remove", "confirm=ada-notes", "/account"),
    ] {
        let reply = reply(&Ask::post(body_path, body), Some(Ask::who()));
        assert_eq!(reply.status, Status::SeeOther, "{body_path}");
        assert_eq!(reply.location.unwrap().as_str(), goes, "{body_path}");
        assert!(matches!(reply.body, Body::Empty));
    }
}

#[test]
fn ending_a_session_is_a_post_that_goes_back_to_the_account() {
    let path = format!("/account/sessions/{}", "0a".repeat(16));
    let done = reply(&Ask::post(&path, ""), Some(Ask::who()));
    assert_eq!((done.status, done.location.unwrap().as_str().to_owned()), (Status::SeeOther, "/account".to_owned()));
    // a get changes nothing: it is refused
    assert_eq!(reply(&Ask::get(&path), Some(Ask::who())).status, Status::MethodNotAllowed);
    // an id that is no id, and one that names nothing, are an address of no page
    for bad in ["/account/sessions/nope".to_owned(), format!("/account/sessions/{}", "0b".repeat(16))] {
        assert_eq!(reply(&Ask::post(&bad, ""), Some(Ask::who())).status, Status::NotFound, "{bad}");
    }
    // and signed out is signed out
    assert_eq!(reply(&Ask::post(&path, ""), None).status, Status::Unauthorized);
}

#[test]
fn a_failure_is_a_page_with_its_code_and_status() {
    let cases = [
        (Ask::get("/ada-notes"), None, Status::Unauthorized, "account.session.missing"),
        (Ask::get("/a/b/c/d"), Some(Ask::who()), Status::NotFound, "page.route.unknown"),
        (Ask::get("/Not_A_Handle"), Some(Ask::who()), Status::NotFound, "page.route.unknown"),
        (Ask::get("/ada-notes/build"), Some(Ask::who()), Status::MethodNotAllowed, "page.method.refused"),
        (Ask::post("/ada-notes/rollback", ""), Some(Ask::who()), Status::BadRequest, "form.field.missing"),
        (Ask::post("/ada-notes/rollback", "deploy=zz"), Some(Ask::who()), Status::BadRequest, "form.field.invalid"),
        (Ask::get("/ada-notes/settings"), Some(Ask::who()), Status::NotFound, "site.handle.unknown"),
    ];
    for (request, who, status, code) in cases {
        let reply = reply(&request, who);
        assert_eq!(reply.status, status, "{code}");
        let Body::Page(page) = reply.body else { panic!("{code}: not a page") };
        assert!(page.contains(code), "{code}: the page names its code");
        assert!(page.contains("<dt>Fix</dt>"), "{code}: and its fix");
    }
}

#[test]
fn the_export_is_a_download_and_the_log_of_a_running_build_is_streamed() {
    let download = reply(&Ask::get("/ada-notes/export"), Some(Ask::who()));
    assert!(matches!(download.body, Body::Download(ref file) if file.to_string() == "ada-notes.export"));
    let build = "01".repeat(16);
    let Body::Streamed(mut stream) = reply(&Ask::get(&format!("/ada-notes/deploys/{build}")), Some(Ask::who())).body
    else {
        panic!("a running build streams")
    };
    let mut chunks = Vec::new();
    pollster::block_on(async {
        while let Some(chunk) = stream.chunk().await {
            chunks.push(chunk.unwrap());
        }
    });
    let whole = chunks.concat();
    assert_eq!(chunks.len(), 8, "head, six events, tail");
    assert!(chunks[0].starts_with("<!doctype html>") && chunks[0].contains("role=\"log\""));
    assert!(chunks[0].ends_with("aria-label=\"Build log\" tabindex=\"0\">"), "the log stays open across chunks");
    assert!(
        chunks[7].starts_with("</div>") && chunks[7].contains("Result: published"),
        "the last chunk holds the result"
    );
    assert!(whole.ends_with("</html>"));
    assert_eq!(Structure(&whole).tags("h1").len(), 1);
    assert!(!whole.contains("http-equiv"), "a stream does not also refresh");
    fixture::cases("fixture/stream", [("whole", chunks.join("\n--- chunk ---\n"))]);
}

#[test]
fn the_sign_in_page_and_the_script_need_no_session_and_nothing_else_is_scripted() {
    let page = reply(&Ask::get("/signin"), None);
    assert_eq!(page.status, Status::Ok);
    let Body::Page(text) = page.body else { panic!("a page") };
    assert!(text.contains(r#"data-passkey="login""#) && text.contains(r#"src="/passkey.js""#), "{text}");
    assert!(!text.contains("<script>"), "no inline script");
    let script = reply(&Ask::get("/passkey.js"), None);
    let Body::Script(source) = script.body else { panic!("a script") };
    assert!(source.contains("/v1/passkeys/challenge") && !source.contains("localStorage"));
    assert!(source.contains("auth.step-up.required"), "an action that needs a fresh passkey asks for one and retries");
    assert!(source.contains("/v1/recovery/codes"), "the script makes recovery codes");
    // the policy allows the one script and the fetches to this host, and no inline script
    let csp = Reply::<Replay>::HEADERS.iter().find(|(name, _)| *name == "content-security-policy").unwrap().1;
    assert!(csp.contains("script-src 'self'") && csp.contains("connect-src 'self'"), "{csp}");
    assert!(!csp.contains("script-src 'unsafe-inline'"));
    // a person's pages still need one
    assert_eq!(reply(&Ask::get("/account"), None).status, Status::Unauthorized);
}

#[test]
fn the_limits_page_needs_no_session() {
    let page = reply(&Ask::get("/limits"), None);
    assert_eq!(page.status, Status::Ok);
    let Body::Page(text) = page.body else { panic!("a page") };
    assert!(text.contains("<h1") && text.contains("Limits") && text.contains("1 GiB"), "{text}");
}

#[test]
fn the_home_page_sends_each_person_where_they_start() {
    let signed_in = reply(&Ask::get("/"), Some(Ask::who()));
    assert_eq!((signed_in.status, signed_in.location.unwrap().as_str().to_owned()), (Status::SeeOther, "/account".to_owned()));
    let signed_out = reply(&Ask::get("/"), None);
    assert_eq!((signed_out.status, signed_out.location.unwrap().as_str().to_owned()), (Status::SeeOther, "/signin".to_owned()));
}

#[test]
fn asking_for_a_link_and_using_one_need_no_session_and_are_posts() {
    let asked = reply(&Ask::post("/signin", "address=ann%40example.org"), None);
    assert_eq!((asked.status, asked.location.unwrap().as_str().to_owned()), (Status::SeeOther, "/signin/sent".to_owned()));
    assert_eq!(reply(&Ask::post("/signin", ""), None).status, Status::BadRequest, "no address, no ask");
    assert_eq!(reply(&Ask::post("/signin", "address=nobody"), None).status, Status::BadRequest);
    // opening a link only shows a page; a GET spends nothing and the page posts to the same path
    let page = reply(&Ask::get("/login/good"), None);
    let Body::Page(text) = page.body else { panic!("a page") };
    assert!(text.contains(r#"action="/login/good""#) && text.contains(r#"name="address""#), "{text}");
    let entered = reply(&Ask::post("/login/good", ""), None);
    assert_eq!((entered.status, entered.location.unwrap().as_str().to_owned()), (Status::SeeOther, "/account".to_owned()));
    assert_eq!(reply(&Ask::post("/login/bad", ""), None).status, Status::NotFound);
    assert_eq!(reply(&Ask::get("/login/a.b"), None).status, Status::NotFound, "a token has a shape");
    assert_eq!(reply(&Ask::get("/signin/sent"), None).status, Status::Ok);
}

#[test]
fn signing_out_is_a_post_that_needs_a_session() {
    let done = reply(&Ask::post("/logout", ""), Some(Ask::who()));
    assert_eq!((done.status, done.location.unwrap().as_str().to_owned()), (Status::SeeOther, "/signin".to_owned()));
    assert_eq!(reply(&Ask::get("/logout"), Some(Ask::who())).status, Status::MethodNotAllowed);
    assert_eq!(reply(&Ask::post("/logout", ""), None).status, Status::Unauthorized);
}
