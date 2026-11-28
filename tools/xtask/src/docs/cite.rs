//! Every citation of a decision, in `.rs`, `.toml`, `.yml` and `.md` files, names a decision that
//! exists: a file in `docs/decisions/`, or a reserved number. A citation is `decision NNNN` (also
//! `decisions NNNN, NNNN and NNNN`), `(NNNN)` and `decisions/NNNN-slug.md`.

use super::Check;
use super::corpus::Corpus;
use super::decision::Decisions;
use super::fault::Fault;
use super::number::Number;

/// One citation found on a line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Cited {
    /// By number.
    Number(Number),
    /// By the name of its file, `NNNN-slug.md`.
    File(String),
}

impl Cited {
    /// The separators of a list of numbers.
    const JOINS: [&'static str; 3] = [", and ", ", ", " and "];

    /// Every citation on `line`. A bare `(NNNN)` is one only up to `ceiling`: above it the
    /// number is a file mode, a year or a count. A `decision NNNN` is one at any number, and
    /// is not when a date follows (`decision 2026-11-26`).
    pub(crate) fn scan(line: &str, ceiling: u16) -> Vec<Self> {
        let mut found = Vec::new();
        let lower = line.to_ascii_lowercase();
        for (at, word) in lower.match_indices("decision") {
            let alone =
                line.get(..at).and_then(|before| before.chars().next_back()).is_none_or(|c| !c.is_alphanumeric());
            let mut from = at + word.len();
            if line.get(from..).is_some_and(|rest| rest.starts_with('s')) {
                from += 1;
            }
            let Some(rest) = line.get(from..).and_then(|rest| rest.strip_prefix(' ')) else { continue };
            let skipped = usize::from(rest.starts_with('`'));
            let start = line.len() - rest.len() + skipped;
            if alone {
                found.extend(Self::list(line, start, true, ceiling).0.into_iter().map(Self::Number));
            }
        }
        for (at, _) in line.match_indices('(') {
            let (numbers, end) = Self::list(line, at + 1, false, ceiling);
            if !numbers.is_empty() && line.get(end..).is_some_and(|rest| rest.starts_with(')')) {
                found.extend(numbers.into_iter().map(Self::Number));
            }
        }
        for (at, word) in line.match_indices("decisions/") {
            let rest = line.get(at + word.len()..).unwrap_or_default();
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            let numbered = name.get(..Number::WIDTH).is_some_and(|digits| Number::parse(digits).is_ok())
                && name.get(Number::WIDTH..).is_some_and(|tail| tail.starts_with('-') && tail.len() > 1);
            if numbered && rest.get(name.len()..).is_some_and(|tail| tail.starts_with(".md")) {
                found.push(Self::File(format!("{name}.md")));
            }
        }
        found.sort();
        found.dedup();
        found
    }

    /// The numbers written from byte `at` of `line`, as `NNNN`, `NNNN, NNNN` or `NNNN and NNNN`,
    /// and where the last one ends. The first is read at any value when `free`; every other
    /// one only up to `ceiling`.
    fn list(line: &str, at: usize, free: bool, ceiling: u16) -> (Vec<Number>, usize) {
        let mut numbers = Vec::new();
        let mut end = at;
        let mut start = at;
        while let Some(number) = Self::digits(line, start) {
            if (!free || !numbers.is_empty()) && number.get() > ceiling {
                break;
            }
            numbers.push(number);
            end = start + Number::WIDTH;
            let tail = line.get(end..).unwrap_or_default();
            let Some(join) = Self::JOINS.iter().find(|join| tail.starts_with(**join)) else { break };
            start = end + join.len();
        }
        (numbers, end)
    }

    /// The number of the four digits at byte `at`, when they stand alone: no digit around them
    /// and no `-` and digit after, which would make them a year.
    fn digits(line: &str, at: usize) -> Option<Number> {
        let text = line.get(at..at + Number::WIDTH)?;
        let before = line.get(..at).and_then(|head| head.chars().next_back());
        let after = line.get(at + Number::WIDTH..).unwrap_or_default();
        let dated = after.strip_prefix('-').is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
        let joined = before.is_some_and(|c| c.is_ascii_digit()) || after.starts_with(|c: char| c.is_ascii_digit());
        if joined || dated { None } else { Number::parse(text).ok() }
    }
}

/// The citation check.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cites;

impl Check for Cites {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) {
        let numbers = corpus.numbers();
        let ceiling = corpus.config.decisions.ceiling;
        let dir = std::path::Path::new(Decisions::DIR);
        let quoting =
            |path: &std::path::Path| corpus.config.docs.quoted.iter().any(|quoted| path.starts_with(&quoted.path));
        for doc in corpus.files.iter().filter(|doc| !quoting(&doc.path)) {
            for (index, line) in doc.text.lines().enumerate() {
                for cited in Cited::scan(line, ceiling) {
                    let file = doc.path.clone();
                    match cited {
                        Cited::Number(number) if !numbers.contains(&number) => {
                            faults.push(Fault::Cite { file, line: index + 1, number });
                        }
                        Cited::File(name) if !corpus.entries.contains(&dir.join(&name)) => {
                            faults.push(Fault::Named { file, line: index + 1, name });
                        }
                        Cited::Number(_) | Cited::File(_) => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(line: &str) -> Vec<u16> {
        Cited::scan(line, 99)
            .into_iter()
            .filter_map(|cited| if let Cited::Number(number) = cited { Some(number.get()) } else { None })
            .collect()
    }

    #[test]
    fn the_keyword_form_reads_one_number_or_a_list() {
        assert_eq!(numbers("see decision 0042 for it"), [42]);
        assert_eq!(numbers("Decision `0042` too"), [42]);
        assert_eq!(numbers("decisions 0042, 0043 and 0044."), [42, 43, 44]);
        assert_eq!(numbers("Owner decision 2026-11-26."), Vec::<u16>::new());
        assert_eq!(numbers("an indecision 0042"), Vec::<u16>::new());
    }

    #[test]
    fn the_keyword_form_takes_a_number_over_the_ceiling_so_it_fails_to_resolve() {
        assert_eq!(numbers("decision 0700"), [700]);
    }

    #[test]
    fn the_paren_form_is_a_citation_only_up_to_the_ceiling() {
        assert_eq!(numbers("a thing (0047) and (0050, 0051)"), [47, 50, 51]);
        assert_eq!(numbers("mode (0700) and year (2026) and (0047 words)"), Vec::<u16>::new());
        assert_eq!(numbers("the count 0047 alone"), Vec::<u16>::new());
    }

    #[test]
    fn a_file_name_is_a_citation_by_name() {
        let found = Cited::scan("see docs/decisions/0042-the-slug.md and decisions/*.md", 99);
        assert_eq!(found, [Cited::File("0042-the-slug.md".to_owned())]);
        assert_eq!(Cited::scan("decisions/0042.md", 99), []);
    }

    #[test]
    fn a_citation_resolves_to_a_file_or_a_reservation() {
        let mut corpus = Corpus::sample(&[
            ("docs/decisions/0042-slug.md", ""),
            ("src/lib.rs", "// decision 0042, (0058) and (0043)\n"),
            ("docs/a.md", "see decisions/0042-slug.md and decisions/0042-gone.md\n"),
        ]);
        corpus.config.decisions.reserved = vec![crate::layout::config::Reserved::sample(58, "A log")];
        let mut faults = Vec::new();
        Cites.check(&corpus, &mut faults);
        let found: Vec<String> = faults.iter().map(ToString::to_string).collect();
        assert_eq!(
            found,
            [
                "src/lib.rs:1: cites decision 0043, which has no file and is not reserved",
                "docs/a.md:1: cites `decisions/0042-gone.md`, which is not a file of docs/decisions/"
            ]
        );
    }

    #[test]
    fn a_file_that_quotes_citations_is_left() {
        let mut corpus = Corpus::sample(&[("tool/tests.rs", "decision 0043\n"), ("src/a.rs", "decision 0043\n")]);
        corpus.config.docs.quoted = vec![crate::layout::config::Quoted::sample("tool")];
        let mut faults = Vec::new();
        Cites.check(&corpus, &mut faults);
        assert_eq!(faults.len(), 1);
    }
}
