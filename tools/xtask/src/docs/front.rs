//! The front matter of a decision file: `number`, `title`, `status`, `date`, `area` and
//! `supersedes`, between two `---` lines. Each value is read into a type; every defect is kept.

use std::error::Error;
use std::fmt;

use super::number::{Number, Refusal};

/// Where a decision stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Proposed,
    Accepted,
    Built,
    Superseded,
}

impl Status {
    /// Every status, as the front matter spells it.
    const ALL: [Self; 4] = [Self::Proposed, Self::Accepted, Self::Built, Self::Superseded];

    const fn name(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Built => "built",
            Self::Superseded => "superseded",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.name() == text)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A calendar day, as `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// Reads `YYYY-MM-DD`, a day that exists.
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('-');
        let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
        let shaped = parts.next().is_none()
            && [(year, 4), (month, 2), (day, 2)]
                .iter()
                .all(|(part, width)| part.len() == *width && part.bytes().all(|byte| byte.is_ascii_digit()));
        if !shaped {
            return None;
        }
        let (year, month, day) = (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
        (1..=Self::days(year, month)).contains(&day).then_some(Self { year, month, day })
    }

    /// How many days the month has; none for a month that does not exist.
    const fn days(year: u16, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => 29,
            2 => 28,
            _ => 0,
        }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// The front matter of one decision, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Front {
    pub(crate) number: Number,
    pub(crate) title: String,
    pub(crate) status: Status,
    pub(crate) date: Date,
    /// A free label: a lowercase slug, so the set of areas follows the work and a typo shows.
    pub(crate) area: String,
    pub(crate) supersedes: Vec<Number>,
}

/// One thing wrong with a front matter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Defect {
    /// The file does not start with a `---` line.
    Open,
    /// No closing `---` line.
    Unclosed,
    /// A line that is no `key: value`.
    Line { number: usize, text: String },
    /// A key that is not one of the six.
    Unknown { key: String },
    /// A key given twice.
    Twice { key: String },
    /// One of the six missing.
    Absent { key: &'static str },
    /// A value that should be in double quotes.
    Unquoted { key: &'static str },
    /// A quoted value with an escape other than `\"` and `\\`, or a bare quote.
    Escape { key: &'static str },
    /// A value that is empty.
    Empty { key: &'static str },
    /// A number that is not one.
    Number { source: Refusal },
    /// A `supersedes` that is not a list of quoted numbers.
    List { key: &'static str },
    /// A status that is none of the four.
    Status { given: String },
    /// A date that is not a day of the calendar.
    Date { given: String },
    /// An area that is not a lowercase slug.
    Area { given: String },
}

impl fmt::Display for Defect {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(out, "no front matter: the file must start with a `---` line"),
            Self::Unclosed => write!(out, "the front matter is not closed by a `---` line"),
            Self::Line { number, text } => write!(out, "line {number} is no `key: value`: `{text}`"),
            Self::Unknown { key } => write!(out, "unknown key `{key}`; the keys are {}", Front::KEYS.join(", ")),
            Self::Twice { key } => write!(out, "`{key}` is given twice"),
            Self::Absent { key } => write!(out, "`{key}` is missing"),
            Self::Unquoted { key } => write!(out, "`{key}` must be in double quotes"),
            Self::Escape { key } => write!(out, "`{key}` has an escape other than `\\\"` and `\\\\`"),
            Self::Empty { key } => write!(out, "`{key}` is empty"),
            Self::Number { source } => write!(out, "{source}"),
            Self::List { key } => write!(out, "`{key}` must be a list of quoted numbers, like `[\"0012\"]` or `[]`"),
            Self::Status { given } => {
                let all: Vec<&str> = Status::ALL.iter().map(|status| status.name()).collect();
                write!(out, "status `{given}` is none of {}", all.join(", "))
            }
            Self::Date { given } => write!(out, "date `{given}` is not a day as `YYYY-MM-DD`"),
            Self::Area { given } => write!(out, "area `{given}` is not a lowercase slug (letters, digits, `-`)"),
        }
    }
}

impl Error for Defect {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Number { source } = self { Some(source) } else { None }
    }
}

