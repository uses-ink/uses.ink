//! Claiming, proving, rotating and releasing a repository.

use authority_forge::{Challenge, Client, Host, Limits, Proven, Repo, Secret, Transport, Unproven};
use authority_forgejo::{Forgejo, Instances};
use authority_github::Open;
use authority_gitlab::Gitlab;
use clock::Draw;
use control::{Binding, Control, Id, NewRepo, Ports, Refusal, Releasing, RepoClaim, Rotating, SiteKind};
use jiff::Timestamp;
use secret::Plain;
use std::fmt::Write as _;

use crate::{Error, Shared};

/// The forges a server reaches, and how far.
#[derive(Clone, Debug)]
pub struct Forges {
    /// The Forgejo instances the operator listed.
    pub forgejo: Instances,
    /// The GitLab hosts the operator listed.
    pub gitlab: Vec<Host>,
    /// What one call to a forge may cost.
    pub limits: Limits,
}

/// A claim just made, and the secret to give the forge: shown once, then only sealed.
#[derive(Debug)]
pub struct Claimed {
    /// The claim.
    pub claim: RepoClaim,
    /// The webhook secret, in hex. It prints as redacted.
    pub secret: Secret,
}

/// The repository authority over one transport.
#[derive(Debug)]
pub struct Repos<T> {
    github: Open<Shared<T>>,
    forgejo: Forgejo<Shared<T>>,
    gitlab: Gitlab<Shared<T>>,
}

/// A new secret and the id of the credential that will seal it, or why none could be drawn.
type Fresh<P, T, D> =
    Result<(Secret, Id<control::CredentialKind>), Error<P, <T as Transport>::Error, <D as Draw>::Error>>;

/// Bytes as lower-case hex: the spelling of a secret a person pastes into a forge.
struct Hex;

impl Hex {
    fn of(bytes: &[u8]) -> String {
        bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            // writing to a `String` cannot fail
            write!(out, "{byte:02x}").ok();
            out
        })
    }
}

impl<T: Transport> Repos<T> {
    /// The authority reaching `forges` through `transport`.
    pub fn new(forges: Forges, transport: T) -> Self {
        let shared = Shared::new(transport);
        let client = || Client::new(shared.clone(), forges.limits);
        Self {
            github: Open::new(client()),
            forgejo: Forgejo::new(forges.forgejo.clone(), client()),
            gitlab: Gitlab::new(forges.gitlab.clone(), client()),
        }
    }

    /// The transport, for a test to read what a fake was asked.
    pub fn transport(&self) -> &T {
        self.github.client().transport().inner()
    }

    /// Reads the challenge file of `repo` through the forge it lives on. A host no forge lists is
    /// [`Unproven::Unlisted`] and nothing is sent.
    pub async fn read(&self, repo: &Repo, challenge: &Challenge) -> Result<Proven, Unproven<T::Error>> {
        if repo.host == Host::GITHUB {
            self.github.prove(repo, challenge).await
        } else if self.forgejo.instances().find(&repo.host).is_some() {
            self.forgejo.prove(repo, challenge).await
        } else {
            self.gitlab.prove(repo, challenge).await
        }
    }

    /// A claim of `repo` for `site`: the challenge to commit, and the secret to give the forge.
    /// Asking again replaces both.
    pub async fn claim<P: Ports, D: Draw>(
        &self,
        control: &Control<P>,
        draw: &D,
        asking: (Id<SiteKind>, Repo, Timestamp),
    ) -> Result<Claimed, Error<P, T::Error, D::Error>> {
        let (site, repo, at) = asking;
        let challenge = Challenge::from_bytes(draw.bytes().map_err(Error::Draw)?);
        let (secret, credential) = Self::secret(draw)?;
        let new = NewRepo { site, repo, challenge, secret: Plain::from(secret.reveal().to_vec()), credential, at };
        Ok(Claimed { claim: control.claim_repo(new).await?, secret })
    }

    /// Reads the forge for the challenge of the claim of `repo` by `site`, and binds the repository
    /// to the site when it is there.
    pub async fn prove<P: Ports>(
        &self,
        control: &Control<P>,
        asking: (Id<SiteKind>, Repo, Timestamp),
    ) -> Result<RepoClaim, Error<P, T::Error, std::convert::Infallible>> {
        let (site, repo, at) = asking;
        let claim = control.repo_claim(site, &repo).await?;
        let claim = claim.ok_or_else(|| control::Error::from(Refusal::NoRepo { site, repo: repo.clone() }))?;
        let proof = self.read(&repo, &claim.challenge).await.map_err(Error::Unproven)?;
        Ok(control.bind_repo(Binding { site, proof, at }).await?)
    }

    /// A new webhook secret for the claim of `repo` by `site`; the old one stops verifying.
    pub async fn rotate<P: Ports, D: Draw>(
        &self,
        control: &Control<P>,
        draw: &D,
        asking: (Id<SiteKind>, Repo, Timestamp),
    ) -> Result<Claimed, Error<P, T::Error, D::Error>> {
        let (site, repo, at) = asking;
        let (secret, credential) = Self::secret(draw)?;
        let rotating = Rotating { site, repo, secret: Plain::from(secret.reveal().to_vec()), credential, at };
        Ok(Claimed { claim: control.rotate_repo(rotating).await?, secret })
    }

    /// Lets go of the claim of `repo` by `site`.
    pub async fn release<P: Ports>(
        &self,
        control: &Control<P>,
        asking: (Id<SiteKind>, Repo, Timestamp),
    ) -> Result<(), Error<P, T::Error, std::convert::Infallible>> {
        let (site, repo, at) = asking;
        Ok(control.release_repo(Releasing { site, repo, at }).await?)
    }

    /// A secret of 32 random bytes in hex, and the id of the credential that will seal it.
    fn secret<P: Ports, D: Draw>(draw: &D) -> Fresh<P, T, D> {
        let (first, second) = (draw.bytes().map_err(Error::Draw)?, draw.bytes().map_err(Error::Draw)?);
        let id = Id::from_bytes(draw.bytes().map_err(Error::Draw)?);
        let both: Vec<u8> = first.iter().chain(&second).copied().collect();
        Ok((Secret::new(Hex::of(&both)), id))
    }
}
