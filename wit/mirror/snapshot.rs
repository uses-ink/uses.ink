// What a source hands the engine, and the git capability's values. Part of `wit/mirror`.

use model::git::{Auth, Budget, Cloned, Depth, Grant, Locator, Network, Refused, Request, Revision, Scheme};
use model::{
    Ability, Bytes as Size, ContentPath, Entry, FileFacts, Gap, Hash, Known, Lack, Observed, Oid, Provenance, Snapshot,
    Timestamp, UnknownReason, UploadId, Version,
};

use super::refusal::{Malformed, Unmirrored};
use super::wit::host::git as fetched;
use super::wit::model::snapshot as wire;
use super::written::{Bytes, Written};

impl From<Timestamp> for wire::Instant {
    fn from(at: Timestamp) -> Self {
        Self { seconds: at.as_second(), nanos: u32::try_from(at.subsec_nanosecond()).unwrap_or(0) }
    }
}

impl TryFrom<wire::Instant> for Timestamp {
    type Error = Malformed;

    fn try_from(at: wire::Instant) -> Result<Self, Malformed> {
        let nanos = i32::try_from(at.nanos).map_err(|_| Malformed::Instant { seconds: at.seconds, nanos: at.nanos })?;
        Timestamp::new(at.seconds, nanos).map_err(|_| Malformed::Instant { seconds: at.seconds, nanos: at.nanos })
    }
}

impl TryFrom<&Provenance> for wire::Provenance {
    type Error = Unmirrored;

    fn try_from(from: &Provenance) -> Result<Self, Unmirrored> {
        Ok(match from {
            Provenance::GitHistory { commit } => Self::GitHistory(commit.to_string()),
            Provenance::Commit { commit } => Self::Commit(commit.to_string()),
            Provenance::Filesystem => Self::Filesystem,
            _ => return Err(Unmirrored::Provenance),
        })
    }
}

impl TryFrom<wire::Provenance> for Provenance {
    type Error = Malformed;

    fn try_from(from: wire::Provenance) -> Result<Self, Malformed> {
        Ok(match from {
            wire::Provenance::GitHistory(commit) => Self::GitHistory { commit: Written::read(commit)? },
            wire::Provenance::Commit(commit) => Self::Commit { commit: Written::read(commit)? },
            wire::Provenance::Filesystem => Self::Filesystem,
        })
    }
}

impl TryFrom<&Known<Timestamp>> for wire::Known {
    type Error = Unmirrored;

    fn try_from(known: &Known<Timestamp>) -> Result<Self, Unmirrored> {
        let stamped = |value: &Timestamp, from: &Provenance| {
            Ok::<_, Unmirrored>(wire::Stamped { at: (*value).into(), provenance: from.try_into()? })
        };
        Ok(match known {
            Known::Exact { value, from } => Self::Exact(stamped(value, from)?),
            Known::NotAfter { value, from } => Self::NotAfter(stamped(value, from)?),
            Known::Unknown { reason } => Self::Unknown(match reason {
                UnknownReason::NoHistory => wire::Unknown::NoHistory,
                UnknownReason::BeyondDepth { depth } => wire::Unknown::BeyondDepth(*depth),
                UnknownReason::NotReported => wire::Unknown::NotReported,
                UnknownReason::Merged => wire::Unknown::Merged,
                _ => return Err(Unmirrored::Known),
            }),
            _ => return Err(Unmirrored::Known),
        })
    }
}

impl TryFrom<wire::Known> for Known<Timestamp> {
    type Error = Malformed;

