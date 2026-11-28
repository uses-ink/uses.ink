//! The sign-in routes: a method and a path become one [`Enter`], or say why they are none.

use std::fmt;

use control::keys::UserCode;
use control::signin::Proof;
use control::{Id, SessionKind};

use crate::reply::Method;

/// What a request to the sign-in routes asks for. A token in the path is parsed here; one that
/// is not shaped like a token is `None`, and is answered exactly as a token that was never made.
#[derive(Debug)]
#[non_exhaustive]
pub enum Enter {
    /// `POST /v1/login`: ask for a sign-in link to be sent.
    Ask,
    /// `GET /v1/login/{token}`: what the link is for, without using it. With the operator's
    /// `reveals` setting it uses the link as `POST` does.
    Look(Option<Proof>),
    /// `POST /v1/login/{token}`: use the link and sign in.
    Redeem(Option<Proof>),
    /// `GET /v1/session`: the session the request holds.
    Show,
    /// `DELETE /v1/session`: end it.
    End,
    /// `GET /v1/session/fresh`: whether a passkey signed this session in recently enough to change
    /// credentials, members or keys; the dashboard asks before it makes such a change.
    Fresh,
    /// `GET /v1/sessions`: the account's live sessions, with the one asking marked.
    Sessions,
    /// `DELETE /v1/sessions/{id}`: end one of the account's sessions. An id that is no id is
    /// `None`, and is answered as an id that names nothing.
    Revoke(Option<Id<SessionKind>>),
    /// `GET /v1/approve/{code}`: what a person is asked to approve: the fingerprint, the label
    /// and the rights. The code is the 8 letters the requester shows.
    Seeing(Option<UserCode>),
    /// `POST /v1/approve/{code}`: approve it, naming the fingerprint compared.
    Approving(Option<UserCode>),
    /// `DELETE /v1/approve/{code}`: turn it down.
    Declining(Option<UserCode>),
}

/// A method a sign-in route does not take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnterError {
    /// The method asked.
    pub asked: Method,
    /// The methods the path takes.
    pub allowed: Vec<Method>,
}

impl fmt::Display for EnterError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let allowed: Vec<String> = self.allowed.iter().map(ToString::to_string).collect();
        write!(out, "{} is not taken here, only {}", self.asked, allowed.join(", "))
    }
}

impl std::error::Error for EnterError {}

impl Enter {
    /// The path under which a link's token is the last segment.
    const LOGIN: &'static str = "/v1/login";
    /// The dashboard's page for a link, under which the token is the last segment too.
    const PAGE: &'static str = "/login";
    /// The session's path.
    const SESSION: &'static str = "/v1/session";
    /// Whether the session was proved with a passkey within the step-up window.
    const FRESH: &'static str = "/v1/session/fresh";
    /// The path of the account's sessions, under which a session's id is the last segment.
    const SESSIONS: &'static str = "/v1/sessions";
    /// The path under which an enrolment's code is the last segment.
    const APPROVE: &'static str = "/v1/approve";

    /// The parts of a path of these routes: `None` when the path is not one of them.
    fn parts(path: &str) -> Option<(&str, Option<&str>)> {
        let path = path.trim_end_matches('/');
        if path == Self::LOGIN || path == Self::SESSION || path == Self::SESSIONS || path == Self::FRESH {
            return Some((path, None));
        }
        let (base, token) = [Self::LOGIN, Self::APPROVE, Self::SESSIONS]
            .into_iter()
            .find_map(|base| Some((base, path.strip_prefix(base)?.strip_prefix('/')?)))?;
        (!token.contains('/') && !token.is_empty()).then_some((base, Some(token)))
    }

    /// Whether `path` is one of the sign-in routes, whatever the method.
    pub fn owns(path: &str) -> bool {
        Self::parts(path).is_some()
    }

