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

/// Which forge a repository lives on, and so whose webhook tells of its pushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forge {
    /// `github.com`.
    Github,
    /// An instance on the operator's list of Forgejo instances.
    Forgejo,
    /// A host on the operator's list of GitLab hosts.
    Gitlab,
}

impl Forge {
    /// Every forge.
    pub const ALL: [Self; 3] = [Self::Github, Self::Forgejo, Self::Gitlab];

    /// The name the webhook path and the API write: `github`, `forgejo`, `gitlab`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Forgejo => "forgejo",
            Self::Gitlab => "gitlab",
        }
    }
}

impl std::fmt::Display for Forge {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.name())
    }
}

/// Text that names no forge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unknown {
    /// What was written.
    pub written: String,
}

impl std::fmt::Display for Unknown {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "`{}` is no forge: it is github, forgejo or gitlab", self.written)
    }
}

impl std::error::Error for Unknown {}

impl std::str::FromStr for Forge {
    type Err = Unknown;

    fn from_str(text: &str) -> Result<Self, Unknown> {
        Self::ALL.into_iter().find(|forge| forge.name() == text).ok_or_else(|| Unknown { written: text.to_owned() })
    }
}

/// The repository authority over one transport.
#[derive(Debug)]
pub struct Repos<T> {
    github: Open<Shared<T>>,
    forgejo: Forgejo<Shared<T>>,
    gitlab: Gitlab<Shared<T>>,
    hosts: Vec<Host>,
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
            hosts: forges.gitlab,
        }
    }

    /// The transport, for a test to read what a fake was asked.
    pub fn transport(&self) -> &T {
        self.github.client().transport().inner()
    }

    /// Reads the challenge file of `repo` through the forge it lives on. A host no forge lists is
    /// [`Unproven::Unlisted`] and nothing is sent.
    pub async fn read(&self, repo: &Repo, challenge: &Challenge) -> Result<Proven, Unproven<T::Error>> {
        match self.forge(repo) {
            Some(Forge::Github) => self.github.prove(repo, challenge).await,
            Some(Forge::Forgejo) => self.forgejo.prove(repo, challenge).await,
            Some(Forge::Gitlab) => self.gitlab.prove(repo, challenge).await,
            None => Err(Unproven::Unlisted { host: repo.host.clone() }),
        }
    }

    /// The forge `repo` lives on, when its host is one the operator listed.
    pub fn forge(&self, repo: &Repo) -> Option<Forge> {
        if repo.host == Host::GITHUB {
            Some(Forge::Github)
        } else if self.forgejo.instances().find(&repo.host).is_some() {
            Some(Forge::Forgejo)
        } else if self.hosts.contains(&repo.host) {
            Some(Forge::Gitlab)
        } else {
            None
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
