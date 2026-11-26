//! Frontmatter values as YAML gives them, and their conversion into the types `Front` holds.

use std::fmt;

use jiff::civil::Date;
use model::{Diagnostic, PageKind, Problem, Slug, Span, Spanned, Tag, Timestamp, When};
use serde::Deserialize;
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_saphyr::Spanned as Located;

use super::Source;

/// A YAML value, kept loose so one bad value does not fail the whole block. A number or a
/// boolean is read back from the text as written when a key wants text.
#[derive(Debug)]
pub(super) enum Raw {
    Null,
    Bool(bool),
    Number,
    Text(String),
    List(Vec<Located<Raw>>),
    Map,
}

impl<'de> Deserialize<'de> for Raw {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(RawVisitor)
    }
}

struct RawVisitor;

impl<'de> Visitor<'de> for RawVisitor {
    type Value = Raw;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("any YAML value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Raw, E> {
        Ok(Raw::Bool(value))
    }

    fn visit_i64<E>(self, _: i64) -> Result<Raw, E> {
        Ok(Raw::Number)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Raw, E> {
        Ok(Raw::Number)
    }

    fn visit_i128<E>(self, _: i128) -> Result<Raw, E> {
        Ok(Raw::Number)
    }

    fn visit_u128<E>(self, _: u128) -> Result<Raw, E> {
        Ok(Raw::Number)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Raw, E> {
        Ok(Raw::Number)
    }

    fn visit_str<E>(self, value: &str) -> Result<Raw, E> {
        Ok(Raw::Text(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Raw, E> {
        Ok(Raw::Text(value))
    }

    fn visit_unit<E>(self) -> Result<Raw, E> {
        Ok(Raw::Null)
    }

    fn visit_none<E>(self) -> Result<Raw, E> {
        Ok(Raw::Null)
    }

    fn visit_some<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Raw, D::Error> {
        Raw::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Raw, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Located<Raw>>()? {
            items.push(item);
        }
        Ok(Raw::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Raw, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Raw::Map)
    }
}

/// A key the engine reads.
#[derive(Clone, Copy, Debug)]
pub(super) enum Field {
    Title,
    Date,
    Updated,
    Slug,
    Tags,
    Publish,
    Draft,
    Kind,
}

impl Field {
    const fn key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Date => "date",
            Self::Updated => "updated",
            Self::Slug => "slug",
            Self::Tags => "tags",
            Self::Publish => "publish",
            Self::Draft => "draft",
            Self::Kind => "type",
        }
    }
}

/// What a key takes, as the `expected` of `frontmatter.value.invalid`.
#[derive(Clone, Copy, Debug)]
struct Expected(Field);

impl fmt::Display for Expected {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Field::Title => "some text",
            Field::Date | Field::Updated => {
                "a day (`2024-03-01`) or a time with its offset (`2024-03-01T09:30:00+01:00`)"
            }
            Field::Slug => "a slug: lower-case letters and digits, joined by single `-`",
            Field::Tags => "a list of tags, or tags separated by commas, each with a letter or digit",
            Field::Publish | Field::Draft => "`true` or `false`",
            Field::Kind => "`post` or `page`",
        })
    }
}

/// One value and the text written for it.
#[derive(Debug)]
pub(super) struct Value<'a> {
    raw: &'a Raw,
    written: Option<&'a str>,
}

impl Value<'_> {
    /// The value as text: a string, or a number or boolean as written.
    fn text(&self) -> Option<&str> {
        match self.raw {
            Raw::Text(text) => Some(text),
            Raw::Bool(_) | Raw::Number => self.written.map(str::trim),
            Raw::Null | Raw::List(_) | Raw::Map => None,
        }
    }
}

pub(super) fn title(value: &Value<'_>) -> Option<String> {
    value.text().filter(|text| !text.trim().is_empty()).map(str::to_owned)
}

