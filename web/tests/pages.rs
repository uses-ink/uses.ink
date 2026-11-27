//! Every page rendered from fixture view models and compared with its golden, a structural check
//! over all of them (one `h1`, every control labelled, every form named), and the dashboard's
//! routing, redirects and stream over a fake of what the edge provides.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
#![allow(missing_docs, reason = "a test file")]

use std::time::Duration;

use build::{BuildId, Event, Status as Ended, Trigger};
use control::{AccountKind, Handle, Id, JobState, Purpose, Refusal, SiteState};
use engine::{Standing, Witnessed};
use jiff::Timestamp;
use model::code::{Date, Home, Ignored, Title};
use model::decision::{File, Site as Whole};
use model::{
    AnyDecision, Bytes, ContentPath, Decision, DefaultReason, DeployId, Diagnostic, Evidence, ExternalUrl, Problem,
    Span, Subject,
};
use uses_ink_web::view::{
    Account, Attempt, Build, Deploy, Deploys, Ended as How, Failed, Heard, Held, Live, Outcome, Owned, Pointer, Rung,
    Serving, Settings, Site, Understood, Why,
};
use uses_ink_web::{
    Action, Asked, Body, Cause, Dashboard, Document, Download, Failure, Field, FieldError, Follow, Form, Frame, Logged,
    Method, Only, Reply, Request, Screen, Screens, Status, View,
};

/// The pages made so far, by name.
struct Pages(Vec<(String, String)>);

impl Pages {
    fn put(&mut self, name: &str, view: &dyn Pageable) {
        self.0.push((name.to_owned(), view.page()));
    }
}

struct Fixtures;

impl Fixtures {
    fn handle() -> Handle {
        "ada-notes".parse().unwrap()
    }

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    fn deploy(digit: char) -> DeployId {
        digit.to_string().repeat(64).parse().unwrap()
    }

    fn build(byte: u8) -> BuildId {
        BuildId::from_bytes([byte; 16])
    }

    fn address() -> ExternalUrl {
        ExternalUrl::parse("https://ada-notes.uses.ink").unwrap()
    }

    fn source() -> build::Source {
        build::Source::Git { url: "https://github.com/ada/notes".parse().unwrap(), revision: None }
    }

    fn pointer(digit: char) -> Pointer {
        Pointer {
            deploy: Self::deploy(digit),
            since: Self::at("2026-11-27T09:30:00Z"),
            files: 23,
            bytes: Bytes(412_000),
        }
    }

    fn made(digit: char, live: bool) -> Deploy {
        Deploy {
            id: Self::deploy(digit),
            created: Self::at("2026-11-27T09:30:12Z"),
            files: 23,
            bytes: Bytes(412_000),
            is_live: live,
        }
    }

    fn attempt(byte: u8, state: JobState, deploy: Option<Deploy>) -> Attempt {
        Attempt {
            build: Self::build(byte),
            trigger: Trigger::Push,
            state,
            requested: Self::at("2026-11-27T09:30:00Z"),
            finished: (!matches!(state, JobState::Queued | JobState::Running))
                .then(|| Self::at("2026-11-27T09:30:12Z")),
            deploy,
        }
    }

    fn path(text: &str) -> ContentPath {
        ContentPath::parse(text).unwrap()
    }

    fn problem() -> Diagnostic {
        Diagnostic::new(Problem::SourceFileUnreadable {
            path: Self::path("posts/x.md"),
            reason: "permission denied".to_owned(),
        })
    }

    fn decisions() -> Vec<AnyDecision> {
        let post = Self::path("posts/rust-allocators.md");
        let old = Self::path("posts/old/x.md");
        vec![
            Decision::<Title>::new(
                File(post.clone()),
                "Rust allocators".to_owned(),
                Evidence::Heading { path: post.clone(), span: Span::new(0, 17).unwrap() },
            )
            .into(),
            Decision::<Date>::new(File(old.clone()), None, Evidence::Default { reason: DefaultReason::NothingMatched })
                .into(),
            Decision::<Ignored>::new(
                File(Self::path("LICENSE")),
                true,
                Evidence::Marker { path: Self::path("LICENSE") },
            )
            .into(),
            Decision::<Home>::new(
                Whole,
                Some(Self::path("README.md")),
                Evidence::Config { span: Span::new(10, 20).unwrap() },
            )
            .into(),
        ]
    }

