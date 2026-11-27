//! The control plane behind `ink serve --data`: its adapters, its operator, its limits, as one value.
//!
//! The adapters are SQLite in `<data>/ink.db`, a directory of blobs in `<data>/blobs`, the queue
//! in the same database, and a sealing key in `<data>/secret.key`. The one owner is an account
//! with a fixed id. Its credential is a line in `<data>/operator-keys`, a public key that
//! `ink login --data <folder>` adds: the server holds no private key and prints no secret. A
//! `<data>/owner-token` left by an older version is not read, and the start says so.

use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use blob_dir::Directory;
use clock::System;
use control::signin::{Entry, Settings, SignIn};
use control::{AccountKind, Control, Domain, Id, NewAccount, Origin, Refusal, Remembered};
use db::Steps;
use db_sqlite::Sqlite;
use edge_api::door::Config;
use edge_api::{Api, Courier, Door, Draws, Enrol, Limits, OperatorFile, Outbox, Rates, Routed, Signatures, Signing};
use edge_control::{Back, Fronts};
use mail::{Base, Link};
use queue_local::{Local as Queue, Queued};
use secret_local::Local as Sealer;

use crate::args;
use crate::signin::{Flags, Sender, Told};

/// The adapters of the control plane `ink serve` runs.
pub(crate) type Parts = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

/// The ports of the control plane `ink serve` runs, shared by the API, the lookups of the edge and
/// the loops that run beside them.
pub(crate) type Ports = Arc<Parts>;

/// The half of the edge's lookup that holds a control plane.
pub(crate) type Lookup = Back<Ports, System, Directory>;

/// The database the API and the sign-in share.
pub(crate) type Store = Arc<Sqlite>;

/// Who may call: a signature of the operator's key, or of a key a person approved.
pub(crate) type Callers = Signatures<Store, System, OperatorFile>;

/// The API as `ink serve` wires it, with the sign-in routes beside it.
pub(crate) type Wired = Routed<Api<Ports, Callers, System, System>, Door<Store, System, System>>;

/// Why the control plane could not be set up.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unready {
    /// A file or folder of the data folder could not be made or read.
    Data {
        /// Where.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// An adapter or the migration failed.
    Adapter {
        /// Which part.
        part: &'static str,
        /// Why.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// A setting is wrong.
    Setting {
        /// Its flag.
        flag: &'static str,
        /// Why.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl std::fmt::Display for Unready {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data { path, .. } => write!(out, "{} could not be made or read", path.display()),
            Self::Adapter { part, .. } => write!(out, "the {part} could not be set up"),
            Self::Setting { flag, .. } => write!(out, "{flag} is not usable"),
        }
    }
}

impl std::error::Error for Unready {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Data { source, .. } => Some(source),
            Self::Adapter { source, .. } | Self::Setting { source, .. } => Some(source.as_ref()),
        }
    }
}

/// What sign-in runs with: the mail sender, and every setting it was given, for the start-up
/// listing.
pub(crate) struct Mailing {
    pub(crate) courier: Courier<Sender, System>,
    pub(crate) told: Told,
    pub(crate) settings: Settings,
    pub(crate) rates: Rates,
    pub(crate) base: Base,
    pub(crate) outbox: usize,
    pub(crate) secure: bool,
    /// The first-run link, when this start made it. It is shown once and kept nowhere.
    pub(crate) admin: Option<Link>,
}

/// The control plane, its API, and what was decided on the way.
pub(crate) struct Product {
    pub(crate) api: Wired,
    pub(crate) mailing: Mailing,
    pub(crate) limits: Limits,
    pub(crate) interval: watch::Interval,
    /// The operator's keys file.
    pub(crate) operators: PathBuf,
    /// What the keys and signatures run with.
    pub(crate) keys: crate::auth::Resolved,
    /// The ports the loops run over, shared with the API and the lookups.
    pub(crate) ports: Ports,
    /// Where the control plane says its sites are served.
    pub(crate) origin: Origin,
    /// The data folder.
    pub(crate) data: PathBuf,
    /// How the loops reach an https git remote.
    pub(crate) https: https::Https,
    /// The edge's way to the control plane's sites.
    pub(crate) fronts: Fronts,
    /// The half that answers the edge's lookups.
    pub(crate) back: Option<Lookup>,
    /// The owner token file of an older version, when one is there: it no longer works.
    pub(crate) retired: Option<PathBuf>,
}

impl Product {
    /// The owner's account: one fixed id, since a self-host has one person.
    const OWNER: [u8; 16] = *b"ink-serve-owner\0";

    fn adapter(part: &'static str) -> impl FnOnce(Box<dyn std::error::Error + Send + Sync>) -> Unready {
        move |source| Unready::Adapter { part, source }
    }

