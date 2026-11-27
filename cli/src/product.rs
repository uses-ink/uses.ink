//! The control plane behind `ink serve --data`: its adapters, its owner, its limits, as one value.
//!
//! The adapters are SQLite in `<data>/ink.db`, a directory of blobs in `<data>/blobs`, the queue
//! in the same database, and a sealing key in `<data>/secret.key`. The one owner is an account
//! with a fixed id, and holds one token, kept in `<data>/owner-token` (mode 0600) and shown once,
//! when it is made.

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use blob_dir::Directory;
use clock::System;
use control::signin::{Entry, Settings, SignIn};
use control::{AccountKind, Control, Domain, Id, NewAccount, Origin, Refusal};
use db::Steps;
use db_sqlite::Sqlite;
use edge_api::door::Config;
use edge_api::{Api, Courier, Door, Draws, Either, Limits, Outbox, Owner, Rates, Routed, Sessions, Token};
use mail::{Base, Link};
use queue_local::{Local as Queue, Queued};
use secret_local::Local as Sealer;

use crate::args;
use crate::signin::{Flags, Sender, Told};

/// The ports of the control plane `ink serve` runs.
pub(crate) type Ports = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

/// The database the API and the sign-in share.
pub(crate) type Store = Arc<Sqlite>;

/// Who may call: the operator's owner token, or a session that signing in made.
pub(crate) type Callers = Either<Owner, Sessions<Store, System>>;

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
    pub(crate) token: PathBuf,
    /// The token, when this start made it.
    pub(crate) fresh: Option<Token>,
}

impl Product {
    /// The owner's account: one fixed id, since a self-host has one person.
    const OWNER: [u8; 16] = *b"ink-serve-owner\0";

    fn adapter(part: &'static str) -> impl FnOnce(Box<dyn std::error::Error + Send + Sync>) -> Unready {
        move |source| Unready::Adapter { part, source }
    }

    /// Sets the control plane up in `settings.data`, migrating the database and making the owner.
    pub(crate) fn open(settings: &args::Serve, data: &Path) -> Result<Self, Unready> {
        std::fs::create_dir_all(data).map_err(|source| Unready::Data { path: data.to_path_buf(), source })?;
        let file = data.join("ink.db");
        let db = Arc::new(Sqlite::open(&file).map_err(|e| Self::adapter("database")(Box::new(e)))?);
        let steps = Steps::new().with::<Domain>().and_then(Steps::with::<Queued>).and_then(Steps::with::<Entry>);
        let steps = steps.map_err(|e| Self::adapter("migration")(Box::new(e)))?;
        pollster::block_on(steps.apply(&db)).map_err(|e| Self::adapter("migration")(Box::new(e)))?;
        let blobs = Directory::open(data.join("blobs")).map_err(|e| Self::adapter("blob store")(Box::new(e)))?;
        let sealer = Sealer::beside(&file).map_err(|e| Self::adapter("sealing key")(Box::new(e)))?;
        let origin = model::ExternalUrl::parse(settings.origin.clone())
            .map_err(|e| Unready::Setting { flag: "--origin", source: Box::new(e) })?;
        let origin =
            Origin::try_from(&origin).map_err(|e| Unready::Setting { flag: "--origin", source: Box::new(e) })?;
        let control = Control::new((db.clone(), blobs, Queue::new(db.clone()), sealer), origin);
        let account = Self::owner(&control)?;
        let (token, fresh) = Self::token(&data.join("owner-token"))?;
        let limits = Self::limits(settings)?;
        let interval = Self::interval(settings)?;
        let flags = &settings.signin;
        let secure = Flags::secure(&settings.origin)?;
        let base = flags.base(&settings.origin, &settings.api_host)?;
        let (sign, rates) = (flags.settings(), flags.rates());
        let capacity = flags.outbox.unwrap_or(Flags::OUTBOX);
        let outbox = Outbox::new(capacity);
        let rest = flags.courier_rest.unwrap_or(Flags::REST).duration();
        let courier = Courier::new(outbox.clone(), flags.sender()?, System, rest);
        let entry = SignIn::new(db, sign);
        let admin = Self::admin(&entry, account, &base)?;
        let credentials = Either(Owner::new(account, &token), Sessions::new(entry.clone(), System));
        let api = Api::new(control, credentials, (System, System), limits);
        let config =
            Config { jar: flags.jar(secure)?, base: base.clone(), rates, outbox, reveals: flags.login_get_signs_in };
        let door = Door::new(entry, (System, System), config);
        let told = Told(std::sync::Mutex::new(std::io::stderr()));
        let mailing = Mailing { courier, told, settings: sign, rates, base, outbox: capacity, secure, admin };
        Ok(Self { api: Routed::new(api, door), mailing, limits, interval, token: data.join("owner-token"), fresh })
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

    /// The owner's token: read from its file, or made and written there with mode 0600.
    fn token(path: &Path) -> Result<(Token, Option<Token>), Unready> {
        let io = |source| Unready::Data { path: path.to_path_buf(), source };
        match std::fs::read_to_string(path) {
            Ok(text) => Ok((Token::from(text.trim()), None)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let token = Token::drawn(&System).map_err(|e| Self::adapter("randomness")(Box::new(e)))?;
                let mut file =
                    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(io)?;
                file.write_all(token.reveal().as_bytes()).map_err(io)?;
                Ok((token.clone(), Some(token)))
            }
            Err(error) => Err(io(error)),
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

    fn interval(settings: &args::Serve) -> Result<watch::Interval, Unready> {
        let defaults = watch::Interval::default();
        let base = settings.poll_interval.map_or(defaults.base(), Duration::from_secs);
        let max = settings.poll_max.map_or(defaults.max(), Duration::from_secs);
        watch::Interval::new(base, max).map_err(|e| Unready::Setting { flag: "--poll-interval", source: Box::new(e) })
    }
}

impl std::fmt::Debug for Product {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Product(..)")
    }
}