    fn why() -> Why {
        let post = Self::path("posts/rust-allocators.md");
        let said = |value: &str, evidence: Evidence, chosen: bool| Heard {
            testimony: Decision::<Title>::new(File(post.clone()), value.to_owned(), evidence).into(),
            standing: Standing::Value,
            chosen,
        };
        Why {
            frame: Frame::of(&Self::handle(), Screen::Understood),
            handle: Self::handle(),
            code: model::AnyCode::Title,
            subject: Subject::File(post.clone()),
            rungs: vec![
                Rung { witness: Witnessed::Pin, heard: vec![] },
                Rung { witness: Witnessed::Front, heard: vec![] },
                Rung {
                    witness: Witnessed::Heading,
                    heard: vec![said(
                        "Rust allocators",
                        Evidence::Heading { path: post.clone(), span: Span::new(0, 17).unwrap() },
                        true,
                    )],
                },
                Rung {
                    witness: Witnessed::Default,
                    heard: vec![said("posts", Evidence::Filename { path: post.clone() }, false)],
                },
            ],
        }
    }

    fn events() -> Vec<Event> {
        vec![
            Event::Started { build: Self::build(1), trigger: Trigger::Push },
            Event::Decided(Self::decisions().remove(0)),
            Event::Diagnosed(Self::problem()),
            Event::Rendered { files: 23, bytes: Bytes(412_000) },
            Event::Published {
                deploy: Self::deploy('a'),
                previous: Some(Self::deploy('b')),
                visible_within: Duration::from_secs(30),
            },
            Event::Finished { status: Ended::Succeeded, wall: Duration::from_millis(1840) },
        ]
    }

    fn refused() -> Vec<Event> {
        vec![
            Event::Started { build: Self::build(2), trigger: Trigger::Manual },
            Event::Refused(Self::problem()),
            Event::Finished { status: Ended::Refused, wall: Duration::from_millis(210) },
        ]
    }

    fn failures() -> Vec<(&'static str, Failure)> {
        let site = Id::<control::SiteKind>::from_bytes([7; 16]);
        vec![
            ("failure_no_site", Failure::NoSite { handle: Self::handle() }),
            ("failure_signed_out", Failure::SignedOut),
            ("failure_refused", Failure::Refused { refusal: Refusal::AlreadyLive { site, deploy: Self::deploy('a') } }),
            (
                "failure_store",
                Failure::Store {
                    part: uses_ink_web::Part::Blobs,
                    source: Cause::of(&std::io::Error::other("no space left on device")),
                },
            ),
            (
                "failure_form",
                Failure::Invalid {
                    field: Field::Deploy,
                    source: FieldError::Deploy("zz".parse::<model::Hash>().unwrap_err()),
                },
            ),
        ]
    }

    /// Every page, by name.
    fn pages() -> Vec<(String, String)> {
        let mut pages = Pages(Vec::new());
        Self::site_pages(&mut pages);
        Self::deploy_pages(&mut pages);
        Self::account_pages(&mut pages);
        Self::build_pages(&mut pages);
        pages.0
    }

    fn site_pages(pages: &mut Pages) {
        let handle = Self::handle();
        let mut live = Site::new(handle.clone(), Self::address(), Serving::Live(Self::pointer('a')));
        live.recent = vec![
            Self::attempt(1, JobState::Succeeded, Some(Self::made('a', true))),
            Self::attempt(2, JobState::Refused, None),
        ];
        live.observed = model::Observed::new(vec![model::Ability::Snapshot, model::Ability::History], vec![]);
        live.problems = vec![Self::problem()];
        pages.put("site_live", &live);
        let mut paused = Site::new(handle.clone(), Self::address(), Serving::Paused(Self::pointer('b')));
        paused.recent = vec![Self::attempt(3, JobState::Running, None)];
        pages.put("site_paused_building", &paused);
        pages.put("site_empty", &Site::new(handle.clone(), Self::address(), Serving::Nothing));

        let mut understood = Understood::new(handle.clone(), Self::source(), Self::decisions());
        understood.deploy = Some(Self::deploy('a'));
        pages.put("understood", &understood);
        understood.only = Only::Check;
        pages.put("understood_check_only", &understood);
        pages.put("understood_empty", &Understood::new(handle.clone(), Self::source(), vec![]));
        pages.put("why", &Self::why());
        pages.put("why_empty", &Why { rungs: vec![], ..Self::why() });
    }

