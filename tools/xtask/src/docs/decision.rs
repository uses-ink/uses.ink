//! The decision files of `docs/decisions/`: read into [`Decision`]s, and the checks over them:
//! valid front matter, no gap in the numbers that is not explained, and supersession that agrees
//! with the statuses.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::Check;
use super::corpus::Corpus;
use super::fault::Fault;
use super::front::{Front, Status};
use super::number::Number;

/// One decision file, read.
#[derive(Debug)]
pub(crate) struct Decision {
    /// Relative to the root.
    pub(crate) file: PathBuf,
    /// The file's name, `NNNN-slug.md`.
    pub(crate) name: String,
    /// The number its name gives.
    pub(crate) number: Number,
    pub(crate) front: Front,
}

/// Every decision file that reads, and the faults of those that do not.
#[derive(Debug)]
pub(crate) struct Decisions {
    /// Sorted by number.
    pub(crate) all: Vec<Decision>,
    pub(crate) faults: Vec<Fault>,
}

impl Decisions {
    /// Where the decision files are.
    pub(crate) const DIR: &'static str = "docs/decisions";
    /// The generated index, which is no decision.
    pub(crate) const INDEX: &'static str = "docs/decisions/README.md";

    /// Reads the decision files of `corpus`.
    pub(crate) fn read(corpus: &Corpus) -> Self {
        let mut all = Vec::new();
        let mut faults = Vec::new();
        let ceiling = corpus.config.decisions.ceiling;
        let files = corpus.files.iter().filter(|doc| {
            doc.path.parent() == Some(std::path::Path::new(Self::DIR))
                && doc.path.extension().is_some_and(|extension| extension == "md")
                && doc.path != std::path::Path::new(Self::INDEX)
        });
        for doc in files {
            let name = doc.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let Some(number) = Self::stem(&name) else {
                faults.push(Fault::Shape { file: doc.path.clone() });
                continue;
            };
            match Front::parse(&doc.text) {
                Ok(front) => {
                    if front.number != number {
                        faults.push(Fault::Misnumbered { file: doc.path.clone(), found: front.number });
                    }
                    if number.get() > ceiling {
                        faults.push(Fault::Ceiling { file: doc.path.clone(), number });
                    }
                    all.push(Decision { file: doc.path.clone(), name, number, front });
                }
                Err(defects) => {
                    faults.extend(defects.into_iter().map(|defect| Fault::Front { file: doc.path.clone(), defect }));
                }
            }
        }
        all.sort_by_key(|decision| decision.number);
        for pair in all.windows(2) {
            if let [first, second] = pair
                && first.number == second.number
            {
                faults.push(Fault::Twice { number: first.number });
            }
        }
        Self { all, faults }
    }

    /// The number a file name `NNNN-slug.md` gives; none for any other name.
    fn stem(name: &str) -> Option<Number> {
        let (digits, slug) = name.strip_suffix(".md")?.split_once('-')?;
        let slug = !slug.is_empty() && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if slug { Number::parse(digits).ok() } else { None }
    }
}

/// Every decision file has valid front matter, and supersession agrees with status.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fronts;

impl Check for Fronts {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) {
        let decisions = Decisions::read(corpus);
        faults.extend(decisions.faults);
        let known: BTreeSet<Number> = decisions.all.iter().map(|decision| decision.number).collect();
        let mut replaced = BTreeSet::new();
        for decision in &decisions.all {
            for number in &decision.front.supersedes {
                if known.contains(number) {
                    replaced.insert(*number);
                } else {
                    faults.push(Fault::Ghost { file: decision.file.clone(), number: *number });
                }
            }
        }
        for decision in &decisions.all {
            let superseded = decision.front.status == Status::Superseded;
            match (superseded, replaced.contains(&decision.number)) {
                (true, false) => faults.push(Fault::Orphan { number: decision.number }),
                (false, true) => faults.push(Fault::Replaced { number: decision.number }),
                _ => {}
            }
        }
    }
}

/// Every number up to the highest has a file, a reservation or a gap row with a reason, and
/// every reserved number is at its home.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gaps;