    /// The call `method` of `path` is: `None` when the path is not a sign-in route.
    pub fn parse(method: Method, path: &str) -> Option<Result<Self, EnterError>> {
        let (base, token) = Self::parts(path)?;
        let proof = || token.and_then(|text| text.parse::<Proof>().ok());
        let code = || token.and_then(|text| text.parse::<UserCode>().ok());
        let id = || token.and_then(|text| text.parse::<Id<SessionKind>>().ok());
        let table: Vec<(Method, Self)> = match (base, token) {
            (Self::APPROVE, _) => vec![
                (Method::Get, Self::Seeing(code())),
                (Method::Post, Self::Approving(code())),
                (Method::Delete, Self::Declining(code())),
            ],
            (Self::FRESH, _) => vec![(Method::Get, Self::Fresh)],
            (Self::SESSIONS, None) => vec![(Method::Get, Self::Sessions)],
            (Self::SESSIONS, Some(_)) => vec![(Method::Delete, Self::Revoke(id()))],
            (Self::LOGIN, None) => vec![(Method::Post, Self::Ask)],
            (Self::LOGIN, Some(_)) => vec![(Method::Get, Self::Look(proof())), (Method::Post, Self::Redeem(proof()))],
            _ => vec![(Method::Get, Self::Show), (Method::Delete, Self::End)],
        };
        let allowed: Vec<Method> = table.iter().map(|(known, _)| *known).collect();
        Some(
            table
                .into_iter()
                .find(|(known, _)| *known == method)
                .map(|(_, call)| call)
                .ok_or(EnterError { asked: method, allowed }),
        )
    }

    /// `path` as it may be logged: the token of a link is cut out.
    pub fn redact(path: &str) -> String {
        let secret = [Self::LOGIN, Self::PAGE, Self::APPROVE].into_iter().find(|base| {
            path.strip_prefix(*base).and_then(|rest| rest.strip_prefix('/')).is_some_and(|rest| !rest.is_empty())
        });
        match secret {
            Some(base) => format!("{base}/<redacted>"),
            None => path.to_owned(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const TOKEN: &str = "0101010101010101010101010101010101010101010101010101010101010101";

    #[test]
    fn routes_are_methods_and_paths() {
        assert!(matches!(Enter::parse(Method::Post, "/v1/login"), Some(Ok(Enter::Ask))));
        assert!(matches!(Enter::parse(Method::Post, "/v1/login/"), Some(Ok(Enter::Ask))));
        assert!(matches!(Enter::parse(Method::Get, &format!("/v1/login/{TOKEN}")), Some(Ok(Enter::Look(Some(_))))));
        assert!(matches!(Enter::parse(Method::Post, &format!("/v1/login/{TOKEN}")), Some(Ok(Enter::Redeem(Some(_))))));
        assert!(matches!(Enter::parse(Method::Get, "/v1/session"), Some(Ok(Enter::Show))));
        assert!(matches!(Enter::parse(Method::Delete, "/v1/session"), Some(Ok(Enter::End))));
        assert!(matches!(Enter::parse(Method::Get, "/v1/sessions"), Some(Ok(Enter::Sessions))));
        let id = "0a".repeat(16);
        assert!(matches!(
            Enter::parse(Method::Delete, &format!("/v1/sessions/{id}")),
            Some(Ok(Enter::Revoke(Some(_))))
        ));
        assert!(matches!(Enter::parse(Method::Delete, "/v1/sessions/nope"), Some(Ok(Enter::Revoke(None)))));
    }

    #[test]
    fn a_token_that_is_no_token_is_still_a_route() {
        assert!(matches!(Enter::parse(Method::Post, "/v1/login/short"), Some(Ok(Enter::Redeem(None)))));
    }

    #[test]
    fn a_wrong_method_lists_the_right_ones() {
        let error = Enter::parse(Method::Get, "/v1/login").unwrap().unwrap_err();
        assert_eq!(error, EnterError { asked: Method::Get, allowed: vec![Method::Post] });
    }

    #[test]
    fn other_paths_are_not_ours() {
        for path in ["/v1/sites", "/v1/login/a/b", "/v1/logins", "/v1/sessionsx", "/v1/sessions/a/b", "/"] {
            assert!(Enter::parse(Method::Get, path).is_none(), "{path}");
            assert!(!Enter::owns(path), "{path}");
        }
    }

    #[test]
    fn a_log_line_has_no_token() {
        assert_eq!(Enter::redact(&format!("/v1/login/{TOKEN}")), "/v1/login/<redacted>");
        assert_eq!(Enter::redact(&format!("/login/{TOKEN}")), "/login/<redacted>");
        assert_eq!(Enter::redact("/login"), "/login");
        assert_eq!(Enter::redact("/v1/login"), "/v1/login");
        assert_eq!(Enter::redact("/v1/sites"), "/v1/sites");
    }
}