    /// Sets the control plane up in `settings.data`, migrating the database and making the owner.
    #[expect(clippy::too_many_lines, reason = "one sequence of adapters, each a line")]
    pub(crate) fn open(settings: &args::Serve, data: &Path) -> Result<Self, Unready> {
        std::fs::create_dir_all(data).map_err(|source| Unready::Data { path: data.to_path_buf(), source })?;
        let file = data.join("ink.db");
        let db = Self::database(&file)?;
        let blobs = Directory::open(data.join("blobs")).map_err(|e| Self::adapter("blob store")(Box::new(e)))?;
        let sealer = Sealer::beside(&file).map_err(|e| Self::adapter("sealing key")(Box::new(e)))?;
        let origin = Self::origin(settings)?;
        let ports: Ports = Arc::new((db.clone(), blobs, Queue::new(db.clone()), sealer));
        let control = Control::new(Arc::clone(&ports), origin.clone());
        let account = Self::owner(&control)?;
        let (operators, retired, keys) = Self::credentials(settings, data)?;
        let limits = Self::limits(settings)?;
        let flags = &settings.signin;
        let secure = Flags::secure(&settings.origin)?;
        let base = flags.base(&settings.origin, &settings.api_host)?;
        let (sign, rates) = (flags.settings(), flags.rates());
        let (courier, outbox, capacity) = Self::courier(flags)?;
        let db_for_keys = Arc::clone(&db);
        let entry = SignIn::new(db, sign);
        let admin = Self::admin(&entry, account, &base)?;
        let (api, enrol) = Self::api(control, &keys, (account, &operators), (db_for_keys, limits));
        let api = api.with_schemes(settings.hosting.schemes());
        let (fronts, back) = Self::lookups(&ports, &origin, data, settings)?;
        let config =
            Config { jar: flags.jar(secure)?, base: base.clone(), rates, outbox, reveals: flags.login_get_signs_in };
        let door = Door::new(entry, (System, System), enrol, config);
        let told = Told(std::sync::Mutex::new(std::io::stderr()));
        let mailing = Mailing { courier, told, settings: sign, rates, base, outbox: capacity, secure, admin };
        Ok(Self {
            api: Routed::new(api, door),
            mailing,
            limits,
            interval: Self::interval(settings)?,
            operators,
            keys,
            ports,
            origin,
            data: data.to_path_buf(),
            https: Self::https(settings)?,
            fronts,
            back: Some(back),
            retired,
        })
    }

    /// The database at `file`, with every migration applied.
    fn database(file: &Path) -> Result<Store, Unready> {
        let db = Arc::new(Sqlite::open(file).map_err(|e| Self::adapter("database")(Box::new(e)))?);
        let steps = Steps::new()
            .with::<Domain>()
            .and_then(Steps::with::<Remembered>)
            .and_then(Steps::with::<Queued>)
            .and_then(Steps::with::<Entry>)
            .and_then(Steps::with::<control::keys::Entry>);
        let steps = steps.map_err(|e| Self::adapter("migration")(Box::new(e)))?;
        pollster::block_on(steps.apply(&db)).map_err(|e| Self::adapter("migration")(Box::new(e)))?;
        Ok(db)
    }

    /// Where the control plane says its sites are served, from `--origin`.
    fn origin(settings: &args::Serve) -> Result<Origin, Unready> {
        let url = model::ExternalUrl::parse(settings.origin.clone())
            .map_err(|e| Unready::Setting { flag: "--origin", source: Box::new(e) })?;
        Origin::try_from(&url).map_err(|e| Unready::Setting { flag: "--origin", source: Box::new(e) })
    }

    /// The mail courier, its outbox and the outbox's capacity.
    fn courier(flags: &Flags) -> Result<(Courier<Sender, System>, Outbox, usize), Unready> {
        let capacity = flags.outbox.unwrap_or(Flags::OUTBOX);
        let outbox = Outbox::new(capacity);
        let rest = flags.courier_rest.unwrap_or(Flags::REST).duration();
        Ok((Courier::new(outbox.clone(), flags.sender()?, System, rest), outbox, capacity))
    }

    /// The edge's lookup over its own control plane and blob directory.
    fn lookups(
        ports: &Ports,
        origin: &Origin,
        data: &Path,
        settings: &args::Serve,
    ) -> Result<(Fronts, Lookup), Unready> {
        let control = Control::new(Arc::clone(ports), origin.clone());
        let blobs = Directory::open(data.join("blobs")).map_err(|e| Self::adapter("blob store")(Box::new(e)))?;
        Ok(Back::pair(control, System, blobs, settings.hosting.lookup()))
    }

