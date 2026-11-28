//! Proving a name.

use jiff::Timestamp;

use control::{Control, DomainName, Found, Id, Ports, Proven, Proving, SiteKind};

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