impl Front {
    /// The six keys, in the order the files write them.
    pub(crate) const KEYS: [&'static str; 6] = ["number", "title", "status", "date", "area", "supersedes"];

    /// Reads the front matter at the start of `text`; every defect is returned, not the first.
    pub(crate) fn parse(text: &str) -> Result<Self, Vec<Defect>> {
        let mut lines = text.lines();
        if lines.next() != Some("---") {
            return Err(vec![Defect::Open]);
        }
        let mut defects = Vec::new();
        let pairs = Self::pairs(lines, &mut defects);
        let number = Self::take(&pairs, "number", Self::number, &mut defects);
        let title = Self::take(&pairs, "title", Self::title, &mut defects);
        let status = Self::take(&pairs, "status", Self::status, &mut defects);
        let date = Self::take(&pairs, "date", Self::date, &mut defects);
        let area = Self::take(&pairs, "area", Self::area, &mut defects);
        let supersedes = Self::take(&pairs, "supersedes", Self::list, &mut defects);
        match (number, title, status, date, area, supersedes) {
            (Some(number), Some(title), Some(status), Some(date), Some(area), Some(supersedes))
                if defects.is_empty() =>
            {
                Ok(Self { number, title, status, date, area, supersedes })
            }
            _ => Err(defects),
        }
    }

    /// The `key: value` lines up to the closing `---`; every line that is not one goes into
    /// `defects`.
    fn pairs<'a>(lines: impl Iterator<Item = &'a str>, defects: &mut Vec<Defect>) -> Vec<(&'a str, &'a str)> {
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        for (index, line) in lines.enumerate() {
            if line == "---" {
                return pairs;
            }
            let Some((key, value)) = line.split_once(':') else {
                defects.push(Defect::Line { number: index + 2, text: line.to_owned() });
                continue;
            };
            let key = key.trim();
            if !Self::KEYS.contains(&key) {
                defects.push(Defect::Unknown { key: key.to_owned() });
            } else if pairs.iter().any(|(known, _)| *known == key) {
                defects.push(Defect::Twice { key: key.to_owned() });
            } else {
                pairs.push((key, value.trim()));
            }
        }
        defects.push(Defect::Unclosed);
        pairs
    }

    /// The value of `key` read by `parse`; a missing key or a bad value goes into `defects`.
    fn take<T>(
        pairs: &[(&str, &str)],
        key: &'static str,
        parse: impl FnOnce(&'static str, &str) -> Result<T, Defect>,
        defects: &mut Vec<Defect>,
    ) -> Option<T> {
        let Some((_, raw)) = pairs.iter().find(|(known, _)| *known == key) else {
            defects.push(Defect::Absent { key });
            return None;
        };
        match parse(key, raw) {
            Ok(value) => Some(value),
            Err(defect) => {
                defects.push(defect);
                None
            }
        }
    }

    /// A double-quoted value, unescaped.
    fn quoted(key: &'static str, raw: &str) -> Result<String, Defect> {
        let inner = raw.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).ok_or(Defect::Unquoted { key })?;
        let mut text = String::new();
        let mut chars = inner.chars();
        while let Some(found) = chars.next() {
            match found {
                '\\' => match chars.next() {
                    Some(escaped @ ('"' | '\\')) => text.push(escaped),
                    _ => return Err(Defect::Escape { key }),
                },
                '"' => return Err(Defect::Escape { key }),
                other => text.push(other),
            }
        }
        Ok(text)
    }

    fn number(key: &'static str, raw: &str) -> Result<Number, Defect> {
        Number::parse(&Self::quoted(key, raw)?).map_err(|source| Defect::Number { source })
    }

    fn title(key: &'static str, raw: &str) -> Result<String, Defect> {
        let title = Self::quoted(key, raw)?;
        if title.trim().is_empty() { Err(Defect::Empty { key }) } else { Ok(title) }
    }

    fn status(_key: &'static str, raw: &str) -> Result<Status, Defect> {
        Status::parse(raw).ok_or_else(|| Defect::Status { given: raw.to_owned() })
    }

    fn date(_key: &'static str, raw: &str) -> Result<Date, Defect> {
        Date::parse(raw).ok_or_else(|| Defect::Date { given: raw.to_owned() })
    }

