//! The YAML frontmatter block, read into [`Front`]. serde-saphyr parses the YAML into nodes with
//! their places; each node becomes the model's [`Raw`], and each field of [`Keys`] reads its
//! keys' values through its value type's [`FromFront`]. Nothing here knows what a title or a
//! date is.

mod edit;
mod field;
mod node;

use model::code::{Date, Kind, Publish, Slug, Summary, Tags, Title, Updated};
use model::{FromFront, Front, Raw, Span, Spanned};
use serde_saphyr::Spanned as Located;

use crate::error::{Error, Report};
use crate::file::{Fence, File};
use field::{Aliases, Draft, Field, Lang};
use node::{Entries, Node};

pub use edit::{Edit, Malformed, Publishing, Refusal};

/// The fields the engine reads, each a [`Field`]. Every other key is the author's own and is
/// neither kept nor refused.
pub(crate) type Keys = (Title, Date, Updated, Slug, Tags, Publish, Draft, Kind, Summary, Lang);

/// [`Keys`] and `aliases`, for a dialect that names pages by them.
pub(crate) type Aliased = (Title, Date, Updated, Slug, Tags, Publish, Draft, Kind, Summary, Lang, Aliases);

/// The YAML of one frontmatter block, in its file.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Yaml<'a> {
    file: &'a str,
    /// Where the YAML is, between the fences.
    region: Span,
}

impl<'a> Yaml<'a> {
    /// The frontmatter of `file` read into the fields `F`, every problem into `report`.
    pub(crate) fn read<F: Fields>(file: &File<'a>, report: &mut Report) -> Front {
        let mut front = Front::default();
        match file.front {
            None => {}
            Some(Fence::Unclosed { opening }) => report.refuse(Error::Unclosed, opening),
            Some(Fence::Closed { yaml, block }) => {
                front.block = Some(block);
                Self { file: file.text, region: yaml }.fill::<F>(&mut front, report);
            }
        }
        front
    }

    /// Parses the YAML and fills `front` with the fields `F`. A block that does not parse is
    /// one diagnostic and fills nothing; refused values are reported in the order written.
    fn fill<F: Fields>(self, front: &mut Front, report: &mut Report) {
        let entries = match self.entries() {
            Ok(Some(entries)) => entries,
            // an empty block: no keys
            Ok(None) => return,
            Err(refused) => return report.refuse(refused.0, refused.1),
        };
        let mut refused = Refused::default();
        F::fill(&Reading { yaml: self, entries: &entries }, front, &mut refused);
        refused.0.sort_by_key(|(_, span)| span.start());
        for (error, span) in refused.0 {
            report.refuse(error, span);
        }
    }

    /// The keys of the block, `None` for an empty one; the error and its place when the YAML does
    /// not parse or is not a mapping of keys.
    fn entries(self) -> Result<Option<Entries>, Box<(Error, Span)>> {
        // the fences cut at line ends, which are character boundaries
        let Some(yaml) = self.file.get(self.region.range()) else { return Ok(None) };
        serde_saphyr::from_str::<Option<Entries>>(yaml).map_err(|source| {
            let span = source.location().and_then(|at| self.place(at)).unwrap_or(self.region);
            let before = self.file.get(..self.region.start()).map_or(0, |before| before.matches('\n').count());
            Box::new((Error::Yaml { source, before: before as u64 }, span))
        })
    }

    /// The block of `file` between its fences, when it has a closed one.
    const fn between(file: &'a str, region: Span) -> Self {
        Self { file, region }
    }

    /// A YAML location as a span of the file.
    fn place(self, at: serde_saphyr::Location) -> Option<Span> {
        let span = at.span();
        let start = usize::try_from(span.byte_offset()?).ok()?.checked_add(self.region.start())?;
        let end = start.checked_add(usize::try_from(span.byte_len()?).ok()?)?;
        Span::new(start, end.min(self.region.end())).ok()
    }

    /// A node as the model's [`Raw`], each part with its span; a number or a boolean keeps the
    /// text written for it. A node the parser cannot place is placed at the whole block.
    fn raw(self, node: &'a Located<Node>) -> Raw<'a> {
        let span = self.place(node.referenced).unwrap_or(self.region);
        // a span the parser gave lies on character boundaries inside the block
        let written = self.file.get(span.range()).unwrap_or_default();
        match &node.value {
            Node::Null => Raw::Null { span },
            Node::Bool(value) => Raw::Bool { value: *value, written, span },
            Node::Number => Raw::Number { written, span },
            Node::Text(text) => Raw::Text { value: text.as_str().into(), span },
            Node::Map => Raw::Map { span },
            Node::List(items) => {
                let items: Vec<Raw<'a>> = items.iter().map(|item| self.raw(item)).collect();
                let end = items.iter().map(|item| item.span().end()).max().unwrap_or(span.end());
                let span = Span::new(span.start(), end.max(span.end())).unwrap_or(span);
                Raw::List { items, span }
            }
        }
    }
}

/// The entries of one parsed block, being read field by field.
#[derive(Debug)]
pub(crate) struct Reading<'a> {
    yaml: Yaml<'a>,
    entries: &'a Entries,
}

