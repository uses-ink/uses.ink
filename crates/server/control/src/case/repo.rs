//! Repositories (decision 0160): a site asks to prove a repository, commits the challenge to it,
//! and the repository then builds the site when a webhook says it moved. The claim and its sealed
//! webhook secret are rows; what reads the forge is the repository authority, which hands the
//! control plane a typed proof.

use authority_forge::{Proven, Repo};
use db::{Constraint, Db as _, Statement};
use jiff::Timestamp;
use sea_query::{Expr, ExprTrait, Order, Query};
use secret::Plain;
use secret::Secret as _;

use crate::credential::{self, Use};
use crate::id::{Credential as CredentialKind, Id, Site as SiteKind};
use crate::table;
use crate::{Control, Error, Event, NewRepo, Ports, Refusal, RepoClaim};

/// A proof to record: the repository the forge showed the site's challenge in.
#[derive(Debug)]
pub struct Binding {
    /// The site that claimed the repository.
    pub site: Id<SiteKind>,
    /// What the authority proved.
    pub proof: Proven,
    /// When it was read.
    pub at: Timestamp,
}

/// A secret to put in place of a claim's: drawn and sealed by the caller's call.
#[derive(Debug)]
pub struct Rotating {
    /// The site.
    pub site: Id<SiteKind>,
    /// The repository.
    pub repo: Repo,
    /// The new secret, shown to the person once by the caller.
    pub secret: Plain,
    /// The id of the credential that will hold it.
    pub credential: Id<CredentialKind>,
    /// When it is replaced.
    pub at: Timestamp,
}

/// A claim to let go of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Releasing {
    /// The site.
    pub site: Id<SiteKind>,
    /// The repository.
    pub repo: Repo,
    /// When it is let go.
    pub at: Timestamp,
}

impl<P: Ports> Control<P> {
    /// Records that `new.site` asks to prove `new.repo`, and seals the webhook secret of the claim.
    /// Asking again replaces the site's earlier challenge and secret for the repository.
    ///
    /// The repository must be the site's own git source (compared as the lower-case host and path
    /// of the address, without a `.git`), so a site has at most one claim; a repository another
    /// site proved is refused. A claim serves nothing: only
    /// [`Control::bind_repo`] makes a push build the site.
    pub async fn claim_repo(&self, new: NewRepo) -> Result<RepoClaim, Error<P>> {
        let site = self.active(new.site).await?;
        let own = match &site.source {
            build::Source::Git { url, .. } => Repo::at(&url.to_string()).ok(),
            _ => None,
        };
        if own.as_ref() != Some(&new.repo) {
            return Err(Refusal::RepoSource { site: site.id, repo: new.repo }.into());
        }
        let earlier = self.repo_claim(site.id, &new.repo).await?;
        match self.bound(&new.repo).await? {
            Some(held) if held.site == site.id => return Err(Refusal::Exists { id: new.repo.to_string() }.into()),
            Some(_) => return Err(Refusal::RepoTaken { repo: new.repo }.into()),
            None => {}
        }
        let secret = credential::New {
            id: new.credential,
            site: site.id,
            purpose: Use::WebhookSecret,
            secret: new.secret,
            at: new.at,
        };
        let sealed = self.ports.secret().seal(&secret.secret, &Self::binding(secret.id)).await?;
        let claim = RepoClaim {
            site: site.id,
            repo: new.repo,
            challenge: new.challenge,
            secret: new.credential,
            created: new.at,
            proven: None,
        };
        let mut batch: Vec<Statement> = Vec::new();
        batch.extend(earlier.iter().flat_map(Self::drop_claim));
        batch.push(Self::credential_row(&secret, &sealed)?);
        batch.push(claim.insert()?);
        batch.push(Event::RepoClaimed { repo: claim.repo.clone() }.insert(site.id, new.at)?);
        self.ports.db().batch(batch).await?;
        Ok(claim)
    }

