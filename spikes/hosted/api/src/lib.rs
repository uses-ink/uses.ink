//! H0b spike (a): the API crates, with `webauthn-rs-core` under them, as one Worker.
//!
//! `GET /ping` answers at once (the cold and warm start are timed by the caller). `GET /passkey`
//! runs a whole passkey ceremony through the server's real `Webauthn` adapter with a software
//! authenticator, then four refusals, and says `ok`, `no`, `note` or `fail` per line.

mod ceremony;

use worker::{Context, Env, Request, Response, Result, event};

#[event(fetch)]
async fn fetch(req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    match req.path().as_str() {
        "/ping" => Response::ok("ok ping pong\n"),
        "/passkey" => Response::ok(ceremony::Ceremony::run().text()),
        _ => Response::error("not found", 404),
    }
}
