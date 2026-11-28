//! What one isolate holds: the ports on the bindings, the API over them, and the serve path.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use blob_r2::R2;
use control::{Control, Origin};
use db_d1::D1;
use edge::{Edge, Limits};
use edge_api::Limits as Budget;
use edge_worker::{Calls, Hosting, Platform, Remote, Serve, Sited, Stored, Unready, Vars, Wired};
use queue_cf::{Cf, Queues};
use queue_rows::Local;
use secret_worker::Worker;
use worker::{Env, Request, Response, console_error};

/// The row adapter, shared by every port that reads rows.
type Store = Arc<D1>;

/// The control plane's adapters, one set for the API and the serve path.
type Ports = Arc<(Store, R2, Cf<Store, Queues>, Worker)>;

/// The API over the Worker's ports and the nonce object.
type Answers = Wired<Ports, Remote, Platform, Platform>;

/// Why a request could not be started: a variable or a binding.
#[derive(Debug)]
pub(crate) enum Unstarted {
    /// A variable is missing or unusable.
    Variables(Unready),
    /// A binding is not declared, or a secret is not set.
    Binding {
        /// Which.
        name: &'static str,
        /// What the platform said.
        source: worker::Error,
    },
    /// The edge refused its own limits.
    Edge(edge::Error),
    /// The sealing key did not read.
    Sealing(secret_worker::Error),
}

impl std::fmt::Display for Unstarted {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Variables(_) => out.write_str("the variables of the Worker are not usable"),
            Self::Binding { name, .. } => write!(out, "the binding {name} is not declared"),
            Self::Edge(_) => out.write_str("the edge's limits are not usable"),
            Self::Sealing(_) => out.write_str("the sealing key is not usable"),
        }
    }
}

impl std::error::Error for Unstarted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Variables(source) => Some(source),
            Self::Binding { source, .. } => Some(source),
            Self::Edge(source) => Some(source),
            Self::Sealing(source) => Some(source),
        }
    }
}

thread_local! {
    /// The product of this isolate, started by its first request.
    static STARTED: RefCell<Option<Rc<Product>>> = const { RefCell::new(None) };
}

impl Unstarted {
    /// What was wrong with its causes, as the answer of an isolate that could not start.
    pub(crate) fn respond(&self) -> worker::Result<Response> {
        let mut said = vec![self.to_string()];
        let mut next = std::error::Error::source(self);
        while let Some(cause) = next {
            said.push(cause.to_string());
            next = cause.source();
        }
        let text = said.join(": ");
        console_error!("the product did not start: {text}");
        Response::error(text, 500)
    }
}

/// The variables of a Worker's environment: a plain variable, else a secret.
struct Bindings<'a>(&'a Env);

impl Vars for Bindings<'_> {
    fn var(&self, name: &str) -> Option<String> {
        self.0.var(name).or_else(|_| self.0.secret(name)).ok().map(|value| value.to_string())
    }
}

/// An isolate's product.
pub(crate) struct Product {
    api: Calls<Answers>,
    host: String,
    serve: Serve<Sited<Ports, Platform>, Stored<R2>>,
}

impl Product {
    /// The binding `name`, or why it is not there.
    fn bound<T>(name: &'static str, found: worker::Result<T>) -> Result<T, Unstarted> {
        found.map_err(|source| Unstarted::Binding { name, source })
    }

    /// The product of this isolate: started by the first request, kept for the rest.
    pub(crate) fn of(env: &Env) -> Result<Rc<Self>, Unstarted> {
        if let Some(held) = STARTED.with(|started| started.borrow().clone()) {
            return Ok(held);
        }
        let made = Rc::new(Self::start(env)?);
        STARTED.with(|started| *started.borrow_mut() = Some(Rc::clone(&made)));
        Ok(made)
    }

    /// Starts the product on `env`.
    fn start(env: &Env) -> Result<Self, Unstarted> {
        let hosting = Hosting::read(&Bindings(env)).map_err(Unstarted::Variables)?;
        let db: Store = Arc::new(D1::new(Self::bound("DB", env.d1("DB"))?));
        let queue = Cf::new(Local::new(Arc::clone(&db)), Queues::new(Self::bound("WAKE", env.queue("WAKE"))?));
        let secret = Worker::from_env(env, Worker::KEY).map_err(Unstarted::Sealing)?;
        let bucket = || Self::bound("BUCKET", env.bucket("BUCKET"));
        let ports: Ports = Arc::new((Arc::clone(&db), R2::new(bucket()?), queue, secret));
        let origin: Origin = hosting.origin().clone();
        let nonces = || Ok::<_, Unstarted>(Remote::new(Self::bound("NONCES", env.durable_object("NONCES"))?));
        let (host, limits): (String, Budget) = (hosting.api().to_owned(), hosting.limits());
        let lookup = hosting.lookup();
        let wired = hosting.wire((Arc::clone(&ports), db), (nonces()?, nonces()?), (Platform, Platform));
        let sites = Sited::new(Control::new(ports, origin), Platform, &lookup);
        let edge = Edge::new(sites, Stored::new(R2::new(bucket()?)), Limits::default()).map_err(Unstarted::Edge)?;
        Ok(Self { api: Calls::new(wired, limits), host, serve: Serve::new(edge) })
    }

    /// Answers `request`: the API on its host, the serve path on every other.
    pub(crate) async fn respond(&self, mut request: Request) -> worker::Result<Response> {
        let url = request.url()?;
        let authority = url.port().map_or_else(
            || url.host_str().unwrap_or_default().to_owned(),
            |port| format!("{}:{port}", url.host_str().unwrap_or_default()),
        );
        if authority == self.host {
            self.api.respond(&mut request).await
        } else {
            Ok(self.serve.respond(&request).await?.response)
        }
    }
}
