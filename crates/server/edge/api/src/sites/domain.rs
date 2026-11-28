//! Custom domains: list, claim, prove and remove (decision 0093).

use authority_dns::Resolver;
use authority_forge::Transport;
use clock::{Clock, Draw};
use control::{DomainName, Id, NewClaim, Ports, SiteKind, Token};

use crate::Api;
use crate::auth::credential::{Credentials, Principal};
use crate::fault::Fault;
use crate::port::{Port, Unnamed};
use crate::reply::{Reply, Status};
use crate::view::{DomainView, Listed, Unclaimed};

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw, R: Resolver, T: Transport> Api<P, C, K, D, R, T> {
    /// `GET /v1/sites/{site}/domains`: the names that serve the site, then the ones it has still
    /// to prove with their records.
    pub(crate) async fn domains(&self, principal: Principal, site: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        self.owned(principal, site).await?;
        let listed = self.control.domains(site).await?;
        let proven = listed.proven.iter().map(DomainView::proven);
        let pending = listed.pending.iter().map(DomainView::pending);
        Self::json(Status::Ok, &Listed { items: proven.chain(pending).collect() })
    }

    /// `PUT /v1/sites/{site}/domains/{name}`: asks to prove the name and answers with the record
    /// to publish. Asking again makes a new token, which the old record does not prove.
    pub(crate) async fn claim(
        &self,
        principal: Principal,
        target: (Id<SiteKind>, DomainName),
    ) -> Result<Reply, Fault<P>> {
        let (site, name) = target;
        self.owned(principal, site).await?;
        let token = Token::from_bytes(self.drawn()?);
        let claim = self.control.claim_domain(NewClaim { site, name, token, at: self.clock.now() }).await?;
        Self::json(Status::Created, &DomainView::pending(&claim))
    }

    /// `POST /v1/sites/{site}/domains/{name}/proof`: reads the record and, when it carries the
    /// claim's token, makes the name serve the site.
    pub(crate) async fn prove(
        &self,
        principal: Principal,
        target: (Id<SiteKind>, DomainName),
    ) -> Result<Reply, Fault<P>> {
        let (site, name) = target;
        self.owned(principal, site).await?;
        let proven = match self.dns.prove(&self.control, (site, name), self.clock.now()).await {
            Ok(proven) => proven,
            Err(authority_dns::Error::Lookup(error)) => return Err(Fault::Port(Unnamed::new(Port::Resolver, error))),
            Err(authority_dns::Error::Control(error)) => return Err(Fault::Control(error)),
        };
        Self::json(Status::Ok, &DomainView::proven(&proven))
    }

    /// `DELETE /v1/sites/{site}/domains/{name}`: the name stops serving the site, or its claim is
    /// dropped.
    pub(crate) async fn unclaim(
        &self,
        principal: Principal,
        target: (Id<SiteKind>, DomainName),
    ) -> Result<Reply, Fault<P>> {
        let (site, name) = target;
        self.owned(principal, site).await?;
        self.control.remove_domain(site, &name, self.clock.now()).await?;
        Self::json(Status::Ok, &Unclaimed { name: name.to_string() })
    }
}