    fn area(_key: &'static str, raw: &str) -> Result<String, Defect> {
        let mut chars = raw.chars();
        let slug = chars.next().is_some_and(|first| first.is_ascii_lowercase())
            && chars.all(|next| next.is_ascii_lowercase() || next.is_ascii_digit() || next == '-');
        if slug { Ok(raw.to_owned()) } else { Err(Defect::Area { given: raw.to_owned() }) }
    }

    fn list(key: &'static str, raw: &str) -> Result<Vec<Number>, Defect> {
        let inner = raw.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).ok_or(Defect::List { key })?;
        if inner.trim().is_empty() {
            return Ok(Vec::new());
        }
        inner.split(',').map(|item| Self::number(key, item.trim())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(number: &str, extra: &str) -> String {
        format!(
            "---\nnumber: \"{number}\"\ntitle: \"A title, with \\\"quotes\\\"\"\nstatus: accepted\ndate: 2026-11-26\narea: engine\nsupersedes: []\n{extra}---\n\nbody\n"
        )
    }

    #[test]
    fn a_valid_front_matter_is_read_into_types() {
        let front = Front::parse(&sample("0042", "")).unwrap();
        assert_eq!(front.number, Number::sample(42));
        assert_eq!(front.title, "A title, with \"quotes\"");
        assert_eq!(front.status, Status::Accepted);
        assert_eq!(front.date.to_string(), "2026-11-26");
        assert_eq!(front.area, "engine");
        assert_eq!(front.supersedes, []);
    }

    #[test]
    fn supersedes_is_a_list_of_numbers() {
        let text = sample("0042", "").replace("supersedes: []", "supersedes: [\"0012\", \"0013\"]");
        assert_eq!(Front::parse(&text).unwrap().supersedes, [Number::sample(12), Number::sample(13)]);
    }

    #[test]
    fn a_file_without_front_matter_is_refused() {
        assert_eq!(Front::parse("# title\n"), Err(vec![Defect::Open]));
    }

    #[test]
    fn every_defect_is_kept() {
        let text = "---\nnumber: 0042\nstatus: done\ndate: 2026-02-30\narea: Engine\nsupersedes: 3\nowner: me\n";
        let Err(defects) = Front::parse(text) else { panic!("accepted") };
        assert_eq!(
            defects,
            [
                Defect::Unknown { key: "owner".to_owned() },
                Defect::Unclosed,
                Defect::Unquoted { key: "number" },
                Defect::Absent { key: "title" },
                Defect::Status { given: "done".to_owned() },
                Defect::Date { given: "2026-02-30".to_owned() },
                Defect::Area { given: "Engine".to_owned() },
                Defect::List { key: "supersedes" },
            ]
        );
    }

    #[test]
    fn a_key_twice_and_a_line_without_a_key_are_defects() {
        let text = sample("0042", "area: web\nnot a pair\n");
        let Err(defects) = Front::parse(&text) else { panic!("accepted") };
        assert_eq!(
            defects,
            [Defect::Twice { key: "area".to_owned() }, Defect::Line { number: 9, text: "not a pair".to_owned() }]
        );
    }

    #[test]
    fn a_number_defect_has_its_refusal_as_source() {
        let Err(defects) = Front::parse(&sample("42", "")) else { panic!("accepted") };
        let [defect] = defects.as_slice() else { panic!("{defects:?}") };
        assert_eq!(defect.source().map(ToString::to_string), Some("`42` is not four digits".to_owned()));
    }

    #[test]
    fn dates_are_days_of_the_calendar() {
        assert!(Date::parse("2024-02-29").is_some());
        assert!(Date::parse("2026-02-29").is_none());
        assert!(Date::parse("2026-13-01").is_none());
        assert!(Date::parse("2026-1-01").is_none());
        assert!(Date::parse("2026-11-26-1").is_none());
    }

    #[test]
    fn an_empty_title_and_a_stray_escape_are_refused() {
        assert_eq!(Front::title("title", "\"\""), Err(Defect::Empty { key: "title" }));
        assert_eq!(Front::title("title", "\"a\\nb\""), Err(Defect::Escape { key: "title" }));
        assert_eq!(Front::title("title", "\"a\"b\""), Err(Defect::Escape { key: "title" }));
    }
}