impl Check for Gaps {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) {
        let numbering = &corpus.config.decisions;
        let filed = corpus.filed();
        let numbers = corpus.numbers();
        if let Some(last) = numbers.last() {
            for number in Number::FIRST.through(*last) {
                let gapped = numbering.gap.iter().any(|gap| gap.first <= number && number <= gap.last);
                match (numbers.contains(&number), gapped) {
                    (false, false) => faults.push(Fault::Gap { number }),
                    (true, true) => faults.push(Fault::Mended { number }),
                    _ => {}
                }
            }
        }
        let dir = std::path::Path::new(Decisions::DIR);
        let home = Corpus::resolve(dir, &numbering.home).unwrap_or_else(|| PathBuf::from(&numbering.home));
        for reserved in &numbering.reserved {
            if filed.contains(&reserved.number) {
                faults.push(Fault::Mended { number: reserved.number });
            }
            let heading = format!("## {} ", reserved.number);
            let at_home = corpus.text(&home).is_some_and(|text| text.lines().any(|line| line.starts_with(&heading)));
            if !at_home {
                faults.push(Fault::Lost { number: reserved.number, home: home.clone() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::corpus::Doc;
    use crate::layout::config::{Gap, Reserved};

    fn file(number: &str, extra: &str) -> (String, String) {
        let text = format!(
            "---\nnumber: \"{number}\"\ntitle: \"T {number}\"\nstatus: accepted\ndate: 2026-11-26\narea: engine\nsupersedes: []\n{extra}---\n"
        );
        (format!("docs/decisions/{number}-t.md"), text)
    }

    fn corpus(files: &[(String, String)]) -> Corpus {
        let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        Corpus::sample(&borrowed)
    }

    fn found(check: &impl Check, corpus: &Corpus) -> Vec<String> {
        let mut faults = Vec::new();
        check.check(corpus, &mut faults);
        faults.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn valid_files_have_no_fault() {
        let files = [file("0001", ""), file("0002", "")];
        assert_eq!(found(&Fronts, &corpus(&files)), Vec::<String>::new());
    }

    #[test]
    fn a_bad_front_matter_a_wrong_number_and_a_wrong_name_are_faults() {
        let mut files = vec![file("0001", ""), file("0002", "")];
        files.push(("docs/decisions/0003-t.md".to_owned(), "no front matter".to_owned()));
        files.push(("docs/decisions/notes.md".to_owned(), String::new()));
        files.push(("docs/decisions/0004-t.md".to_owned(), file("0005", "").1));
        let found = found(&Fronts, &corpus(&files));
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.iter().any(|f| f.contains("0003-t.md") && f.contains("no front matter")));
        assert!(found.iter().any(|f| f.contains("notes.md") && f.contains("NNNN-slug.md")));
        assert!(found.iter().any(|f| f.contains("0004-t.md") && f.contains("0005")));
    }

    #[test]
    fn a_number_over_the_ceiling_is_a_fault() {
        let files = [file("0100", "")];
        let found = found(&Fronts, &corpus(&files));
        assert_eq!(found.len(), 1);
        assert!(found.first().is_some_and(|f| f.contains("decisions.ceiling")));
    }

    #[test]
    fn two_files_with_one_number_are_a_fault() {
        let mut files = vec![file("0001", "")];
        files.push(("docs/decisions/0001-other.md".to_owned(), file("0001", "").1));
        assert_eq!(found(&Fronts, &corpus(&files)), ["two decision files are numbered 0001"]);
    }

    #[test]
    fn supersession_agrees_with_status() {
        let second = file("0002", "").1.replace("supersedes: []", "supersedes: [\"0001\", \"0009\"]");
        let mut files = vec![file("0001", "")];
        files.push(("docs/decisions/0002-t.md".to_owned(), second));
        files.push(("docs/decisions/0003-t.md".to_owned(), file("0003", "").1.replace("accepted", "superseded")));
        let found = found(&Fronts, &corpus(&files));
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.iter().any(|f| f.contains("0009, which has no file")));
        assert!(found.iter().any(|f| f.contains("0001 is superseded by another decision")));
        assert!(found.iter().any(|f| f.contains("0003 has status superseded")));
    }

    fn gapped(files: &[(String, String)]) -> Corpus {
        let mut corpus = corpus(files);
        corpus.config.decisions.home = "../history/log.md".to_owned();
        corpus
    }

    #[test]
    fn a_gap_needs_a_row() {
        let files = [file("0001", ""), file("0004", "")];
        let mut corpus = gapped(&files);
        assert_eq!(
            found(&Gaps, &corpus),
            [
                "no decision 0002: no file, no reservation and no `decisions.gap` row with a reason in .config/layout.toml",
                "no decision 0003: no file, no reservation and no `decisions.gap` row with a reason in .config/layout.toml"
            ]
        );
        corpus.config.decisions.gap = vec![Gap::sample(2, 3), Gap::sample(4, 4)];
        assert_eq!(
            found(&Gaps, &corpus),
            ["a `decisions.gap` row names 0004, which now has a file or a reservation; remove it"]
        );
    }

    #[test]
    fn a_reserved_number_fills_its_gap_and_must_be_at_home() {
        let files = [file("0001", ""), file("0003", "")];
        let mut corpus = gapped(&files);
        corpus.config.decisions.reserved = vec![Reserved::sample(2, "A log")];
        assert_eq!(found(&Gaps, &corpus), ["0002 is reserved, but docs/history/log.md has no `## 0002` heading"]);
        let mut corpus = corpus.also("docs/history/log.md");
        corpus
            .files
            .push(Doc { path: PathBuf::from("docs/history/log.md"), text: "# Log\n\n## 0002 A log\n".to_owned() });
        assert_eq!(found(&Gaps, &corpus), Vec::<String>::new());
    }
}
