use engine::Sink as _;
use model::{Diagnostic, Notice};

use super::State;
use crate::bind::plugin::ink::host::diagnostic;
use crate::bind::plugin::ink::model as types;

impl diagnostic::Host for State {
    fn emit(&mut self, diagnostic: types::diagnostic::Diagnostic) {
        match Diagnostic::try_from(diagnostic) {
            Ok(diagnostic) => self.said.diagnose(diagnostic),
            Err(refused) => self.malformed(&refused),
        }
    }

    fn announce(&mut self, notice: types::diagnostic::Notice) {
        match Notice::try_from(notice) {
            Ok(notice) => self.said.notice(notice),
            Err(refused) => self.malformed(&refused),
        }
    }
}
