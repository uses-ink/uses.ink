//! Response headers as they are built: values are checked once, here, and the first one that is
//! not a header value is kept for the caller instead of being dropped.

use std::fmt;

use model::{Header, HeaderValue, HeaderValueError};

use crate::http::request::Scheme;
use crate::http::response::{Field, Name};

/// A header value the edge wrote that is not a header value. The edge writes only digits, hex,
/// ASCII and percent escapes, so this is a bug, and it is reported as one.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldError {
    /// Which header.
    pub name: Name,
    /// Why its value is refused.
    pub source: HeaderValueError,
}

impl fmt::Display for FieldError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the edge wrote a bad `{}` header", self.name)
    }
}

impl std::error::Error for FieldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// The fields of a response so far.
#[derive(Debug, Default)]
pub(crate) struct Fields {
    list: Vec<Field>,
    error: Option<FieldError>,
}

impl Fields {
    /// Adds `name: value`, unless a field of that name is there already.
    pub(crate) fn set(&mut self, name: Name, value: impl fmt::Display) {
        if self.has(name) {
            return;
        }
        match HeaderValue::parse(value.to_string()) {
            Ok(value) => self.list.push(Field { name, value }),
            Err(source) => {
                self.error.get_or_insert(FieldError { name, source });
            }
        }
    }

    /// Adds a field already checked, unless a field of that name is there already.
    pub(crate) fn add(&mut self, field: &Field) {
        if !self.has(field.name) {
            self.list.push(field.clone());
        }
    }

    /// Adds a manifest entry's headers.
    pub(crate) fn entry(&mut self, headers: &[Header]) {
        for header in headers {
            self.add(&Field { name: Name::Entry(header.name), value: header.value.clone() });
        }
    }

    /// Whether a field of that name is there.
    pub(crate) fn has(&self, name: Name) -> bool {
        self.list.iter().any(|field| field.name == name)
    }

    /// The fields, or the first value that was no header value.
    pub(crate) fn finish(self) -> Result<Vec<Field>, FieldError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.list),
        }
    }
}

/// The headers every response carries, whatever made it.
#[derive(Clone, Debug)]
pub(crate) struct Hardening {
    fields: Vec<Field>,
    hsts: Option<Field>,
}

impl Hardening {
    /// What a file the edge makes itself, or an entry that lacks them, is served with: the strict
    /// set of the engine's `Policy::FILE` (architecture.md section 13). A manifest entry's own
    /// headers win; these fill what it leaves out. A test compares them with a built manifest.
    pub(crate) const FIXED: &'static [(model::HeaderName, &'static str)] = &[
        (model::HeaderName::ContentSecurityPolicy, "default-src 'none'; frame-ancestors 'none'; sandbox"),
        (model::HeaderName::CrossOriginResourcePolicy, "same-origin"),
        (model::HeaderName::XContentTypeOptions, "nosniff"),
    ];

    /// What a response with no cache header of its own is told: keep it, ask before using it.
    pub(crate) const REVALIDATE: &'static str = "public, max-age=0, must-revalidate";

    pub(crate) fn new(hsts: Option<&HeaderValue>) -> Result<Self, FieldError> {
        let mut fields = Fields::default();
        for &(name, value) in Self::FIXED {
            let name = Name::Entry(name);
            fields.set(name, value);
        }
        Ok(Self {
            fields: fields.finish()?,
            hsts: hsts.map(|value| Field {
                name: Name::Entry(model::HeaderName::StrictTransportSecurity),
                value: value.clone(),
            }),
        })
    }

    /// Fills `fields` with what is missing, and HSTS over TLS.
    pub(crate) fn apply(&self, fields: &mut Fields, scheme: Scheme) {
        for field in &self.fields {
            fields.add(field);
        }
        if let (Scheme::Tls, Some(hsts)) = (scheme, &self.hsts) {
            fields.add(hsts);
        }
    }
}

impl Hardening {
    /// Every field a response gets whatever made it, for a response that could not be built.
    pub(crate) fn all(&self, scheme: Scheme) -> Vec<Field> {
        let mut fields = self.fields.clone();
        if let (Scheme::Tls, Some(hsts)) = (scheme, &self.hsts) {
            fields.push(hsts.clone());
        }
        fields
    }
}
