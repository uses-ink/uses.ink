//! The secret port's suite, on a Workers secret binding.

use secret::{Binding, Plain, Secret as _};
use secret_conformance::{Case, Pair};
use secret_worker::{Rotated, Worker};
use worker::{Env, Result};

/// The secret bindings of the rig.
pub(crate) struct Vault<'a> {
    pub(crate) env: &'a Env,
}

impl Vault<'_> {
    /// The cases of the adapter, beside the port's.
    const ROTATED: &'static str = "keeps_what_the_old_key_sealed_readable_for_a_rotation";
    const BOUND: &'static str = "reads_the_key_and_the_previous_one_from_bindings";
    const BAD: &'static str = "refuses_a_binding_that_is_not_a_key";

    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Case::ALL.iter().map(|case| case.name()).collect();
        names.extend([Self::ROTATED, Self::BOUND, Self::BAD]);
        names
    }

    /// The error of a failed set-up, as the rig's.
    fn rig(error: impl std::error::Error) -> worker::Error {
        worker::Error::RustError(error.to_string())
    }

    /// The old key opens through the binding named previous.
    async fn rotate(&self) -> Result<()> {
        let row = Binding::from("credential-1");
        let text = self.env.secret(Worker::PREVIOUS)?.to_string();
        let old = Worker::from_hex(&text, Some(Worker::PREVIOUS)).map_err(Self::rig)?;
        let sealed = old.seal(&Plain::from("token"), &row).await.map_err(|error| worker::Error::RustError(error.to_string()))?;
        let worker = Worker::from_env(self.env, Worker::KEY)
            .and_then(|worker| worker.rotating(self.env, Worker::PREVIOUS))
            .map_err(Self::rig)?;
        let opened = worker.open(&sealed, &row).await.map_err(|error| worker::Error::RustError(error.to_string()))?;
        assert_eq!(opened.expose(), b"token", "the previous key's seal did not open");
        Ok(())
    }

    /// Runs the case; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        if name == Self::ROTATED {
            Rotated::check().await;
        } else if name == Self::BOUND {
            self.rotate().await?;
        } else if name == Self::BAD {
            let error = Worker::from_env(self.env, "BAD_KEY").unwrap_err();
            assert_eq!(error.to_string(), "the secret binding `BAD_KEY` is not a key");
        } else if let Some(case) = Case::named(name) {
            let one = Worker::from_env(self.env, Worker::KEY).map_err(Self::rig)?;
            let other = Worker::ephemeral().map_err(Self::rig)?;
            case.run(Pair { one: &one, other: &other }).await;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