    /// The owner's account, made when it is not there.
    fn owner(control: &Control<Ports>) -> Result<Id<AccountKind>, Unready> {
        let account: Id<AccountKind> = Id::from_bytes(Self::OWNER);
        let added = pollster::block_on(control.add_account(NewAccount { id: account, at: clock::Clock::now(&System) }));
        match added {
            Ok(_) => Ok(account),
            Err(error) if matches!(error.refusal(), Some(Refusal::Exists { .. })) => Ok(account),
            Err(error) => Err(Self::adapter("owner account")(Box::new(error))),
        }
    }

    /// The first-run link, made once: `None` when one was made before or an account can already
    /// sign in. The link is returned here and held nowhere else; the database has its digest.
    fn admin(entry: &SignIn<Store>, account: Id<AccountKind>, base: &Base) -> Result<Option<Link>, Unready> {
        let drawn = Draws(&System).drawn().map_err(|e| Self::adapter("randomness")(Box::new(e)))?;
        let issued = pollster::block_on(entry.admin(Some(account), drawn, clock::Clock::now(&System)))
            .map_err(|e| Self::adapter("first-run link")(Box::new(e)))?;
        Ok(issued.map(|issued| base.link(issued.proof.reveal())))
    }

    /// The operator's keys file, the owner token file of an older version when one is there, and
    /// what signatures run with.
    fn credentials(
        settings: &args::Serve,
        data: &Path,
    ) -> Result<(PathBuf, Option<PathBuf>, crate::auth::Resolved), Unready> {
        let operators = Self::operators(&data.join("operator-keys"))?;
        let old = data.join("owner-token");
        let keys = settings.auth.resolve(&settings.api_host, settings.address)?;
        Ok((operators, old.exists().then_some(old), keys))
    }

    /// The API over signatures, and the enrolment its door shares.
    fn api(
        control: Control<Ports>,
        keys: &crate::auth::Resolved,
        operator: (Id<AccountKind>, &Path),
        parts: (Store, Limits),
    ) -> (Api<Ports, Callers, System, System>, Enrol<Store>) {
        let (account, operators) = operator;
        let (db, limits) = parts;
        let enrol = Enrol::new(db, keys.keys, keys.closing);
        let mut signing = Signing::new(keys.audience.clone(), account);
        signing.window = keys.window;
        signing.nonces = keys.nonces;
        let signatures = Signatures::new(enrol.keys().clone(), (System, OperatorFile::at(operators)), signing);
        (Api::new(control, (signatures, enrol.clone()), (System, System), limits), enrol)
    }

    /// The operator's keys file, made empty when it is not there so its place is known. It holds
    /// public keys only, so a backup of the data folder leaks no credential.
    fn operators(path: &Path) -> Result<PathBuf, Unready> {
        let made = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path);
        match made {
            Ok(_) => Ok(path.to_path_buf()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(path.to_path_buf()),
            Err(source) => Err(Unready::Data { path: path.to_path_buf(), source }),
        }
    }

    /// The operator's limits: the flags, over the defaults.
    fn limits(settings: &args::Serve) -> Result<Limits, Unready> {
        let ceiling = Limits::default();
        let asked = edge_api::Asked::default();
        let mut limits = ceiling;
        limits.body = settings.api_body.unwrap_or(ceiling.body);
        limits.archive = settings.upload_bytes.unwrap_or(ceiling.archive);
        limits.files = settings.upload_files.unwrap_or(ceiling.files);
        limits.file = settings.upload_file.unwrap_or(ceiling.file);
        limits.export = settings.export_bytes.unwrap_or(ceiling.export);
        limits.lower(&asked).map_err(|e| Unready::Setting { flag: "the limits", source: Box::new(e) })
    }

    /// How the loops reach an https git remote: the flags, over the defaults.
    fn https(settings: &args::Serve) -> Result<https::Https, Unready> {
        settings.hosting.https().map_err(|e| Unready::Setting { flag: "--git-ca", source: Box::new(e) })
    }

    fn interval(settings: &args::Serve) -> Result<watch::Interval, Unready> {
        let defaults = watch::Interval::default();
        let base = settings.poll_interval.map_or(defaults.base(), Duration::from_secs);
        let max = settings.poll_max.map_or(defaults.max(), Duration::from_secs);
        let interval = watch::Interval::new(base, max)
            .map_err(|e| Unready::Setting { flag: "--poll-interval", source: Box::new(e) })?;
        let lookout = settings.hosting.poll_lookout.map_or(defaults.lookout(), Duration::from_secs);
        interval.with_lookout(lookout).map_err(|e| Unready::Setting { flag: "--poll-lookout", source: Box::new(e) })
    }
}

impl std::fmt::Debug for Product {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Product(..)")
    }
}
