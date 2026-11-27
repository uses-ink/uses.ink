//! Setting and clearing `publish` in a file's frontmatter, for `ink select`. The block is found
//! by the reader's own fences and parsed by the reader's own YAML, so the writer edits exactly
//! the block the reader reads, and every edit is read again before it is given back: a text that
//! would not read as the edit says is refused, never written.

use std::fmt;

use model::{Span, Spanned, Subject};

use super::{Keys, Yaml};
use crate::error::{Error, Report};
use crate::file::{Fence, File};

/// The `publish` key of one markdown file's frontmatter, as the file says it.
#[derive(Debug)]
pub struct Publishing<'a> {
    text: &'a str,
    /// Between the fences.
    yaml: Span,
    publish: Option<Spanned<bool>>,
}

/// What an edit did to a file's text: the key as it stood, and the new text when it changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Edit {
    /// `publish` before the edit; `None` when the frontmatter had no such key.
    pub was: Option<bool>,
    /// The file's text after the edit; `None` when it needed none.
    pub text: Option<String>,
}

/// A frontmatter block or a file that cannot be edited, with its cause when the reader has one.
#[derive(Debug)]
#[non_exhaustive]
pub enum Refusal {
    /// The file has no frontmatter block, so there is nowhere the key is known to belong.
    Missing,
    /// The file opens a block and never closes it.
    Unclosed {
        /// The opening `---` line.
        opening: Span,
    },
    /// The file is not UTF-8, or its YAML does not parse or is not a mapping of keys.
    Malformed(Malformed),
    /// `publish` holds a value that is neither `true` nor `false`.
    Flag {
        /// The value.
        span: Span,
    },
    /// The key is written in a form a line edit cannot remove, such as inside `{ ... }`.
    Inline {
        /// The value.
        span: Span,
    },
    /// The edit did not read back as it was meant: nothing was changed.
    Misplaced,
}

/// Why the reader refused a file's frontmatter, as the reader says it.
#[derive(Debug)]
pub struct Malformed(Error);

impl fmt::Display for Malformed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, out)
    }
}

impl std::error::Error for Malformed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.0)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => out.write_str("the file has no frontmatter block (`---` lines) to hold `publish`"),
            Self::Unclosed { .. } => out.write_str("the frontmatter opens with `---` and never closes"),
            Self::Malformed(_) => out.write_str("the frontmatter cannot be read"),
            Self::Flag { .. } => out.write_str("`publish` is neither `true` nor `false`"),
            Self::Inline { .. } => out.write_str("`publish` is written in a form that is not one line of its own"),
            Self::Misplaced => out.write_str("the edit would not read back as `publish`, so nothing was changed"),
        }
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Malformed(source) => Some(source),
            _ => None,
        }
    }
}

impl<'a> Publishing<'a> {
    /// The `publish` key of the file `bytes`.
    ///
    /// # Errors
    ///
    /// [`Refusal`] when the file has no readable, closed frontmatter block, or `publish` holds
    /// something other than a flag.
    pub fn of(bytes: &'a [u8]) -> Result<Self, Refusal> {
        let file = File::try_from(bytes).map_err(|encoding| Refusal::Malformed(Malformed(encoding.into())))?;
        let yaml = match file.front {
            None => return Err(Refusal::Missing),
            Some(Fence::Unclosed { opening }) => return Err(Refusal::Unclosed { opening }),
            Some(Fence::Closed { yaml, .. }) => yaml,
        };
        let block = Yaml::between(file.text, yaml);
        let entries = block.entries().map_err(|refused| Refusal::Malformed(Malformed(refused.0)))?;
        // the subject only labels a report this does not read
        let mut report = Report::new(Subject::Site);
        let publish = Yaml::read::<Keys>(&file, &mut report).publish;
        if publish.is_none()
            && let Some((_, node)) = entries.iter().flat_map(|entries| &entries.0).find(|(key, _)| key == "publish")
            && !node.value.is_null()
        {
            return Err(Refusal::Flag { span: block.place(node.referenced).unwrap_or(yaml) });
        }
        Ok(Self { text: file.text, yaml, publish })
    }

    /// `publish: true`, replacing a `false` in place or adding the key as the last of the block.
    ///
    /// # Errors
    ///
    /// [`Refusal::Misplaced`] when the edited text does not read back as selected.
    pub fn select(&self) -> Result<Edit, Refusal> {
        let was = self.publish.as_ref().map(|publish| publish.value);
        let text = match &self.publish {
            Some(publish) if publish.value => return Ok(Edit { was, text: None }),
            Some(publish) => self.replace(publish.span, "true"),
            None => self.insert(),
        };
        Self::verified(was, text, Some(true))
    }

    /// Removes a `publish: true` line, leaving everything else. A `publish: false` is not a
    /// selection and stays.
    ///
    /// # Errors
    ///
    /// [`Refusal::Inline`] when the key is not a line of its own, [`Refusal::Misplaced`] when the
    /// edited text does not read back without it.
    pub fn clear(&self) -> Result<Edit, Refusal> {
        let was = self.publish.as_ref().map(|publish| publish.value);
        let Some(publish) = self.publish.as_ref().filter(|publish| publish.value) else {
            return Ok(Edit { was, text: None });
        };
        let line = Line::around(self.text, publish.span).ok_or(Refusal::Inline { span: publish.span })?;
        let text = format!(
            "{}{}",
            self.text.get(..line.start).unwrap_or_default(),
            self.text.get(line.end..).unwrap_or_default()
        );
        Self::verified(was, text, None)
    }

    fn replace(&self, span: Span, with: &str) -> String {
        let (before, after) = (self.text.get(..span.start()), self.text.get(span.end()..));
        format!("{}{with}{}", before.unwrap_or_default(), after.unwrap_or_default())
    }

    /// A `publish: true` line before the closing fence, in the block's own line ending.
    fn insert(&self) -> String {
        let ending = if self.text.find('\n').and_then(|at| self.text.get(..at)).is_some_and(|line| line.ends_with('\r'))
        {
            "\r\n"
        } else {
            "\n"
        };
        let at = self.yaml.end();
        format!(
            "{}publish: true{ending}{}",
            self.text.get(..at).unwrap_or_default(),
            self.text.get(at..).unwrap_or_default()
        )
    }

    /// `text`, when it reads back with `publish` as `expected`.
    fn verified(was: Option<bool>, text: String, expected: Option<bool>) -> Result<Edit, Refusal> {
        match Publishing::of(text.as_bytes()) {
            Ok(back) if back.publish.as_ref().map(|publish| publish.value) == expected => {
                Ok(Edit { was, text: Some(text) })
            }
            _ => Err(Refusal::Misplaced),
        }
    }
}

/// The whole line holding a `publish` key's value, when the line is that key and nothing else.
#[derive(Clone, Copy, Debug)]
struct Line {
    start: usize,
    end: usize,
}

impl Line {
    /// The line around `value`: from its start to the line ending after it, the ending included.
    fn around(text: &str, value: Span) -> Option<Self> {
        let start = text.get(..value.start())?.rfind('\n').map_or(0, |at| at + 1);
        let key = text.get(start..value.start())?;
        let named = key.trim_start().trim_end_matches([' ', '\t']).strip_suffix(':')?.trim_end_matches([' ', '\t']);
        if !matches!(named, "publish" | "\"publish\"" | "'publish'") {
            return None;
        }
        let end = text.get(value.end()..)?.find('\n').map_or(text.len(), |at| value.end() + at + 1);
        Some(Self { start, end })
    }
}
