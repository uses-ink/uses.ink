//! Which origins may make a request that changes state with a cookie (`docs/auth.md` section 8,
//! CSRF): the Fetch Metadata defence of the OWASP CSRF cheat sheet.

use crate::reply::Request;

/// The origins a cookie route takes a state-changing request from. `Origin` must be one of them
/// exactly (scheme, host and port); with no `Origin`, the browser must say `Sec-Fetch-Site:
/// same-origin`; with neither, the request is refused, so a script that is not a browser has to
/// sign its request instead.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Origins(Vec<String>);

impl Origins {
    /// No origin: every state-changing request with an `Origin` is refused, and one with none
    /// needs `same-origin`.
    pub const fn none() -> Self {
        Self(Vec::new())
    }

    /// The origin of the address links point at, which is where the page that asks for a link and
    /// spends one is served.
    pub fn of(base: &mail::Base) -> Self {
        let text = base.to_string();
        let (scheme, rest) = text.split_once("://").unwrap_or(("", &text));
        let host = rest.split('/').next().unwrap_or_default();
        Self::none().with(format!("{scheme}://{host}"))
    }

    /// These origins and one more. A browser writes the host in lower case, so it is kept so.
    #[must_use]
    pub fn with(mut self, origin: impl Into<String>) -> Self {
        let origin = origin.into().to_ascii_lowercase();
        if !self.0.contains(&origin) {
            self.0.push(origin);
        }
        self
    }

    /// The origins.
    pub fn list(&self) -> &[String] {
        &self.0
    }

    /// Whether `request` may change state.
    pub fn admits(&self, request: &Request) -> bool {
        match (&request.origin, &request.site) {
            (Some(origin), _) => self.0.iter().any(|known| known == origin),
            (None, Some(site)) => site == "same-origin",
            (None, None) => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::reply::{Method, Payload};

    fn request(origin: Option<&str>, site: Option<&str>) -> Request {
        Request {
            method: Method::Post,
            path: "/v1/login".to_owned(),
            query: None,
            authorization: None,
            cookie: None,
            origin: origin.map(str::to_owned),
            site: site.map(str::to_owned),
            source: None,
            body: Payload::Whole(Vec::new()),
        }
    }

    #[test]
    fn the_origin_of_a_link_base_is_scheme_host_and_port() {
        let base: mail::Base = "https://Ink.Example:8443/v1/login".parse().unwrap();
        assert_eq!(Origins::of(&base).list(), ["https://ink.example:8443"]);
    }

    #[test]
    fn an_origin_must_match_exactly_and_no_origin_needs_same_origin() {
        let origins = Origins::none().with("https://ink.example");
        assert!(origins.admits(&request(Some("https://ink.example"), None)));
        for other in ["https://ink.example:8443", "http://ink.example", "https://evil.example", "null"] {
            assert!(!origins.admits(&request(Some(other), Some("same-origin"))), "{other}");
        }
        assert!(origins.admits(&request(None, Some("same-origin"))));
        assert!(!origins.admits(&request(None, Some("cross-site"))));
        assert!(!origins.admits(&request(None, Some("same-site"))));
        assert!(!origins.admits(&request(None, None)), "a request that says nothing is refused");
    }
}
