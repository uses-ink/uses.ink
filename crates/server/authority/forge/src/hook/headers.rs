//! The headers of a delivery.

use crate::Refused;

/// The headers a webhook arrived with. Names compare without regard to case.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    /// No headers.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same headers with one more.
    #[must_use]
    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.0.push((name.to_owned(), value.to_owned()));
        self
    }

    /// The first value of `name`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(have, _)| have.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// The first value of `name`, or the refusal that it is missing.
    pub fn need(&self, name: &'static str) -> Result<&str, Refused> {
        self.get(name).ok_or(Refused::Missing { header: name })
    }

    /// The first value of `name`, else of `other`: a forge that answers to two spellings.
    pub fn either(&self, names: (&'static str, &'static str)) -> Result<&str, Refused> {
        self.get(names.0).map_or_else(|| self.need(names.1), Ok)
    }
}
