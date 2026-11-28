//! A commit status: what a build says about a commit, on the forge's own page.

use url::Url;

/// How a build of a commit stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Queued or running.
    Pending,
    /// Built and published.
    Success,
    /// The source did not build.
    Failure,
    /// The build could not run.
    Error,
}

impl State {
    /// The word GitHub and Forgejo use.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Error => "error",
        }
    }
}

/// A status to post to a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Status {
    /// How the build stands.
    pub state: State,
    /// A page about the build, such as the preview.
    pub link: Option<Url>,
    /// One line about it, at most [`Status::NOTE`] characters.
    pub note: String,
    /// What the status is about, so statuses of one kind replace each other.
    pub context: String,
}

impl Status {
    /// The context when none is given.
    pub const CONTEXT: &'static str = "uses.ink";
    /// The most characters of a note: GitHub's 140. A longer note is cut to that many
    /// characters by [`Status::note`], so no forge refuses it.
    pub const NOTE: usize = 140;

    /// A status in `state` with no link, no note and the default context.
    pub fn new(state: State) -> Self {
        Self { state, link: None, note: String::new(), context: Self::CONTEXT.to_owned() }
    }

    /// The same status linking to `link`.
    #[must_use]
    pub fn link(mut self, link: Url) -> Self {
        self.link = Some(link);
        self
    }

    /// The same status with `note`, cut to [`Status::NOTE`] characters.
    #[must_use]
    pub fn note(mut self, note: &str) -> Self {
        self.note = note.chars().take(Self::NOTE).collect();
        self
    }

    /// The same status about `context`.
    #[must_use]
    pub fn context(mut self, context: &str) -> Self {
        context.clone_into(&mut self.context);
        self
    }

    /// The JSON GitHub and Forgejo take for it.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "state": self.state.word(),
            "target_url": self.link.as_ref().map(Url::as_str),
            "description": self.note,
            "context": self.context,
        })
    }
}