    fn try_from(known: wire::Known) -> Result<Self, Malformed> {
        let stamped = |stamped: wire::Stamped| Ok::<_, Malformed>((stamped.at.try_into()?, stamped.provenance.try_into()?));
        Ok(match known {
            wire::Known::Exact(fact) => {
                let (value, from) = stamped(fact)?;
                Self::Exact { value, from }
            }
            wire::Known::NotAfter(fact) => {
                let (value, from) = stamped(fact)?;
                Self::NotAfter { value, from }
            }
            wire::Known::Unknown(reason) => Self::Unknown {
                reason: match reason {
                    wire::Unknown::NoHistory => UnknownReason::NoHistory,
                    wire::Unknown::BeyondDepth(depth) => UnknownReason::BeyondDepth { depth },
                    wire::Unknown::NotReported => UnknownReason::NotReported,
                    wire::Unknown::Merged => UnknownReason::Merged,
                },
            },
        })
    }
}

impl TryFrom<Ability> for wire::Ability {
    type Error = Unmirrored;

    fn try_from(ability: Ability) -> Result<Self, Unmirrored> {
        Ok(match ability {
            Ability::Snapshot => Self::Snapshot,
            Ability::History => Self::History,
            Ability::Watch => Self::Watch,
            Ability::Private => Self::Private,
            _ => return Err(Unmirrored::Ability),
        })
    }
}

impl From<wire::Ability> for Ability {
    fn from(ability: wire::Ability) -> Self {
        match ability {
            wire::Ability::Snapshot => Self::Snapshot,
            wire::Ability::History => Self::History,
            wire::Ability::Watch => Self::Watch,
            wire::Ability::Private => Self::Private,
        }
    }
}

impl TryFrom<&Lack> for wire::Lack {
    type Error = Unmirrored;

    fn try_from(lack: &Lack) -> Result<Self, Unmirrored> {
        let gap = match &lack.gap {
            Gap::Absent => wire::Gap::Absent,
            Gap::Shallow { commit, bound } => {
                wire::Gap::Shallow(wire::Shallowed { commit: commit.to_string(), bound: (*bound).into() })
            }
            Gap::Renames { moved } => wire::Gap::Renames(*moved),
            _ => return Err(Unmirrored::Gap),
        };
        Ok(Self { ability: lack.ability.try_into()?, consequence: lack.consequence.to_string(), gap })
    }
}

impl TryFrom<wire::Lack> for Lack {
    type Error = Malformed;

    fn try_from(lack: wire::Lack) -> Result<Self, Malformed> {
        let gap = match lack.gap {
            wire::Gap::Absent => Gap::Absent,
            wire::Gap::Shallow(shallowed) => {
                Gap::Shallow { commit: Written::read(shallowed.commit)?, bound: shallowed.bound.try_into()? }
            }
            wire::Gap::Renames(moved) => Gap::Renames { moved },
        };
        Ok(Self::new(lack.ability.into(), Written::read(lack.consequence)?, gap))
    }
}

impl TryFrom<&Snapshot> for wire::Snapshot {
    type Error = Unmirrored;

    fn try_from(snapshot: &Snapshot) -> Result<Self, Unmirrored> {
        let version = match snapshot.version() {
            Version::GitCommit(commit) => wire::Version::GitCommit(commit.to_string()),
            Version::Upload(id) => wire::Version::Upload(id.as_str().to_owned()),
            Version::Folder { hash } => wire::Version::Folder(hash.as_bytes().to_vec()),
            _ => return Err(Unmirrored::Version),
        };
        let tree = snapshot
            .tree()
            .iter()
            .map(|entry| wire::Entry {
                path: entry.path.to_string(),
                blob: entry.blob.as_bytes().to_vec(),
                size: entry.size.count(),
            })
            .collect();
        let facts = snapshot
            .facts()
            .iter()
            .map(|facts| {
                Ok(wire::Facts {
                    path: facts.path.to_string(),
                    created: (&facts.created).try_into()?,
                    updated: (&facts.updated).try_into()?,
                })
            })
            .collect::<Result<_, Unmirrored>>()?;
        let observed = snapshot.observed();
        let has = observed.has.iter().map(|ability| wire::Ability::try_from(*ability)).collect::<Result<_, _>>()?;
        let lacks = observed.lacks.iter().map(wire::Lack::try_from).collect::<Result<_, _>>()?;
        Ok(Self { version, tree, facts, observed: wire::Observed { has, lacks } })
    }
}

