//! `site.format`: which format plugin reads the site's markdown. It is decided before anything
//! is read, since reading is what it chooses, so the host decides it ([`Chosen::new`]), builds
//! the engine over the format it names, and the decision joins what the engine understood
//! ([`Understood::with`](crate::Understood::with)).

use model::code::{AnyCode, AnyDecision};
use model::decision::Site;
use model::record::PluginId;
use model::{Diagnostic, Notice, Snapshot, Subject};

use crate::Pins;
use crate::interpret::Trace;
use crate::interpret::decide::ladder::About;
use crate::interpret::decide::precedence;
use crate::interpret::journal::Journal;

/// The format a snapshot is read with, and the decision that chose it.
#[derive(Debug)]
pub struct Chosen {
    format: PluginId,
    decisions: Vec<AnyDecision>,
    diagnostics: Vec<Diagnostic>,
    notices: Vec<Notice>,
    trace: Option<Trace>,
}

impl Chosen {
    /// Decides `site.format` for `snapshot` from what `pins` pin, replaying its ladder when
    /// `asked` names it. The ladder always decides: the markdown format is its last rung.
    pub fn new<P: Pins>(snapshot: &Snapshot, pins: &P, asked: Option<(AnyCode, Subject)>) -> Self {
        let mut journal = asked.map_or_else(Journal::default, |(code, subject)| Journal::asking(code, subject));
        let about = About { subject: &Site, ground: snapshot };
        let format = precedence::Format::default().record(about, pins, &mut journal).unwrap_or_else(PluginId::markdown);
        let (decisions, diagnostics, notices, trace) = journal.into_parts();
        Self { format, decisions, diagnostics, notices, trace }
    }

    /// The format plugin that reads the site's markdown.
    pub const fn format(&self) -> &PluginId {
        &self.format
    }

    /// The decision, what the ladder said of it (an error when it could not decide, in which
    /// case the markdown format reads), its notices, and its replay.
    pub(crate) fn into_parts(self) -> (Vec<AnyDecision>, Vec<Diagnostic>, Vec<Notice>, Option<Trace>) {
        (self.decisions, self.diagnostics, self.notices, self.trace)
    }
}
