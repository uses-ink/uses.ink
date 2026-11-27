//! The passkey routes: a method and a path become one [`Step`], or say why they are none.

use std::fmt;

use control::{FactorKind, Id};

use crate::reply::Method;

/// What a request to the passkey routes asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Step {
    /// `POST /v1/passkeys/options`: start a registration. Needs a session.
    Options,
    /// `POST /v1/passkeys`: finish it. Needs the session and the ceremony cookie.
    Register,
    /// `POST /v1/passkeys/challenge`: start a sign-in. Needs nothing.
    Challenge,
    /// `POST /v1/passkeys/session`: finish it, and begin a session. Needs the ceremony cookie.
    Assert,
    /// `GET /v1/passkeys`: the account's passkeys. Needs a session.
    List,
    /// `DELETE /v1/passkeys/{id}`: remove one, and end the sessions it made. Needs a session. A
    /// path that is no id is `None`, and is answered as an id that names nothing.
    Remove(Option<Id<FactorKind>>),
}

/// A method a passkey route does not take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepError {
    /// The method asked.
    pub asked: Method,
    /// The methods the path takes.
    pub allowed: Vec<Method>,
}

impl fmt::Display for StepError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let allowed: Vec<String> = self.allowed.iter().map(ToString::to_string).collect();
        write!(out, "{} is not taken here, only {}", self.asked, allowed.join(", "))
    }
}

impl std::error::Error for StepError {}

/// What follows `/v1/passkeys`.
enum Tail<'a> {
    Base,
    Segment(&'a str),
}

impl Step {
    const BASE: &'static str = "/v1/passkeys";

    /// What follows the base path: nothing, or one segment.
    fn tail(path: &str) -> Option<Tail<'_>> {
        let path = path.trim_end_matches('/');
        if path == Self::BASE {
            return Some(Tail::Base);
        }
        let rest = path.strip_prefix(Self::BASE)?.strip_prefix('/')?;
        (!rest.is_empty() && !rest.contains('/')).then_some(Tail::Segment(rest))
    }

    /// Whether `path` is one of the passkey routes, whatever the method.
    pub fn owns(path: &str) -> bool {
        Self::tail(path).is_some()
    }

    /// Whether the step changes something, so a browser must prove where it came from.
    pub const fn mutates(self) -> bool {
        !matches!(self, Self::List)
    }

    /// The step `method` of `path` is: `None` when the path is not a passkey route.
    pub fn parse(method: Method, path: &str) -> Option<Result<Self, StepError>> {
        let table: Vec<(Method, Self)> = match Self::tail(path)? {
            Tail::Base => vec![(Method::Get, Self::List), (Method::Post, Self::Register)],
            Tail::Segment("options") => vec![(Method::Post, Self::Options)],
            Tail::Segment("challenge") => vec![(Method::Post, Self::Challenge)],
            Tail::Segment("session") => vec![(Method::Post, Self::Assert)],
            Tail::Segment(id) => vec![(Method::Delete, Self::Remove(id.parse().ok()))],
        };
        let allowed: Vec<Method> = table.iter().map(|(known, _)| *known).collect();
        Some(
            table
                .into_iter()
                .find(|(known, _)| *known == method)
                .map(|(_, step)| step)
                .ok_or(StepError { asked: method, allowed }),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn routes_are_methods_and_paths() {
        assert_eq!(Step::parse(Method::Get, "/v1/passkeys").unwrap(), Ok(Step::List));
        assert_eq!(Step::parse(Method::Post, "/v1/passkeys/").unwrap(), Ok(Step::Register));
        assert_eq!(Step::parse(Method::Post, "/v1/passkeys/options").unwrap(), Ok(Step::Options));
        assert_eq!(Step::parse(Method::Post, "/v1/passkeys/challenge").unwrap(), Ok(Step::Challenge));
        assert_eq!(Step::parse(Method::Post, "/v1/passkeys/session").unwrap(), Ok(Step::Assert));
        let id = "0a".repeat(16);
        assert!(matches!(Step::parse(Method::Delete, &format!("/v1/passkeys/{id}")).unwrap(), Ok(Step::Remove(Some(_)))));
        assert_eq!(Step::parse(Method::Delete, "/v1/passkeys/nope").unwrap(), Ok(Step::Remove(None)));
    }

    #[test]
    fn a_wrong_method_lists_the_right_ones() {
        let error = Step::parse(Method::Delete, "/v1/passkeys").unwrap().unwrap_err();
        assert_eq!(error, StepError { asked: Method::Delete, allowed: vec![Method::Get, Method::Post] });
    }

    #[test]
    fn other_paths_are_not_ours() {
        for path in ["/v1/passkey", "/v1/passkeys/a/b", "/v1/passkeysx", "/v1/login", "/"] {
            assert!(Step::parse(Method::Get, path).is_none(), "{path}");
            assert!(!Step::owns(path), "{path}");
        }
    }
}
