//! The `git` capability a source plugin asks of its host (decisions 0036 and 0046): fetch a
//! repository into a cache the host manages, then read the fetched commit as a [`Snapshot`].
//!
//! A [`Request`] names a repository by its [`Locator`], what to fetch of it ([`Revision`],
//! [`Depth`]) and, for a private one, the name of the credential to fetch it with ([`Grant`]).
//! The name is all a plugin ever holds: the host keeps the secret and never lets it cross.
//! A fetch gives a [`Cloned`], a handle to what the host now holds; the snapshot is read from
//! that. Everything the host refuses is a typed [`Refused`] with its numbers, and a failure that
//! has no case of its own is [`Refused::Failed`], whose detail the host has already said.
//!
//! [`Git`] is the capability as a native trait, so a source's rule is written once and run
//! against the host's own implementation natively and against the imported one in a component.

mod locator;
mod refused;
mod request;

pub use locator::{Grant, Locator, LocatorError, NameError, Revision, Scheme};
pub use refused::Refused;
pub use request::{Auth, Budget, Cloned, Depth, History, Network, Request, Want};

use crate::Snapshot;

/// The host's `git` capability, as a trait a source's rule is written against.
pub trait Git {
    /// What a failure with no case of its own holds.
    type Error: std::error::Error + 'static;

    /// Fetches the repository into the host's cache, or brings a clone already there to the
    /// depth asked for.
    fn fetch(&mut self, request: &Request) -> Result<Cloned, Refused<Self::Error>>;

    /// The snapshot of `cloned` at its commit, its files stored where the build reads them.
    fn snapshot(&mut self, cloned: &Cloned) -> Result<Snapshot, Refused<Self::Error>>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_cache_key_names_the_grant() {
        let locator: Locator = "https://example.org/a.git".parse().unwrap();
        let open = Request::new(locator.clone());
        let granted = Request { grant: Some("deploy".parse().unwrap()), ..Request::new(locator) };
        assert_ne!(open.key(), granted.key());
    }

    #[test]
    fn a_locator_is_a_file_or_https_address_and_nothing_else() {
        assert_eq!("file:///srv/repo".parse::<Locator>().unwrap().scheme(), Scheme::File);
        assert_eq!(
            "https://example.org/a/b.git".parse::<Locator>().unwrap().to_string(),
            "https://example.org/a/b.git"
        );
        for refused in [
            "git@host:a/b.git",
            "ssh://host/a",
            "git://host/a",
            "http://host/a",
            "file://relative",
            "https://",
            "file:///a b",
        ] {
            assert!(refused.parse::<Locator>().is_err(), "{refused}");
        }
        assert_eq!("git@host:a".parse::<Locator>(), Err(LocatorError::Scheme { written: "git@host:a".into() }));
    }

    #[test]
    fn a_revision_cannot_look_like_an_option() {
        assert!("main".parse::<Revision>().is_ok());
        assert_eq!("--upload-pack=x".parse::<Revision>(), Err(NameError::Character { at: 0 }));
        assert!("a b".parse::<Revision>().is_err());
        assert!("".parse::<Revision>().is_err());
    }

    #[test]
    fn a_grant_is_a_plain_name() {
        assert!("github".parse::<Grant>().is_ok());
        assert!("GitHub".parse::<Grant>().is_err());
        assert!("a".repeat(65).parse::<Grant>().is_err());
    }

    #[test]
    fn the_network_setting_orders_what_it_lets_through() {
        assert!(!Network::Off.allows(Scheme::File));
        assert!(Network::Local.allows(Scheme::File) && !Network::Local.allows(Scheme::Https));
        assert!(Network::On.allows(Scheme::Https));
    }
}