    fn deploy_pages(pages: &mut Pages) {
        let handle = Self::handle();
        pages.put(
            "deploys",
            &Deploys::new(
                handle.clone(),
                Serving::Paused(Self::pointer('b')),
                vec![
                    Self::attempt(1, JobState::Succeeded, Some(Self::made('a', false))),
                    Self::attempt(2, JobState::Succeeded, Some(Self::made('b', true))),
                    Self::attempt(3, JobState::Failed, None),
                ],
            ),
        );
        pages.put("deploys_empty", &Deploys::new(handle.clone(), Serving::Nothing, vec![]));
    }

    fn account_pages(pages: &mut Pages) {
        let handle = Self::handle();
        let mut settings =
            Settings::new(handle.clone(), Self::address(), Self::source(), Serving::Live(Self::pointer('a')));
        settings.credentials = vec![Held { purpose: Purpose::GitToken, created: Self::at("2026-11-20T08:00:00Z") }];
        pages.put("settings", &settings);
        pages.put("settings_empty", &Settings::new(handle.clone(), Self::address(), Self::source(), Serving::Nothing));

        pages.put(
            "account",
            &Account::new(
                Self::at("2026-11-01T12:00:00Z"),
                vec![
                    Owned {
                        handle: handle.clone(),
                        state: SiteState::Active,
                        created: Self::at("2026-11-02T12:00:00Z"),
                    },
                    Owned {
                        handle: "half-gone".parse().unwrap(),
                        state: SiteState::Removing,
                        created: Self::at("2026-11-03T12:00:00Z"),
                    },
                ],
            ),
        );
        pages.put("account_empty", &Account::new(Self::at("2026-11-01T12:00:00Z"), vec![]));
    }

    fn build_pages(pages: &mut Pages) {
        let handle = Self::handle();
        pages.put(
            "build_done",
            &Build::new(
                handle.clone(),
                Self::build(1),
                Self::events(),
                How::Finished(Outcome {
                    status: Ended::Succeeded,
                    deploy: Some(Self::deploy('a')),
                    wall: Duration::from_millis(1840),
                }),
            ),
        );
        pages.put(
            "build_refused",
            &Build::new(
                handle.clone(),
                Self::build(2),
                Self::refused(),
                How::Finished(Outcome { status: Ended::Refused, deploy: None, wall: Duration::from_millis(210) }),
            ),
        );
        pages.put(
            "build_empty",
            &Build::new(handle.clone(), Self::build(4), vec![], How::Cut(Failure::Cut { build: Self::build(4) })),
        );

        for (name, failure) in Self::failures() {
            pages.put(name, &Failed::new(Frame::of(&handle, Screen::Site), failure));
        }
    }
}

/// A view as its whole page.
trait Pageable {
    fn page(&self) -> String;
}

impl<V: View> Pageable for V {
    fn page(&self) -> String {
        Document(self).write().unwrap()
    }
}

#[test]
fn pages_match_their_goldens() {
    fixture::cases("fixture", Fixtures::pages());
}

/// The checks over a page's markup that no golden reads for you.
struct Structure<'a>(&'a str);

impl Structure<'_> {
    fn tags(&self, name: &str) -> Vec<&str> {
        let open = format!("<{name}");
        self.0
            .match_indices(&open)
            .filter(|(at, _)| self.0[at + open.len()..].starts_with([' ', '>', '/']))
            .map(|(at, _)| &self.0[at..=at + self.0[at..].find('>').unwrap()])
            .collect()
    }

    fn forms(&self) -> Vec<&str> {
        let mut forms = Vec::new();
        let mut rest = self.0;
        while let Some(start) = rest.find("<form") {
            let end = rest[start..].find("</form>").unwrap() + start;
            forms.push(&rest[start..end]);
            rest = &rest[end..];
        }
        forms
    }
}

#[test]
fn every_page_has_one_heading_and_every_form_a_label() {
    for (name, page) in Fixtures::pages() {
        let structure = Structure(&page);
        assert_eq!(structure.tags("h1").len(), 1, "{name}: one h1");
        assert!(page.starts_with("<!doctype html><html lang=\"en\">"), "{name}: a document");
        assert!(page.contains("name=\"viewport\""), "{name}: responsive");
        assert!(page.contains("href=\"#main\""), "{name}: a skip link");
        for form in structure.forms() {
            assert!(form.contains("aria-label=\""), "{name}: a form without a name: {form}");
            let controls = Structure(form);
            let visible = ["input", "select", "textarea"]
                .iter()
                .flat_map(|tag| controls.tags(tag))
                .filter(|tag| !tag.contains("type=\"hidden\""))
                .count();
            if visible > 0 {
                assert!(form.contains("<label"), "{name}: a control without a label: {form}");
            }
        }
        assert!(!page.contains("<script"), "{name}: no script");
        assert!(!page.contains("javascript:"), "{name}: no script url");
    }
}

