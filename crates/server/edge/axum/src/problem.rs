//! The JSON the edge itself answers with, in the shape of the API's problems.

use serde::Serialize;

/// A problem the edge found before the API could be asked: its code, what happened, the causes
/// under it and what to do.
#[derive(Debug, Serialize)]
pub(crate) struct Problem<'a> {
    #[serde(rename = "problem")]
    code: &'a str,
    message: String,
    causes: Vec<String>,
    fix: &'a str,
}

impl<'a> Problem<'a> {
    /// A problem with no causes below it.
    pub(crate) fn new(problem: &'a str, message: String, fix: &'a str) -> Self {
        Self { code: problem, message, causes: Vec::new(), fix }
    }

    /// The same problem with the `source()` chain of `error` as its causes.
    #[must_use]
    pub(crate) fn caused_by(mut self, error: &(dyn std::error::Error + 'static)) -> Self {
        let mut next = error.source();
        while let Some(cause) = next {
            self.causes.push(cause.to_string());
            next = cause.source();
        }
        self
    }

    /// The JSON body.
    pub(crate) fn body(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| br#"{"problem":"internal"}"#.to_vec())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
mod test {
    use super::*;

    #[test]
    fn a_message_with_quotes_is_still_json() {
        let body = Problem::new("x.y.z", r#"it said "no""#.to_owned(), "try again").body();
        let read: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(read["message"], r#"it said "no""#);
        assert_eq!(read["causes"], serde_json::json!([]));
    }
}