pub(super) fn when(value: &Value<'_>) -> Option<When> {
    let text = value.text()?.trim();
    // A bare day is exactly `YYYY-MM-DD`; jiff would also read a day out of a longer text.
    if text.len() == "2024-03-01".len() {
        return text.parse::<Date>().ok().map(When::Day);
    }
    text.parse::<Timestamp>().ok().map(When::Instant)
}

pub(super) fn slug(value: &Value<'_>) -> Option<Slug> {
    Slug::parse(value.text()?.trim()).ok()
}

pub(super) fn flag(value: &Value<'_>) -> Option<bool> {
    match value.raw {
        Raw::Bool(flag) => Some(*flag),
        _ => None,
    }
}

pub(super) fn kind(value: &Value<'_>) -> Option<PageKind> {
    let text = value.text()?.trim();
    if text.eq_ignore_ascii_case("post") {
        Some(PageKind::Post)
    } else if text.eq_ignore_ascii_case("page") {
        Some(PageKind::Page)
    } else {
        None
    }
}

/// Converted values out, refused values into diagnostics.
#[derive(Debug)]
pub(super) struct Out<'a> {
    pub(super) source: &'a Source<'a>,
    pub(super) diagnostics: &'a mut Vec<Diagnostic>,
}

impl Out<'_> {
    pub(super) fn convert<T>(
        &mut self,
        field: Field,
        raw: &Located<Raw>,
        convert: fn(&Value<'_>) -> Option<T>,
    ) -> Option<Spanned<T>> {
        if matches!(raw.value, Raw::Null) {
            return None;
        }
        let span = self.span(raw);
        let value = Value { raw: &raw.value, written: self.source.written(span) };
        let converted = convert(&value);
        if converted.is_none() {
            self.refuse(field, span);
        }
        converted.map(|converted| Spanned::new(converted, span))
    }

    /// `tags`: a list, or one text of tags separated by commas. A bad tag is refused alone.
    pub(super) fn tags(&mut self, raw: &Located<Raw>) -> Option<Spanned<Vec<Tag>>> {
        let span = self.span(raw);
        let tags = match &raw.value {
            Raw::Null => return None,
            Raw::List(items) => items.iter().filter_map(|item| self.tag(item)).collect(),
            Raw::Map => {
                self.refuse(Field::Tags, span);
                return None;
            }
            Raw::Text(_) | Raw::Bool(_) | Raw::Number => {
                let value = Value { raw: &raw.value, written: self.source.written(span) };
                let written = value.text().into_iter().flat_map(|text| text.split(','));
                let written = written.map(str::trim).filter(|tag| !tag.is_empty());
                written.filter_map(|tag| self.accept(Tag::new(tag).ok(), span)).collect()
            }
        };
        Some(Spanned::new(tags, span))
    }

    fn tag(&mut self, item: &Located<Raw>) -> Option<Tag> {
        let span = self.span(item);
        let value = Value { raw: &item.value, written: self.source.written(span) };
        let tag = value.text().map(str::trim).and_then(|text| Tag::new(text).ok());
        self.accept(tag, span)
    }

    fn accept(&mut self, tag: Option<Tag>, span: Span) -> Option<Tag> {
        if tag.is_none() {
            self.refuse(Field::Tags, span);
        }
        tag
    }

    fn refuse(&mut self, field: Field, span: Span) {
        let problem =
            Problem::FrontmatterValueInvalid { key: field.key().to_owned(), expected: Expected(field).to_string() };
        self.diagnostics.push(Diagnostic::new(problem).on(self.source.subject.clone()).at(span));
    }

    /// Where a value is; a list reaches to the end of its last item.
    fn span(&self, located: &Located<Raw>) -> Span {
        let start = self.source.span(located.referenced).unwrap_or(self.source.yaml);
        let Raw::List(items) = &located.value else { return start };
        let end = items.iter().map(|item| self.span(item).end()).max().unwrap_or(start.end());
        Span::new(start.start(), end.max(start.end())).unwrap_or(start)
    }
}
