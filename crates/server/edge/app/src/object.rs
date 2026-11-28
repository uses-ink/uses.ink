//! The Durable Object that remembers the nonces of one key.

#![allow(
    clippy::too_many_arguments,
    missing_docs,
    missing_debug_implementations,
    reason = "`durable_object` writes the glue, and the class is a handle the platform owns"
)]

use edge_worker::{Hosting, Serving, Unready, Vars};
use worker::{Date, DurableObject, Env, Request, Response, Result, State, durable_object};

/// The variables of the object's environment: a plain variable, else a secret.
struct Bindings<'a>(&'a Env);

impl Vars for Bindings<'_> {
    fn var(&self, name: &str) -> Option<String> {
        self.0.var(name).or_else(|_| self.0.secret(name)).ok().map(|value| value.to_string())
    }
}

/// The object of one key: a ledger started when the object is built, whose quiet period is the
/// nonce window (`AUTH_SKEW`, the number the API's signatures use), so what was signed before it was
/// built is refused (the boot rule). A window that does not read is said by every call.
#[durable_object]
pub struct NonceObject {
    serving: std::result::Result<Serving, Unready>,
}

impl DurableObject for NonceObject {
    fn new(_state: State, env: Env) -> Self {
        let serving =
            Hosting::skew(&Bindings(&env)).map(|window| Serving::new(Date::now().as_millis() / 1000, window.seconds()));
        Self { serving }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body = req.bytes().await?;
        let serving = match &self.serving {
            Ok(serving) => serving,
            Err(error) => {
                return Response::error(
                    format!("{error}: {:?}", std::error::Error::source(error).map(ToString::to_string)),
                    500,
                );
            }
        };
        match serving.handle(&body) {
            Ok(answer) => Response::from_bytes(answer),
            Err(error) => Response::error(error.to_string(), 400),
        }
    }
}