impl TryFrom<wire::Snapshot> for Snapshot {
    type Error = Malformed;

    fn try_from(snapshot: wire::Snapshot) -> Result<Self, Malformed> {
        let version = match snapshot.version {
            wire::Version::GitCommit(commit) => Version::GitCommit(Written::read(commit)?),
            wire::Version::Upload(id) => {
                Version::Upload(UploadId::parse(id).map_err(|source| Malformed::Version { source })?)
            }
            wire::Version::Folder(hash) => Version::Folder { hash: Hash::try_from(Bytes(hash))? },
        };
        let tree = snapshot
            .tree
            .into_iter()
            .map(|entry| {
                Ok(Entry::new(
                    ContentPath::try_from(Written(entry.path))?,
                    Hash::try_from(Bytes(entry.blob))?,
                    Size(entry.size),
                ))
            })
            .collect::<Result<Vec<_>, Malformed>>()?;
        let facts = snapshot
            .facts
            .into_iter()
            .map(|facts| {
                Ok(FileFacts::new(
                    ContentPath::try_from(Written(facts.path))?,
                    facts.created.try_into()?,
                    facts.updated.try_into()?,
                ))
            })
            .collect::<Result<Vec<_>, Malformed>>()?;
        let has = snapshot.observed.has.into_iter().map(Ability::from).collect();
        let lacks = snapshot.observed.lacks.into_iter().map(Lack::try_from).collect::<Result<_, _>>()?;
        Snapshot::new(version, tree, facts, Observed::new(has, lacks)).map_err(|source| Malformed::Snapshot { source })
    }
}

impl TryFrom<Scheme> for fetched::Scheme {
    type Error = Unmirrored;

    fn try_from(scheme: Scheme) -> Result<Self, Unmirrored> {
        match scheme {
            Scheme::File => Ok(Self::File),
            Scheme::Https => Ok(Self::Https),
            _ => Err(Unmirrored::Fetch),
        }
    }
}

impl From<fetched::Scheme> for Scheme {
    fn from(scheme: fetched::Scheme) -> Self {
        match scheme {
            fetched::Scheme::File => Self::File,
            fetched::Scheme::Https => Self::Https,
        }
    }
}

impl TryFrom<Network> for fetched::Network {
    type Error = Unmirrored;

    fn try_from(network: Network) -> Result<Self, Unmirrored> {
        match network {
            Network::Off => Ok(Self::Off),
            Network::Local => Ok(Self::Local),
            Network::On => Ok(Self::On),
            _ => Err(Unmirrored::Fetch),
        }
    }
}

impl From<fetched::Network> for Network {
    fn from(network: fetched::Network) -> Self {
        match network {
            fetched::Network::Off => Self::Off,
            fetched::Network::Local => Self::Local,
            fetched::Network::On => Self::On,
        }
    }
}

impl From<&Request> for fetched::Request {
    fn from(request: &Request) -> Self {
        let depth = match request.depth {
            Depth::Commits(count) => fetched::Depth::Commits(count.get()),
            _ => fetched::Depth::Full,
        };
        Self {
            locator: request.locator.to_string(),
            revision: request.revision.as_ref().map(ToString::to_string),
            depth,
            grant: request.grant.as_ref().map(ToString::to_string),
        }
    }
}

impl TryFrom<fetched::Request> for Request {
    type Error = Malformed;

    fn try_from(request: fetched::Request) -> Result<Self, Malformed> {
        let locator =
            request.locator.parse::<Locator>().map_err(|source| Malformed::Locator { source })?;
        let name = |source| Malformed::Name { source };
        let revision = request.revision.map(|text| text.parse::<Revision>().map_err(name)).transpose()?;
        let grant = request.grant.map(|text| text.parse::<Grant>().map_err(name)).transpose()?;
        let depth = match request.depth {
            fetched::Depth::Full => Depth::Full,
            fetched::Depth::Commits(count) => {
                std::num::NonZeroU32::new(count).map_or(Depth::Full, Depth::Commits)
            }
        };
        let mut made = Request::new(locator);
        (made.revision, made.depth, made.grant) = (revision, depth, grant);
        Ok(made)
    }
}

