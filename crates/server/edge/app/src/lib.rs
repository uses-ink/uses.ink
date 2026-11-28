//! The product Worker (decision 0201): one `fetch` that answers the API on its own host and serves
//! sites on every other, over D1, R2, Queues, a secret binding and the nonce Durable Object.
//!
//! It is a Worker's alone: a binding and a request exist nowhere else, so a native build of this
//! crate is empty. The bindings and variables it reads are declared in `wrangler.toml` beside it,
//! and `README.md` says what a deploy needs.

#![cfg(target_arch = "wasm32")]

mod object;
mod product;

pub use object::NonceObject;
use product::Product;
use worker::{Context, Env, Request, Response, Result, event};

/// The Worker's `fetch`: the isolate's product answers, or says why it could not start.
#[event(fetch)]
pub async fn fetch(request: Request, env: Env, _ctx: Context) -> Result<Response> {
    match Product::of(&env) {
        Ok(product) => product.respond(request).await,
        Err(error) => error.respond(),
    }
}