/// What the fields refused, each at its place, before they are put in the order written.
#[derive(Debug, Default)]
pub(crate) struct Refused(Vec<(Error, Span)>);

impl Reading<'_> {
    /// Reads the field `F`: each of its keys that is set and not null, through `F::Value`. When
    /// more than one key holds a value, the first written is kept and each later one is a
    /// conflict.
    fn field<F: Field>(&self, front: &mut Front, refused: &mut Refused) {
        let mut held: Vec<(&'static str, Spanned<F::Value>)> = Vec::new();
        for &key in F::KEYS {
            for (_, node) in self.entries.0.iter().filter(|(name, _)| name == key) {
                let raw = self.yaml.raw(node);
                if let Raw::Null { .. } = raw {
                    continue;
                }
                let span = raw.span();
                match F::Value::read(&raw) {
                    Ok(value) => held.push((key, Spanned::new(value, span))),
                    Err(refusal) => {
                        for source in refusal {
                            let span = source.span;
                            refused.0.push((Error::Value { key, source }, span));
                        }
                    }
                }
            }
        }
        held.sort_by_key(|(_, value)| value.span.start());
        let mut held = held.into_iter();
        let Some((first, kept)) = held.next() else { return };
        for (key, ignored) in held {
            refused.0.push((Error::Conflict { key, other: first }, ignored.span));
        }
        *F::slot(front) = Some(kept);
    }
}

/// A set of [`Field`]s read together: a tuple of them, read in order.
pub(crate) trait Fields {
    /// Reads every field of the set from `reading` into `front`.
    fn fill(reading: &Reading<'_>, front: &mut Front, refused: &mut Refused);
}

/// Implements [`Fields`] for a tuple of each arity up to the number of names given, reading
/// its fields in order:
///
/// ```text
/// fields!(A, B, C);
/// // implements `Fields` for `(A,)`, `(A, B)` and `(A, B, C)`
/// ```
macro_rules! fields {
    ($first:ident $(, $rest:ident)*) => {
        impl<$first: Field $(, $rest: Field)*> Fields for ($first, $($rest,)*) {
            fn fill(reading: &Reading<'_>, front: &mut Front, refused: &mut Refused) {
                reading.field::<$first>(front, refused);
                $(reading.field::<$rest>(front, refused);)*
            }
        }
        fields!($($rest),*);
    };
    () => {};
}

fields!(A, B, C, D, E, F, G, H, I, J, K, L);

#[cfg(test)]
mod test {
    use model::{ContentPath, Front, Problem, Spanned, Subject};

    use super::{Field, Yaml};
    use crate::error::Report;
    use crate::file::File;

    /// A stand-in field set by two keys.
    struct Name;

    impl Field for Name {
        const KEYS: &'static [&'static str] = &["name", "alias"];
        type Value = String;
        fn slot(front: &mut Front) -> &mut Option<Spanned<Self::Value>> {
            &mut front.title
        }
    }

    fn read(text: &str) -> (Front, Vec<Problem>) {
        let mut report = Report::new(Subject::File(ContentPath::parse("a.md").unwrap()));
        let file = File::try_from(text.as_bytes()).unwrap();
        let front = Yaml::read::<(Name,)>(&file, &mut report);
        (front, report.diagnostics.into_iter().map(|diagnostic| diagnostic.problem).collect())
    }

    #[test]
    fn the_first_key_written_wins_and_later_ones_conflict() {
        let (front, problems) = read("---\nalias: second\nother: [x]\nname: third\n---\n");
        assert_eq!(front.title.map(|title| title.value).as_deref(), Some("second"));
        let conflict = Problem::FrontmatterKeyConflict { key: "name".to_owned(), other: "alias".to_owned() };
        assert_eq!(problems, [conflict]);
    }

    #[test]
    fn a_refused_key_leaves_the_other_one() {
        let (front, problems) = read("---\nname: [not, text]\nalias: kept\n---\n");
        assert_eq!(front.title.map(|title| title.value).as_deref(), Some("kept"));
        let refused = Problem::FrontmatterValueInvalid { key: "name".to_owned(), expected: "some text".to_owned() };
        assert_eq!(problems, [refused]);
    }
}