impl From<&Cloned> for fetched::Cloned {
    fn from(cloned: &Cloned) -> Self {
        Self {
            clone: cloned.clone.as_bytes().to_vec(),
            commit: cloned.commit.to_string(),
            shallow: cloned.shallow,
            objects: cloned.objects,
            bytes: cloned.bytes.count(),
        }
    }
}

impl TryFrom<fetched::Cloned> for Cloned {
    type Error = Malformed;

    fn try_from(cloned: fetched::Cloned) -> Result<Self, Malformed> {
        Ok(Self {
            clone: Hash::try_from(Bytes(cloned.clone))?,
            commit: Written::read(cloned.commit)?,
            shallow: cloned.shallow,
            objects: cloned.objects,
            bytes: Size(cloned.bytes),
        })
    }
}

impl<E> TryFrom<&Refused<E>> for fetched::Refusal {
    type Error = Unmirrored;

    fn try_from(refused: &Refused<E>) -> Result<Self, Unmirrored> {
        Ok(match refused {
            Refused::Network { scheme, policy } => {
                Self::Network(fetched::Forbidden { scheme: (*scheme).try_into()?, policy: (*policy).try_into()? })
            }
            Refused::Auth { grant, outcome } => Self::Auth(fetched::Unauthorised {
                grant: grant.to_string(),
                outcome: match outcome {
                    Auth::Absent => fetched::Auth::Absent,
                    _ => return Err(Unmirrored::Fetch),
                },
            }),
            Refused::Budget(Budget::Bytes { size, limit }) => {
                Self::Budget(fetched::Budget::Bytes(fetched::Limited { size: size.count(), limit: limit.count() }))
            }
            Refused::Budget(Budget::Objects { count, limit }) => {
                Self::Budget(fetched::Budget::Objects(fetched::Counted { count: *count, limit: *limit }))
            }
            Refused::Revision { written } => Self::Revision(written.to_string()),
            Refused::Transport { scheme } => Self::Transport((*scheme).try_into()?),
            Refused::Failed(_) => Self::Failed,
            _ => return Err(Unmirrored::Fetch),
        })
    }
}

/// A refusal from the WIT, with the failure its `failed` case stands for: the host said why, so
/// the plugin's side holds only a marker.
#[derive(Debug)]
pub(crate) struct Seen<E>(pub(crate) fetched::Refusal, pub(crate) E);

impl<E> TryFrom<Seen<E>> for Refused<E> {
    type Error = Malformed;

    fn try_from(Seen(refusal, failure): Seen<E>) -> Result<Self, Malformed> {
        let name = |source| Malformed::Name { source };
        Ok(match refusal {
            fetched::Refusal::Network(forbidden) => {
                Self::Network { scheme: forbidden.scheme.into(), policy: forbidden.policy.into() }
            }
            fetched::Refusal::Auth(unauthorised) => Self::Auth {
                grant: unauthorised.grant.parse().map_err(name)?,
                outcome: match unauthorised.outcome {
                    fetched::Auth::Absent => Auth::Absent,
                },
            },
            fetched::Refusal::Budget(fetched::Budget::Bytes(limited)) => {
                Self::Budget(Budget::Bytes { size: Size(limited.size), limit: Size(limited.limit) })
            }
            fetched::Refusal::Budget(fetched::Budget::Objects(counted)) => {
                Self::Budget(Budget::Objects { count: counted.count, limit: counted.limit })
            }
            fetched::Refusal::Revision(written) => Self::Revision { written: written.parse().map_err(name)? },
            fetched::Refusal::Transport(scheme) => Self::Transport { scheme: scheme.into() },
            fetched::Refusal::Failed => Self::Failed(failure),
        })
    }
}