    /// Makes the repository of a proof bind to the site that claimed it, so a push to it builds the
    /// site. The proof is what the authority read from the forge; the control plane holds the claim
    /// and decides. Every other site's claim of the repository, with its secret, is dropped in the
    /// same batch, and the database keeps one proof of a repository whatever order sites prove in.
    /// Proving a repository the site already has answers the claim it has.
    pub async fn bind_repo(&self, binding: Binding) -> Result<RepoClaim, Error<P>> {
        let Binding { site, proof, at } = binding;
        let site = self.active(site).await?;
        let claim = self.repo_claim(site.id, &proof.repo).await?;
        let claim = claim.ok_or_else(|| Refusal::NoRepo { site: site.id, repo: proof.repo.clone() })?;
        if claim.bound() {
            return Ok(claim);
        }
        if self.bound(&claim.repo).await?.is_some() {
            return Err(Refusal::RepoTaken { repo: claim.repo }.into());
        }
        let mut batch: Vec<Statement> = vec![
            Query::update()
                .table(table::Repo::Table)
                .value(table::Repo::ProvenAt, at.as_millisecond())
                .value(table::Repo::Owned, 1_i64)
                .and_where(Expr::col(table::Repo::Repo).eq(claim.repo.to_string()))
                .and_where(Expr::col(table::Repo::SiteId).eq(site.id.to_string()))
                .to_owned()
                .into(),
        ];
        for other in self.claims_of(&claim.repo).await?.iter().filter(|other| other.site != site.id) {
            batch.extend(Self::drop_claim(other));
        }
        batch.push(Event::RepoProven { repo: claim.repo.clone() }.insert(site.id, at)?);
        match self.ports.db().batch(batch).await {
            Ok(_) => Ok(RepoClaim { proven: Some(at), ..claim }),
            Err(error) if error.constraint() == Some(Constraint::Unique) => {
                Err(Refusal::RepoTaken { repo: claim.repo }.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Puts a new webhook secret in place of the claim's, and the old one is gone: a delivery
    /// signed with it is refused from now on. The claim, proved or not, keeps its challenge.
    pub async fn rotate_repo(&self, rotating: Rotating) -> Result<RepoClaim, Error<P>> {
        let Rotating { site, repo, secret, credential, at } = rotating;
        let site = self.active(site).await?;
        let claim = self.repo_claim(site.id, &repo).await?;
        let claim = claim.ok_or_else(|| Refusal::NoRepo { site: site.id, repo: repo.clone() })?;
        let new = credential::New { id: credential, site: site.id, purpose: Use::WebhookSecret, secret, at };
        let sealed = self.ports.secret().seal(&new.secret, &Self::binding(new.id)).await?;
        let batch: Vec<Statement> = vec![
            Self::credential_row(&new, &sealed)?,
            Query::update()
                .table(table::Repo::Table)
                .value(table::Repo::SecretId, credential.to_string())
                .and_where(Expr::col(table::Repo::Repo).eq(repo.to_string()))
                .and_where(Expr::col(table::Repo::SiteId).eq(site.id.to_string()))
                .to_owned()
                .into(),
            Query::delete()
                .from_table(table::Credential::Table)
                .and_where(Expr::col(table::Credential::Id).eq(claim.secret.to_string()))
                .to_owned()
                .into(),
            Event::RepoRotated { repo }.insert(site.id, at)?,
        ];
        self.ports.db().batch(batch).await?;
        Ok(RepoClaim { secret: credential, ..claim })
    }

    /// Lets go of a repository: a proved one stops building the site, an asked one is dropped, and
    /// the secret is deleted either way. A site that is being removed may still release.
    pub async fn release_repo(&self, releasing: Releasing) -> Result<(), Error<P>> {
        let Releasing { site, repo, at } = releasing;
        let site = self.site(site).await?;
        let claim = self.repo_claim(site.id, &repo).await?;
        let claim = claim.ok_or_else(|| Refusal::NoRepo { site: site.id, repo: repo.clone() })?;
        let mut batch = Self::drop_claim(&claim);
        batch.push(Event::RepoReleased { repo }.insert(site.id, at)?);
        self.ports.db().batch(batch).await?;
        Ok(())
    }

    /// The repositories `site` has claimed, proved and not, by name.
    pub async fn repos(&self, site: Id<SiteKind>) -> Result<Vec<RepoClaim>, Error<P>> {
        self.site(site).await?;
        let select = Query::select()
            .columns(RepoClaim::COLUMNS)
            .from(table::Repo::Table)
            .and_where(Expr::col(table::Repo::SiteId).eq(site.to_string()))
            .order_by(table::Repo::Repo, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(RepoClaim::read).collect::<Result<_, _>>()?)
    }

    /// The claim of `repo` by `site`, if there is one.
    pub async fn repo_claim(&self, site: Id<SiteKind>, repo: &Repo) -> Result<Option<RepoClaim>, Error<P>> {
        let select = Query::select()
            .columns(RepoClaim::COLUMNS)
            .from(table::Repo::Table)
            .and_where(Expr::col(table::Repo::Repo).eq(repo.to_string()))
            .and_where(Expr::col(table::Repo::SiteId).eq(site.to_string()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| RepoClaim::read(&row)).transpose()?)
    }

    /// The claim that proved `repo`, if a site did: the site a push to it builds.
    pub async fn bound(&self, repo: &Repo) -> Result<Option<RepoClaim>, Error<P>> {
        let select = Query::select()
            .columns(RepoClaim::COLUMNS)
            .from(table::Repo::Table)
            .and_where(Expr::col(table::Repo::Repo).eq(repo.to_string()))
            .and_where(Expr::col(table::Repo::Owned).eq(1_i64))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| RepoClaim::read(&row)).transpose()?)
    }

    /// The webhook secret of a claim, opened for one use and zeroed when it is dropped.
    pub async fn hook_secret(&self, claim: &RepoClaim) -> Result<Plain, Error<P>> {
        self.credential(claim.secret).await
    }

    /// Every claim of `repo`, by any site.
    async fn claims_of(&self, repo: &Repo) -> Result<Vec<RepoClaim>, Error<P>> {
        let select = Query::select()
            .columns(RepoClaim::COLUMNS)
            .from(table::Repo::Table)
            .and_where(Expr::col(table::Repo::Repo).eq(repo.to_string()))
            .to_owned();
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(RepoClaim::read).collect::<Result<_, _>>()?)
    }

    /// The statements that delete a claim and then the credential it held: in that order, since
    /// the claim refers to the credential.
    fn drop_claim(claim: &RepoClaim) -> Vec<Statement> {
        vec![
            Query::delete()
                .from_table(table::Repo::Table)
                .and_where(Expr::col(table::Repo::Repo).eq(claim.repo.to_string()))
                .and_where(Expr::col(table::Repo::SiteId).eq(claim.site.to_string()))
                .to_owned()
                .into(),
            Query::delete()
                .from_table(table::Credential::Table)
                .and_where(Expr::col(table::Credential::Id).eq(claim.secret.to_string()))
                .to_owned()
                .into(),
        ]
    }
}
