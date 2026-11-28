//! The conformance harness of the serve path, over any ports of the control plane: the native
//! tests give it SQLite and a directory, the Worker rig gives it D1 and R2.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::time::Duration;

use blob::{Blob as Store, Tenant};
use build::Source;
use clock::Clock;
use control::{
    AbuseKind, AccountKind, Control, DomainName, Handle, Id, NewAccount, NewClaim, NewDeploy, NewSite, Origin, Ports,
    Proving, SiteKind, Suspension, Token,
};
use edge::{Edge, Limits};
use edge_conformance::{Harness, Offers, Seed, Unseeded};
use edge_control::Settings;
use jiff::Timestamp;
use model::{Hash, Manifest};

use crate::{Sited, Stored};

/// A control plane served through [`Sited`] and [`Stored`], and the means to seed it. A seeded site
/// has the handle of the first label of its host, so the origin's host is the rest.
#[derive(Debug)]
pub struct Hosted<P: Ports, K, B> {
    ports: P,
    origin: Origin,
    clock: K,
    stored: Stored<B>,
    sites: Sited<P, K>,
    made: Cell<u8>,
    by_host: RefCell<BTreeMap<String, Id<SiteKind>>>,
    account: Cell<bool>,
}

impl<P: Ports + Clone + 'static, K: Clock + Clone + 'static, B: Store + 'static> Hosted<P, K, B> {
    /// Lookups are never remembered, so a case sees what it has just done.
    const SETTINGS: Settings = Settings { ttl: Duration::ZERO, deploys: 64, waiting: 1 };

    /// The time every row of a seed is made at.
    fn start() -> Timestamp {
        Timestamp::from_second(1_800_000_000).unwrap_or(Timestamp::UNIX_EPOCH)
    }

    /// The harness over `ports`, whose blobs `blobs` reads (another handle on the same store), at
    /// the hosts below `origin`.
    pub fn new(ports: P, blobs: B, origin: Origin, clock: K) -> Self {
        let sites = Sited::new(Control::new(ports.clone(), origin.clone()), clock.clone(), &Self::SETTINGS);
        Self {
            ports,
            origin,
            clock,
            stored: Stored::new(blobs),
            sites,
            made: Cell::new(1),
            by_host: RefCell::new(BTreeMap::new()),
            account: Cell::new(false),
        }
    }

    fn control(&self) -> Control<P> {
        Control::new(self.ports.clone(), self.origin.clone())
    }

    fn site(&self, host: &str) -> Result<Id<SiteKind>, Unseeded> {
        self.by_host.borrow().get(host).copied().ok_or_else(|| Unseeded::unknown(host))
    }
}

impl<P: Ports + Clone + 'static, K: Clock + Clone + 'static, B: Store + 'static> Harness for Hosted<P, K, B> {
    type Scope = Tenant;
    type Sites = Sited<P, K>;
    type Blobs = Stored<B>;

    fn sites(&self) -> &Self::Sites {
        &self.sites
    }

    fn blobs(&self) -> &Self::Blobs {
        &self.stored
    }

    fn edge(&self) -> Result<Edge<Self::Sites, Self::Blobs>, edge::Error> {
        let sites = Sited::new(self.control(), self.clock.clone(), &Self::SETTINGS);
        Edge::new(sites, self.stored.clone(), Limits::default())
    }

    fn offers(&self) -> Offers {
        Offers { suspensions: true, domains: true }
    }

    async fn publish(&self, seed: &Seed) -> Result<Manifest, Unseeded> {
        let control = self.control();
        let account: Id<AccountKind> = Id::from_bytes([1; 16]);
        if !self.account.replace(true) {
            control
                .add_account(NewAccount { id: account, at: Self::start() })
                .await
                .map_err(Unseeded::because("add the account"))?;
        }
        self.made.set(self.made.get().saturating_add(1));
        let id: Id<SiteKind> = Id::from_bytes([self.made.get(); 16]);
        let label = seed.host.split('.').next().unwrap_or_default();
        let handle = label.parse::<Handle>().map_err(Unseeded::because("name the handle"))?;
        let source = Source::Folder("/srv/site".into());
        let new = NewSite { id, account, handle, source, at: Self::start() };
        control.add_site(new).await.map_err(Unseeded::because("add the site"))?;
        let tenant = Tenant::try_from(id).map_err(Unseeded::because("name the tenant"))?;
        for page in &seed.pages {
            let blob = self.ports.blob();
            blob.put(&tenant, Hash::of(&page.bytes), [page.bytes.as_slice()])
                .await
                .map_err(Unseeded::because("store a blob"))?;
        }
        let manifest = seed.manifest();
        let deploy = NewDeploy { site: id, manifest: manifest.clone(), build: None, at: Self::start() };
        control.deploy(deploy).await.map_err(Unseeded::because("record the deploy"))?;
        self.by_host.borrow_mut().insert(seed.host.clone(), id);
        Ok(manifest)
    }

    async fn suspend(&self, host: &str) -> Result<(), Unseeded> {
        let suspension = Suspension {
            site: self.site(host)?,
            kind: AbuseKind::Malware,
            by: "operator".to_owned(),
            report: None,
            at: Self::start(),
        };
        self.control().suspend(suspension).await.map(drop).map_err(Unseeded::because("suspend the site"))
    }

    async fn alias(&self, host: &str, custom: &str) -> Result<(), Unseeded> {
        let site = self.site(host)?;
        let name: DomainName = custom.parse().map_err(Unseeded::because("name the domain"))?;
        let token = Token::from_bytes([7; 16]);
        let claim = NewClaim { site, name: name.clone(), token, at: Self::start() };
        self.control().claim_domain(claim).await.map_err(Unseeded::because("claim the domain"))?;
        let found = control::Found::new(1, vec![token]);
        let proving = Proving { site, name, found, at: Self::start() };
        self.control().prove_domain(proving).await.map(drop).map_err(Unseeded::because("prove the domain"))
    }
}
