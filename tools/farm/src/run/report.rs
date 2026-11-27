//! The report a verb prints, and the exit it ends with.
use crate::cli::error::{Chain, Error};
use crate::cli::exit::Exit;
use crate::host::transport::Transport;
use crate::job::{Document, Finished};
use crate::place::Placement;
use crate::run::{Batch, Farm};

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Prints a batch as text or as `uses.farm/v1`, and returns the exit it ends with.
    pub(in crate::run) fn report(&self, batch: &Batch) -> Exit {
        let exit = batch
            .done
            .iter()
            .map(Finished::exit)
            .chain(batch.failed.iter().map(Error::exit))
            .fold(Exit::Passed, Exit::worst);
        if self.flags.json {
            let document = Document {
                schema: Document::SCHEMA,
                exit: exit.code(),
                jobs: batch.done.iter().map(Finished::wire).collect(),
                blessed: batch.blessed.clone(),
                formatted: batch.formatted.clone(),
            };
            match serde_json::to_string_pretty(&document) {
                Ok(text) => self.console.out(text),
                Err(error) => self.console.fail(format!("error: the report is not JSON: {error}")),
            }
        } else {
            batch.done.iter().for_each(|done| self.console.out(done));
        }
        batch.failed.iter().for_each(|error| self.console.fail(Chain(error)));
        if !self.flags.json && !batch.blessed.is_empty() {
            self.console.out("blessed (read each diff before committing):");
            batch.blessed.iter().for_each(|path| self.console.out(format!("  {path}")));
        }
        if !self.flags.json && !batch.formatted.is_empty() {
            self.console.out("formatted (`jj diff` shows what changed):");
            batch.formatted.iter().for_each(|path| self.console.out(format!("  {path}")));
        }
        if self.console.is_broken() { exit.worst(Exit::Farm) } else { exit }
    }
}
