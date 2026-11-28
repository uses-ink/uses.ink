//! The decisions index, `docs/decisions/README.md`, generated from the front matter of the files
//! and the reserved rows of `.config/layout.toml`. It is checked in; a stale one fails the stage.

use std::path::{Path, PathBuf};

use super::Check;
use super::corpus::Corpus;
use super::decision::{Decision, Decisions};
use super::fault::Fault;
use super::number::Number;
use crate::error::Error;
use crate::layout::config::Numbering;

/// The index: what it holds, and the check that the file on disk holds it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Index;

impl Index {
    /// The head, up to the sentence about reserved numbers.
    const HEAD: &'static str = "# Decisions\n\nEach decision is one file, `NNNN-slug.md`, with front matter. A number never changes. A superseded\ndecision stays, with a pointer to its replacement.";

    /// The table's header rows.
    const TABLE: &'static str = "| Number | Title | Status | Date | Area |\n|---|---|---|---|---|";

    /// The index of `decisions` and the reserved numbers of `numbering`, in number order.
    pub(crate) fn render(decisions: &[Decision], numbering: &Numbering) -> String {
        let mut rows: Vec<(Number, String)> = decisions
            .iter()
            .map(|decision| (decision.number, Self::row(decision)))
            .chain(numbering.reserved.iter().map(|reserved| {
                (reserved.number, format!("| {} | {} | reserved |  |  |", reserved.number, reserved.title))
            }))
            .collect();
        rows.sort_by_key(|(number, _)| *number);
        let table: Vec<String> = rows.into_iter().map(|(_, row)| row).collect();
        let mut blocks = vec![Self::HEAD.to_owned()];
        blocks.extend(Self::reserved(numbering));
        blocks.push(format!("{}\n{}", Self::TABLE, table.join("\n")));
        format!("{}\n", blocks.join("\n\n"))
    }

    /// One row of a decision: its title links to its file.
    fn row(decision: &Decision) -> String {
        let front = &decision.front;
        format!(
            "| {} | [{}]({}) | {} | {} | {} |",
            decision.number,
            front.title.replace('|', "\\|"),
            decision.name,
            front.status,
            front.date,
            front.area
        )
    }

    /// The sentence that says which numbers are reserved and where they went.
    fn reserved(numbering: &Numbering) -> Option<String> {
        let numbers: Vec<String> = numbering.reserved.iter().map(|reserved| reserved.number.to_string()).collect();
        let home = &numbering.home;
        let link = format!("[`{home}`]({home})");
        match numbers.as_slice() {
            [] => None,
            [only] => Some(format!("Number {only} is reserved: it was a review-fix log, now in {link}.")),
            [rest @ .., last] => Some(format!(
                "Numbers {} and {last} are reserved: they were review-fix logs, now in {link}.",
                rest.join(", ")
            )),
        }
    }

    /// The path written, or none when the index was already current.
    pub(super) fn update(root: &Path) -> Result<Option<PathBuf>, Error> {
        let corpus = Corpus::read(root)?;
        let decisions = Decisions::read(&corpus);
        if !decisions.faults.is_empty() {
            return Err(Error::Docs(decisions.faults));
        }
        let text = Self::render(&decisions.all, &corpus.config.decisions);
        if corpus.text(Path::new(Decisions::INDEX)) == Some(text.as_str()) {
            return Ok(None);
        }
        let path = root.join(Decisions::INDEX);
        std::fs::write(&path, text).map_err(|source| Error::Write { path, source })?;
        Ok(Some(PathBuf::from(Decisions::INDEX)))
    }
}

impl Check for Index {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) {
        let decisions = Decisions::read(corpus);
        // a decision that does not read is a fault of `Fronts`; the index cannot be rendered
        if !decisions.faults.is_empty() {
            return;
        }
        let text = Self::render(&decisions.all, &corpus.config.decisions);
        if corpus.text(Path::new(Decisions::INDEX)) != Some(text.as_str()) {
            faults.push(Fault::Stale { path: PathBuf::from(Decisions::INDEX) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::config::Reserved;

    fn file(number: &str, title: &str, status: &str) -> (String, String) {
        let text = format!(
            "---\nnumber: \"{number}\"\ntitle: \"{title}\"\nstatus: {status}\ndate: 2026-11-26\narea: engine\nsupersedes: []\n---\n"
        );
        (format!("docs/decisions/{number}-t.md"), text)
    }

    fn corpus(files: &[(String, String)]) -> Corpus {
        let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        let mut corpus = Corpus::sample(&borrowed);
        corpus.config.decisions.home = "../history/log.md".to_owned();
        corpus.config.decisions.reserved = vec![Reserved::sample(2, "A log")];
        corpus
    }

    const WANT: &str = "# Decisions\n\nEach decision is one file, `NNNN-slug.md`, with front matter. A number never changes. A superseded\ndecision stays, with a pointer to its replacement.\n\nNumber 0002 is reserved: it was a review-fix log, now in [`../history/log.md`](../history/log.md).\n\n| Number | Title | Status | Date | Area |\n|---|---|---|---|---|\n| 0001 | [One](0001-t.md) | accepted | 2026-11-26 | engine |\n| 0002 | A log | reserved |  |  |\n| 0003 | [Th\\|ree](0003-t.md) | built | 2026-11-26 | engine |\n";

    fn files() -> Vec<(String, String)> {
        vec![file("0003", "Th|ree", "built"), file("0001", "One", "accepted")]
    }

    #[test]
    fn the_index_is_sorted_with_reserved_rows_and_escaped_titles() {
        let corpus = corpus(&files());
        let decisions = Decisions::read(&corpus);
        assert_eq!(Index::render(&decisions.all, &corpus.config.decisions), WANT);
    }

    #[test]
    fn an_index_that_matches_passes_and_any_other_is_stale() {
        let mut all = files();
        all.push((Decisions::INDEX.to_owned(), WANT.to_owned()));
        let mut faults = Vec::new();
        Index.check(&corpus(&all), &mut faults);
        assert!(faults.is_empty(), "{faults:?}");
        all.pop();
        all.push((Decisions::INDEX.to_owned(), WANT.replace("built", "accepted")));
        Index.check(&corpus(&all), &mut faults);
        assert_eq!(faults.len(), 1);
        assert!(matches!(faults.first(), Some(Fault::Stale { path }) if path == Path::new(Decisions::INDEX)));
    }

    #[test]
    fn a_missing_index_is_stale() {
        let mut faults = Vec::new();
        Index.check(&corpus(&files()), &mut faults);
        assert_eq!(faults.len(), 1);
    }

    #[test]
    fn two_reserved_numbers_are_listed_with_and() {
        let mut corpus = corpus(&files());
        corpus.config.decisions.reserved = vec![Reserved::sample(2, "A"), Reserved::sample(4, "B")];
        let text = Index::reserved(&corpus.config.decisions).unwrap();
        assert!(text.starts_with("Numbers 0002 and 0004 are reserved: they were review-fix logs"), "{text}");
    }
}
