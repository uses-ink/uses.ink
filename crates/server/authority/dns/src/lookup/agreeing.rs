//! Two resolvers that must give the same records: what a proof reads is trusted only when a
//! second path to DNS says the same, so one forged or stale answer on the way to one nameserver
//! does not make a proof. DNSSEC would be the other answer; it is not checked.

use clock::Draw;

use super::failed::Failed;
use super::system::System;
use crate::{Host, Resolver};

/// A resolver that asks two sets of nameservers and answers only when they agree.
#[derive(Clone, Debug)]
pub struct Agreeing<D> {
    first: System<D>,
    second: System<D>,
}

impl<D: Draw> Agreeing<D> {
    /// The resolver that answers what `first` and `second` both do.
    pub const fn new(first: System<D>, second: System<D>) -> Self {
        Self { first, second }
    }
}

impl<D: Draw> Resolver for Agreeing<D> {
    type Error = Failed;

    async fn txt(&self, host: &Host) -> Result<Vec<String>, Failed> {
        let (mut first, mut second) = (self.first.txt(host).await?, self.second.txt(host).await?);
        first.sort();
        second.sort();
        if first == second { Ok(first) } else { Err(Failed::Disagree { first: first.len(), second: second.len() }) }
    }
}
