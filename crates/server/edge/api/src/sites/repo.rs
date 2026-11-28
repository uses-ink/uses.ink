//! A site's repository: claim it, prove it, rotate its webhook secret and let it go (decision 0160).

use authority_dns::Resolver;
use authority_forge::{Repo, Transport};
use authority_repo::{Claimed, Error as Repoed};
use build::Source;
use clock::{Clock, Draw};
use control::{Id, Ports, RepoClaim, SiteKind};

use crate::body::{RepoAsk, Unreadable};
use crate::fault::{Fault, Forged};
use crate::port::{Port, Unnamed};
use crate::reply::{Reply, Status};
use crate::route::RepoStep;
use crate::view::{Listed, RepoGiven, RepoReleased, RepoView};
use crate::{Api, Credentials, Principal};

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw, R: Resolver, T: Transport> Api<P, C, K, D, R, T> {
    /// `GET /v1/sites/{site}/repos`: the repositories the site has claimed.
    pub(crate) async fn repo_list(&self, principal: Principal, site: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        self.owned(principal, site).await?;
        let claims = self.control.repos(site).await?;
        Self::json(Status::Ok, &Listed { items: claims.iter().map(|claim| self.repo_view(claim)).collect() })
    }

    /// `POST /v1/sites/{site}/repos/{step}`.
    pub(crate) async fn repo_step(
        &self,
        principal: Principal,
        target: (Id<SiteKind>, RepoStep),
        body: &[u8],
    ) -> Result<Reply, Fault<P>> {
        let (site, step) = target;
        let owned = self.owned(principal, site).await?;
        let ask: RepoAsk =
            if body.is_empty() { RepoAsk::default() } else { Unreadable::read(body).map_err(Fault::Body)? };
        let repos = self.repos.as_ref().ok_or_else(Forged::off)?;
        let repo = match ask.repo {
            Some(named) => named.0,
            None => match &owned.source {
                Source::Git { url, .. } => Repo::at(&url.to_string()).map_err(Forged::name)?,
                _ => return Err(Fault::Unbuilt),
            },
        };
        let asking = (site, repo.clone(), self.clock.now());
        match step {
            RepoStep::Claim => {
                let claimed = repos.claim(&self.control, &self.draw, asking).await.map_err(Self::repoed)?;
                Self::json(Status::Created, &self.repo_given(&claimed))
            }
            RepoStep::Rotate => {
                let claimed = repos.rotate(&self.control, &self.draw, asking).await.map_err(Self::repoed)?;
                Self::json(Status::Ok, &self.repo_given(&claimed))
            }
            RepoStep::Prove => {
                let claim = repos.prove(&self.control, asking).await.map_err(Self::repoed)?;
                Self::json(Status::Ok, &self.repo_view(&claim))
            }
            RepoStep::Release => {
                repos.release(&self.control, asking).await.map_err(Self::repoed)?;
                Self::json(Status::Ok, &RepoReleased { repo: repo.to_string() })
            }
        }
    }

    /// The view of a claim, with the forge it lives on as this server knows it.
    fn repo_view(&self, claim: &RepoClaim) -> RepoView {
        let forge = self.repos.as_ref().and_then(|repos| repos.forge(&claim.repo));
        RepoView::of(claim, forge)
    }

    /// The view of a claim just made, with the secret shown once.
    fn repo_given(&self, claimed: &Claimed) -> RepoGiven {
        RepoGiven::of(self.repo_view(&claimed.claim), &claimed.secret)
    }

    /// The fault of a repository call, whatever the transport and the draw were.
    fn repoed<E: std::error::Error + 'static, G: std::error::Error + 'static>(error: Repoed<P, E, G>) -> Fault<P> {
        match error {
            Repoed::Draw(source) => Fault::Port(Unnamed::new(Port::Draw, source)),
            Repoed::Unproven(source) => Forged::unproven(source).into(),
            Repoed::Control(source) => Fault::Control(source),
        }
    }
}