#[test]
fn a_refresh_is_sent_only_while_a_build_runs() {
    let pages: std::collections::BTreeMap<String, String> = Fixtures::pages().into_iter().collect();
    for (name, page) in &pages {
        let refreshes = page.contains("http-equiv=\"refresh\"");
        assert_eq!(refreshes, name == "site_paused_building", "{name}");
    }
}

/// What the edge provides, over fixtures.
struct Fake;

struct Replay(std::vec::IntoIter<Event>);

impl Follow for Replay {
    fn next(&mut self) -> impl Future<Output = Result<Option<Event>, Failure>> + Send {
        std::future::ready(Ok(self.0.next()))
    }
}

impl Screens for Fake {
    type Follow = Replay;

    fn site(&self, _: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Site, Failure>> + Send {
        std::future::ready(Ok(Site::new(handle.clone(), Fixtures::address(), Serving::Nothing)))
    }

    fn understood(
        &self,
        _: Id<AccountKind>,
        handle: &Handle,
        _: Only,
    ) -> impl Future<Output = Result<Understood, Failure>> + Send {
        std::future::ready(Ok(Understood::new(handle.clone(), Fixtures::source(), vec![])))
    }

    fn why(&self, _: Id<AccountKind>, _: &Handle, _: &Asked) -> impl Future<Output = Result<Why, Failure>> + Send {
        std::future::ready(Ok(Fixtures::why()))
    }

    fn deploys(&self, _: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Deploys, Failure>> + Send {
        std::future::ready(Ok(Deploys::new(handle.clone(), Serving::Nothing, vec![])))
    }

    fn settings(&self, _: Id<AccountKind>, _: &Handle) -> impl Future<Output = Result<Settings, Failure>> + Send {
        std::future::ready(Err(Failure::NoSite { handle: Fixtures::handle() }))
    }

    fn account(&self, _: Id<AccountKind>) -> impl Future<Output = Result<Account, Failure>> + Send {
        std::future::ready(Ok(Account::new(Fixtures::at("2026-11-01T12:00:00Z"), vec![])))
    }

    fn log(
        &self,
        _: Id<AccountKind>,
        handle: &Handle,
        build: BuildId,
    ) -> impl Future<Output = Result<Logged<Replay>, Failure>> + Send {
        std::future::ready(Ok(Logged::Running(
            Live::new(handle.clone(), build),
            Replay(Fixtures::events().into_iter()),
        )))
    }

    fn act(&self, _: Id<AccountKind>, _: &Handle, _: &Action) -> impl Future<Output = Result<(), Failure>> + Send {
        std::future::ready(Ok(()))
    }

    fn export(&self, _: Id<AccountKind>, handle: &Handle) -> impl Future<Output = Result<Download, Failure>> + Send {
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
fn text_from_a_source_never_carries_a_control_character() {
    let hostile = "a\u{1b}]52;c;x\u{7}b\u{9b}c";
    let handle = Fixtures::handle();
    let diagnostic = Diagnostic::new(Problem::SourceFileUnreadable {
        path: Fixtures::path("posts/x.md"),
        reason: hostile.to_owned(),
    });
    let mut site = Site::new(handle.clone(), Fixtures::address(), Serving::Nothing);
    site.problems = vec![diagnostic.clone()];
    let source = build::Source::Folder(std::path::PathBuf::from(hostile));
    let understood = Understood::new(handle.clone(), source.clone(), Fixtures::decisions());
    let settings = Settings::new(handle.clone(), Fixtures::address(), source, Serving::Nothing);
    let log = Build::new(
        handle.clone(),
        Fixtures::build(1),
        vec![Event::Diagnosed(diagnostic.clone()), Event::Refused(diagnostic)],
        How::Cut(Failure::Store {
            part: uses_ink_web::Part::Records,
            source: Cause::of(&std::io::Error::other(hostile)),
        }),
    );
    let failed = Failed::new(
        Frame::of(&handle, Screen::Site),
        Failure::Store { part: uses_ink_web::Part::Blobs, source: Cause::of(&std::io::Error::other(hostile)) },
    );
    let pages = [site.page(), understood.page(), settings.page(), log.page(), failed.page()];
    for page in &pages {
        assert!(
            !page.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\t')),
            "a control character reached a page"
        );
    }
    assert!(pages.iter().all(|page| page.contains("\\u{1b}")), "the escape is shown, not dropped");
}
