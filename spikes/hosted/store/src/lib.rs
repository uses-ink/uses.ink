//! H0b spikes (b) and (c): what D1 carries through workers-rs 0.9's JavaScript boundary, and
//! what a streamed R2 put of unknown length takes. Each route answers lines of `ok`, `no`
//! (a finding: the platform does not do it), `note` or `fail` (the spike itself is wrong).

mod d1;
mod r2;

use worker::{Context, Env, Request, Response, Result, event};

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match segments.as_slice() {
        ["d1"] => Response::ok(d1::Values::new(env.d1("DB")?).run().await.text()),
        ["r2", "direct", name] => r2::Direct::new(env.bucket("BLOBS")?).put(req, name).await,
        ["r2", hash] => r2::Gateway::new(env.bucket("BLOBS")?).route(req, hash).await,
        _ => Response::error("not found", 404),
    }
}
