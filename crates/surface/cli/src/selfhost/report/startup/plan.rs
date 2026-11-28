//! What a serve runs with, resolved from the flags alone: the same values `Product::open` builds
//! from, so a `--print-config` shows what a start would run without making the data folder, the
//! database or the first-run link.

use crate::command::auth::Resolved;
use crate::grammar::args;
use crate::selfhost::product::{Product, Unready};
use crate::selfhost::signin::Flags;

/// Every resolved setting the report names.
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    pub(crate) limits: edge_api::Limits,
    pub(crate) interval: watch::Interval,
    pub(crate) keys: Resolved,
    pub(crate) sign: control::signin::Settings,
    pub(crate) rates: edge_api::Rates,
    pub(crate) base: String,
    pub(crate) outbox: usize,
    pub(crate) secure: bool,
    pub(crate) cookie: String,
    pub(crate) relying: String,
    pub(crate) origins: Vec<String>,
    pub(crate) forwarded: edge_axum::Forwarded,
}

impl Plan {
    /// The settings a start with `serve` would run with, for the origin `site`. Nothing is made
    /// or written.
    pub(crate) fn planned(serve: &args::Serve, site: &str) -> Result<Self, Unready> {
        let flags = &serve.signin;
        let secure = Flags::secure(site)?;
        let relying = flags.relying(site, &serve.api_host)?;
        Ok(Self {
            limits: Product::limits(serve)?,
            interval: Product::interval(serve)?,
            keys: serve.auth.resolve(&serve.api_host, serve.address)?,
            sign: flags.settings(),
            rates: flags.rates(),
            base: flags.base(site, &serve.api_host)?.to_string(),
            outbox: flags.outbox.unwrap_or(Flags::OUTBOX),
            secure,
            cookie: flags.jar(secure)?.name().to_string(),
            relying: relying.id().to_owned(),
            origins: relying.origins().to_vec(),
            forwarded: flags.forwarded(),
        })
    }

    /// The settings a running `product` was made with.
    pub(crate) fn running(serve: &args::Serve, product: &Product) -> Result<Self, Unready> {
        let mailing = &product.mailing;
        Ok(Self {
            limits: product.limits,
            interval: product.interval,
            keys: product.keys.clone(),
            sign: mailing.settings,
            rates: mailing.rates,
            base: mailing.base.to_string(),
            outbox: mailing.outbox,
            secure: mailing.secure,
            cookie: serve.signin.jar(mailing.secure)?.name().to_string(),
            relying: mailing.relying.id().to_owned(),
            origins: mailing.relying.origins().to_vec(),
            forwarded: mailing.forwarded,
        })
    }
}
