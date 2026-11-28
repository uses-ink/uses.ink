//! The fixture values every screen's pages are made from.

use super::{Fixtures, Pages};
use build::{BuildId, Trigger};
use control::{Handle, JobState};
use jiff::Timestamp;
use model::code::{Date, Home, Ignored, Title};
use model::decision::{File, Site as Whole};
use model::{
    AnyDecision, Bytes, ContentPath, Decision, DefaultReason, DeployId, Diagnostic, Evidence, ExternalUrl, Problem,
    Span,
};
use uses_ink_web::view::{Attempt, Deploy, Pointer};

impl Fixtures {
    pub(crate) fn handle() -> Handle {
        "ada-notes".parse().unwrap()
    }

    pub(crate) fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    pub(crate) fn deploy(digit: char) -> DeployId {
        digit.to_string().repeat(64).parse().unwrap()
    }

    pub(crate) fn build(byte: u8) -> BuildId {
        BuildId::from_bytes([byte; 16])
    }

    pub(crate) fn address() -> ExternalUrl {
        ExternalUrl::parse("https://ada-notes.uses.ink").unwrap()
    }

    pub(crate) fn source() -> build::Source {
        build::Source::Git { url: "https://github.com/ada/notes".parse().unwrap(), revision: None }
    }

    pub(crate) fn pointer(digit: char) -> Pointer {
        Pointer {
            deploy: Self::deploy(digit),
            since: Self::at("2026-11-27T09:30:00Z"),
            files: 23,
            bytes: Bytes(412_000),
        }
    }

    pub(crate) fn made(digit: char, live: bool) -> Deploy {
        Deploy {
            id: Self::deploy(digit),
            created: Self::at("2026-11-27T09:30:12Z"),
            files: 23,
            bytes: Bytes(412_000),
            is_live: live,
        }
    }

    pub(crate) fn attempt(byte: u8, state: JobState, deploy: Option<Deploy>) -> Attempt {
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

    pub(crate) fn path(text: &str) -> ContentPath {
        ContentPath::parse(text).unwrap()
    }

    pub(crate) fn problem() -> Diagnostic {
        Diagnostic::new(Problem::SourceFileUnreadable {
            path: Self::path("posts/x.md"),
            reason: "permission denied".to_owned(),
        })
    }

    pub(crate) fn decisions() -> Vec<AnyDecision> {
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
}

impl Fixtures {
    /// Every page, by name.
    pub(crate) fn pages() -> Vec<(String, String)> {
        let mut pages = Pages(Vec::new());
        Self::site_pages(&mut pages);
        Self::deploy_pages(&mut pages);
        Self::settings_pages(&mut pages);
        Self::account_pages(&mut pages);
        Self::build_pages(&mut pages);
        Self::failure_pages(&mut pages);
        Self::limit_pages(&mut pages);
        Self::sign_pages(&mut pages);
        pages.0
    }
}
