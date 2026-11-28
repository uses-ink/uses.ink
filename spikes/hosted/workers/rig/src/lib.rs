//! The conformance Worker of the hosted adapters: `GET /__conformance` lists `<suite>/<case>`, and
//! `POST /__conformance/<suite>/<case>` empties what the suite uses and runs one case on the
//! adapter. A case that passes answers `ok <suite>/<case>`; one that fails panics, and the panic
//! text is workerd's output.

mod blobs;
mod jobs;
mod rows;
mod suite;
mod vault;

use suite::Suite;
use worker::{Context, Env, Method, Request, Response, Result, event};

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (req.method(), segments.as_slice()) {
        (Method::Get, ["__conformance"]) => Response::ok(Suite::listing()),
        (Method::Post, ["__conformance", suite, case]) => match Suite::named(suite) {
            Some(found) => found.run(&env, case).await,
            None => Response::error(format!("no suite {suite}"), 404),
        },
        _ => Response::error("not found", 404),
    }
}
