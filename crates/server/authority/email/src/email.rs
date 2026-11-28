//! Sending a proof and confirming it.

use control::signin::{Proof, SignIn, Ttl};
use jiff::Timestamp;
use mail::{Address, LinkBase, Mailer, Message, Template};

use crate::Error;

/// The email authority: a mailer, the base of the links it sends, and how long a proof lives.
#[derive(Clone, Debug)]
pub struct Email<M> {
    mailer: M,
    base: LinkBase,
    life: Ttl,
}

impl<M: Mailer> Email<M> {
    /// How long a proof works, by default: 15 minutes, as a sign-in link.
    pub const LIFE: Ttl = Ttl::secs(15 * 60);

    /// An authority that mails through `mailer` links under `base`.
    pub const fn new(mailer: M, base: LinkBase) -> Self {
        Self { mailer, base, life: Self::LIFE }
    }

    /// The same authority with proofs that live for `life`.
    #[must_use]
    pub const fn with_life(mut self, life: Ttl) -> Self {
        self.life = life;
        self
    }

    /// How long a proof works.
    pub const fn life(&self) -> Ttl {
        self.life
    }

    /// Stores `proof` for `address` and mails its link. The caller draws the proof and says what
    /// time it is. If the mail fails the proof stays stored and ends on its own.
    pub async fn send<D: db::Db>(
        &self,
        rows: &SignIn<D>,
        address: &Address,
        (proof, now): (Proof, Timestamp),
    ) -> Result<Timestamp, Error<D::Error, M::Error>> {
        let issued = rows.vouch(address, proof, (self.life, now)).await?;
        let link = self.base.link(issued.proof.reveal());
        let template = Template::Prove { link, valid: self.life.duration() };
        self.mailer.send(&Message { to: address.clone(), template }).await.map_err(Error::Mail)?;
        Ok(issued.expires)
    }

    /// Spends `proof` and returns the address it was mailed to. It works once.
    pub async fn confirm<D: db::Db>(
        &self,
        rows: &SignIn<D>,
        proof: &Proof,
        now: Timestamp,
    ) -> Result<Address, Error<D::Error, M::Error>> {
        Ok(rows.attest(proof, now).await?)
    }
}
