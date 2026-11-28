//! The suite: what any smart-HTTP reader of references must answer, over the recorded
//! advertisements of three forges and a few made ones. Async, so a native test and a Worker both
//! run it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::indexing_slicing,
    clippy::assert_is_empty,
    reason = "a conformance case stops at the first surprise"
)]

use build::Revision;
use model::git::{Locator, Network};
use secret::Plain;
use watch::{Access, Head, Heads};

use crate::{Absent, Basic, Forge, Misread, Replay, Reply, Smart, Unlisted};

/// One case of the suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Case {
    /// GitHub's advertisement: default branch, a named branch, every branch.
    Github,
    /// Forgejo's: the same, and a tag.
    Forgejo,
    /// GitLab's: many branches, branch names that need care, peeled tags.
    Gitlab,
    /// The login goes out as `Authorization: Basic`, and only when there is one.
    Login,
    /// A scheme a Worker cannot reach, or the network setting, is refused before any request.
    Refused,
    /// A status that is no advertisement, and a call that got no answer.
    Failed,
    /// A revision that is not there, and a repository with no commit yet.
    Absent,
    /// A body over the limit, a cut one, and one that is no advertisement.
    Misread,
}

impl Case {
    /// Every case, in the order the suite runs them.
    pub const ALL: &'static [Self] = &[
        Self::Github,
        Self::Forgejo,
        Self::Gitlab,
        Self::Login,
        Self::Refused,
        Self::Failed,
        Self::Absent,
        Self::Misread,
    ];

    /// Its name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Github => "reads_githubs_advertisement",
            Self::Forgejo => "reads_forgejos_advertisement",
            Self::Gitlab => "reads_gitlabs_advertisement",
            Self::Login => "sends_the_login_as_basic_only_when_there_is_one",
            Self::Refused => "refuses_a_scheme_a_worker_cannot_reach_before_any_request",
            Self::Failed => "says_what_a_bad_status_or_a_silent_remote_was",
            Self::Absent => "names_a_revision_that_is_not_there",
            Self::Misread => "refuses_a_body_that_is_too_large_cut_or_not_git",
        }
    }

    /// The case named `name`, if there is one.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|case| case.name() == name)
    }

    /// Runs the case. It panics at the first surprise.
    pub async fn run(self) {
        match self {
            Self::Github => {
                Self::forge(
                    Forge::Github,
                    "master",
                    &[("test", "refs/heads/test"), ("octocat-patch-1", "refs/heads/octocat-patch-1")],
                )
                .await;
            }
            Self::Forgejo => {
                Self::forge(
                    Forge::Forgejo,
                    "next",
                    &[("v13.0.0-dev", "refs/tags/v13.0.0-dev"), ("v10.0", "refs/heads/v10.0")],
                )
                .await;
            }
            Self::Gitlab => {
                Self::forge(
                    Forge::Gitlab,
                    "master",
                    &[("feature", "refs/heads/feature"), ("audio", "refs/heads/audio")],
                )
                .await;
            }
            Self::Login => Self::login().await,
            Self::Refused => Self::refused().await,
            Self::Failed => Self::failed().await,
            Self::Absent => Self::absent().await,
            Self::Misread => Self::misread().await,
        }
    }

    /// The URL of the advertisement of `repository`.
    fn advert(repository: &str) -> String {
        format!("{repository}/info/refs?service=git-upload-pack")
    }

    /// A reader over one reply for `repository`.
    fn reading(repository: &str, reply: Reply) -> Smart<Replay> {
        Smart::new(Replay::new().serving(Self::advert(repository), reply), Network::On)
    }

    fn ok(body: &[u8]) -> Reply {
        Reply { status: 200, body: body.to_vec() }
    }

    /// The commit the recorded body gives `name`, found by looking at the text and nothing else.
    fn written(body: &[u8], name: &str) -> Head {
        let text = String::from_utf8_lossy(body);
        let end = if name == "HEAD" { "\0" } else { "\n" };
        let at = text.find(&format!(" {name}{end}")).unwrap();
        text[at - 40..at].parse().unwrap()
    }

    /// One packet of a made advertisement.
    fn packet(text: &str) -> String {
        format!("{:04x}{text}", text.len() + 4)
    }

    async fn forge(forge: Forge, default: &str, named: &[(&str, &str)]) {
        let (body, url) = (forge.body(), forge.url());
        let reader = Self::reading(url, Self::ok(body));
        let locator: Locator = url.parse().unwrap();
        assert_eq!(reader.head(&locator, None, None).await.unwrap(), Self::written(body, "HEAD"));
        for (name, full) in named {
            let revision: Revision = name.parse().unwrap();
            let head = reader.head(&locator, Some(&revision), None).await.unwrap();
            assert_eq!(head, Self::written(body, full), "{name}");
        }
        let branches = reader.branches(&locator, None).await.unwrap();
        assert_eq!(branches.default.map(|branch| branch.to_string()), Some(default.to_owned()));
        let heads = String::from_utf8_lossy(body).matches(" refs/heads/").count();
        assert_eq!(branches.found.len() + branches.skipped.len(), heads);
        let listed: Vec<String> = branches.found.iter().map(|tip| tip.branch.to_string()).collect();
        assert!(listed.windows(2).all(|pair| pair[0] < pair[1]), "branches are in the order of their names");
        let tip = branches.found.iter().find(|tip| tip.branch.to_string() == default).unwrap();
        assert_eq!(tip.head, Self::written(body, "HEAD"), "HEAD is the default branch's commit");
        let checked = Self::peeled(&reader, &locator, body).await;
        assert!(checked > 0 || forge != Forge::Gitlab, "no annotated tag was read");
    }

    /// A tag that is annotated reads as the commit it points at, not the tag object.
    async fn peeled(reader: &Smart<Replay>, locator: &Locator, body: &[u8]) -> usize {
        let (text, mut checked) = (String::from_utf8_lossy(body), 0);
        for line in text.split('\n').filter(|line| line.ends_with("^{}")) {
            let at = line.find(" refs/tags/").unwrap();
            let name = line[at + " refs/tags/".len()..].trim_end_matches("^{}");
            let commit: Head = line[at - 40..at].parse().unwrap();
            if text.contains(&format!(" refs/heads/{name}\n")) {
                // a branch of the same name is found first, as `git` does
                continue;
            }
            let revision: Revision = name.parse().unwrap();
            assert_eq!(reader.head(locator, Some(&revision), None).await.unwrap(), commit, "peeled {name}");
            checked += 1;
        }
        checked
    }

    async fn login() {
        let url = "https://forge.example/ink/site.git";
        let locator: Locator = url.parse().unwrap();
        let access = Access::try_from(Plain::from("ink:tok")).unwrap();
        let reader = Self::reading(url, Self::ok(Forge::Github.body()));
        reader.head(&locator, None, Some(&access)).await.unwrap();
        reader.head(&locator, None, None).await.unwrap();
        let seen = reader.fetch().seen();
        assert_eq!(seen[0].url, Self::advert(url));
        assert_eq!(seen[0].basic, Some(Basic::new("ink", "tok")));
        assert_eq!(seen[1].basic, None);
        assert!(!format!("{:?}", seen[0]).contains("tok"), "the login is in a request's Debug output");
    }

    async fn refused() {
        let file: Locator = "file:///srv/site".parse().unwrap();
        let reader = Smart::new(Replay::new(), Network::Local);
        let error = reader.head(&file, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Scheme { .. }), "{error:?}");
        let off = Smart::new(Replay::new(), Network::Off);
        let https: Locator = "https://forge.example/ink/site.git".parse().unwrap();
        let error = off.head(&https, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Network { .. }), "{error:?}");
        assert!(reader.fetch().seen().is_empty() && off.fetch().seen().is_empty(), "a request was made");
    }

    async fn failed() {
        let url = "https://forge.example/ink/site.git";
        let locator: Locator = url.parse().unwrap();
        let denied = Self::reading(url, Reply { status: 401, body: Vec::new() });
        let error = denied.head(&locator, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Status { code: 401 }), "{error:?}");
        let silent = Smart::new(Replay::new(), Network::On);
        let error = silent.branches(&locator, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Fetch(_)), "{error:?}");
    }

    async fn absent() {
        let url = "https://forge.example/ink/site.git";
        let locator: Locator = url.parse().unwrap();
        let reader = Self::reading(url, Self::ok(Forge::Github.body()));
        let revision: Revision = "nothing".parse().unwrap();
        let error = reader.head(&locator, Some(&revision), None).await.unwrap_err();
        assert!(matches!(&error, Unlisted::Absent(Absent::Revision { written }) if written == "nothing"), "{error:?}");
        let none = "0000000000000000000000000000000000000000 capabilities^{}\0symref=HEAD:refs/heads/main\n";
        let empty = format!("{}0000{}0000", Self::packet("# service=git-upload-pack\n"), Self::packet(none));
        let reader = Self::reading(url, Self::ok(empty.as_bytes()));
        let error = reader.head(&locator, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Absent(Absent::Unborn)), "{error:?}");
        let branches = reader.branches(&locator, None).await.unwrap();
        assert_eq!(branches.found.len(), 0, "{branches:?}");
    }

    async fn misread() {
        let url = "https://forge.example/ink/site.git";
        let locator: Locator = url.parse().unwrap();
        let large = Self::reading(url, Self::ok(Forge::Gitlab.body())).most_of(100);
        let error = large.head(&locator, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Large { most: 100 }), "{error:?}");
        let cut = Self::reading(url, Self::ok(&Forge::Github.body()[..120]));
        let error = cut.head(&locator, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Advert(Misread::Truncated { .. })), "{error:?}");
        let page = Self::reading(url, Self::ok(b"<html><body>sign in</body></html>"));
        let error = page.head(&locator, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Advert(Misread::Length { at: 0 })), "{error:?}");
        let other = format!("{}0000", Self::packet("# service=git-receive-pack\n"));
        let error = Self::reading(url, Self::ok(other.as_bytes())).head(&locator, None, None).await.unwrap_err();
        assert!(matches!(error, Unlisted::Advert(Misread::Service)), "{error:?}");
    }
}
