//! The suites the Worker runs.

use worker::{Env, Response, Result};

use crate::blobs::Blobs;
use crate::jobs::Jobs;
use crate::rows::Rows;

/// One conformance suite, by the port it tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Suite {
    /// The row port on D1.
    Db,
    /// The blob port on R2.
    Blob,
    /// The queue port on the rows of D1, woken through Queues.
    Queue,
}

impl Suite {
    /// Every suite, in the order they are listed.
    const ALL: [Self; 3] = [Self::Db, Self::Blob, Self::Queue];

    /// Its name in the paths.
    const fn name(self) -> &'static str {
        match self {
            Self::Db => "db",
            Self::Blob => "blob",
            Self::Queue => "queue",
        }
    }

    /// The suite named `name`.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|suite| suite.name() == name)
    }

    /// Its cases' names.
    fn cases(self) -> Vec<&'static str> {
        match self {
            Self::Db => Rows::names(),
            Self::Blob => Blobs::names(),
            Self::Queue => Jobs::names(),
        }
    }

    /// Every case of every suite, one `<suite>/<case>` a line.
    pub(crate) fn listing() -> String {
        let mut text = String::new();
        for suite in Self::ALL {
            for case in suite.cases() {
                text.push_str(&format!("{}/{case}\n", suite.name()));
            }
        }
        text
    }

    /// Runs the case `case` of this suite on a fresh store.
    pub(crate) async fn run(self, env: &Env, case: &str) -> Result<Response> {
        let ran = match self {
            Self::Db => Rows { env }.run(case).await?,
            Self::Blob => Blobs { env }.run(case).await?,
            Self::Queue => Jobs { env }.run(case).await?,
        };
        if ran { Response::ok(format!("ok {}/{case}\n", self.name())) } else { Response::error(format!("no case {case}"), 404) }
    }
}
