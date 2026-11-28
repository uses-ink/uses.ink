//! Proving a name.

use jiff::Timestamp;

use control::{Checking, Control, DomainName, Found, Id, Ports, Proven, Proving, SiteKind, Verdict};

use crate::Host;
use crate::{Error, Record, Resolver};

/// The DNS authority over a [`Resolver`].
#[derive(Debug)]
pub struct Dns<R> {
    resolver: R,
}

impl<R: Resolver> Dns<R> {
    /// The authority that reads DNS through `resolver`.
    pub const fn new(resolver: R) -> Self {
        Self { resolver }
    }

    /// The resolver it reads through.
    pub const fn resolver(&self) -> &R {
        &self.resolver
    }

    /// The records `name` has at its proof host, as the control plane reads them: how many, and
    /// the tokens among them.
    pub async fn read(&self, name: &DomainName) -> Result<Found, R::Error> {
        let records = self.resolver.txt(&Host::of(name)).await?;
        let tokens = records.iter().filter_map(|text| Record::read(text)).collect();
        Ok(Found::new(records.len() as u64, tokens))
    }

    /// Reads the records of `name` and asks the control plane to make it serve `site`. The
    /// control plane holds the claim and decides: a record that carries another site's token, or
    /// none, is `domain.proof.missing`, and a name another site proved is `domain.name.taken`.
    pub async fn prove<P: Ports>(
        &self,
        control: &Control<P>,
        target: (Id<SiteKind>, DomainName),
        at: Timestamp,
    ) -> Result<Proven, Error<P, R::Error>> {
        let (site, name) = target;
        let found = self.read(&name).await.map_err(Error::Lookup)?;
        Ok(control.prove_domain(Proving { site, name, found, at }).await?)
    }
}

/// What one pass of rechecks found (decision 0151).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rechecked {
    /// Names whose record still carries their token.
    pub held: u64,
    /// Names that had lapsed and whose record carries their token again.
    pub restored: u64,
    /// Names that do not serve: their record does not carry the token, new or not.
    pub lapsed: u64,
    /// Names whose lookup got no answer: nothing was recorded for them.
    pub unread: u64,
    /// Lapsed names freed after their grace.
    pub freed: u64,
}

impl std::fmt::Display for Rechecked {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            out,
            "{} held, {} lapsed, {} restored, {} freed, {} not readable",
            self.held, self.lapsed, self.restored, self.freed, self.unread
        )
    }
}

impl Rechecked {
    /// Whether the pass changed nothing and could not read nothing: every name it looked at held.
    pub const fn quiet(&self) -> bool {
        self.restored + self.lapsed + self.unread + self.freed == 0
    }
}

impl<R: Resolver> Dns<R> {
    /// Reads the proof record of every name that is due at `at`, at most `limit` of them, and tells
    /// the control plane what it found: a name whose record lost its token stops resolving, one
    /// whose record is back serves again. A lookup that got no answer says nothing about the name,
    /// so it is counted and left for the next pass. Then the names lapsed past their grace are freed.
    pub async fn recheck<P: Ports>(
        &self,
        control: &Control<P>,
        at: Timestamp,
        limit: u64,
    ) -> Result<Rechecked, control::Error<P>> {
        let mut found = Rechecked::default();
        for due in control.due_domains(at, limit).await? {
            let Ok(read) = self.read(&due.name).await else {
                found.unread += 1;
                continue;
            };
            let checking = Checking { site: due.site, name: due.name, found: read, at };
            match control.recheck_domain(checking).await? {
                Verdict::Held => found.held += 1,
                Verdict::Restored => found.restored += 1,
                _ => found.lapsed += 1,
            }
        }
        found.freed = control.free_lapsed(at).await?;
        Ok(found)
    }
}
