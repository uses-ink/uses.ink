//! What a file's name says: a date prefix (`2024-03-01-hello.md`), a title, whether it is a
//! folder's index.

use jiff::civil::Date;
use model::ContentPath;

/// The length of `YYYY-MM-DD`.
const DATE: usize = 10;

/// Stems that make a file its folder's index.
const INDEX: &[&str] = &["index", "readme"];

/// A file's stem, read for what it says.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Name<'a> {
    stem: &'a str,
}

impl<'a> From<&'a ContentPath> for Name<'a> {
    fn from(path: &'a ContentPath) -> Self {
        Self { stem: path.stem() }
    }
}

impl<'a> Name<'a> {
    /// The date a stem starts with, `YYYY-MM-DD` then the end, `-` or `_`.
    pub(crate) fn date(self) -> Option<Date> {
        let head = self.stem.get(..DATE)?;
        let tail = self.stem.get(DATE..)?;
        if !(tail.is_empty() || tail.starts_with(['-', '_'])) {
            return None;
        }
        head.parse().ok()
    }

    /// The stem after its date prefix, or the whole stem when nothing follows the date.
    pub(crate) fn rest(self) -> &'a str {
        let rest = match self.date() {
            Some(_) => self.stem.get(DATE..).unwrap_or(self.stem).trim_start_matches(['-', '_']),
            None => self.stem,
        };
        if rest.is_empty() { self.stem } else { rest }
    }

    /// The title a name gives: hyphens and underscores to spaces, the first letter capitalised.
    /// `2024-03-01-on_shelves` is `On shelves`.
    pub(crate) fn title(self) -> String {
        let words: Vec<&str> = self.rest().split(['-', '_', ' ']).filter(|w| !w.is_empty()).collect();
        let text = if words.is_empty() { self.rest().to_owned() } else { words.join(" ") };
        let mut chars = text.chars();
        chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or(text)
    }

    /// Whether the file is its folder's index: `index` or `README`, in any case.
    pub(crate) fn is_index(self) -> bool {
        INDEX.iter().any(|index| self.stem.eq_ignore_ascii_case(index))
    }
}

#[cfg(test)]
mod test {
    use jiff::civil::date;

    use super::*;

    fn name(path: &str) -> (Option<Date>, String, String) {
        let path = ContentPath::parse(path).unwrap();
        let name = Name::from(&path);
        (name.date(), name.rest().to_owned(), name.title())
    }

    #[test]
    fn reads_dates_and_titles() {
        assert_eq!(
            name("posts/2024-03-01-on_shelves.md"),
            (Some(date(2024, 3, 1)), "on_shelves".into(), "On shelves".into())
        );
        assert_eq!(name("2024-03-01.md"), (Some(date(2024, 3, 1)), "2024-03-01".into(), "2024 03 01".into()));
        assert_eq!(name("2024-13-01-x.md").0, None);
        assert_eq!(name("2024-03-01x.md").0, None);
        assert_eq!(name("été.md").2, "Été");
        assert!(Name::from(&ContentPath::parse("a/README.md").unwrap()).is_index());
    }
}
