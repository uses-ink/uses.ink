//! The conformance Worker for the row port: `GET /__conformance` lists the cases, `POST
//! /__conformance/<name>` empties the database and runs one case on the D1 adapter. A case that
//! passes answers `ok <name>`; one that fails panics, and the panic text is workerd's output.

use db_conformance::Case;
use db_d1::D1;
use worker::{Context, Env, Method, Request, Response, Result, event};

/// The tables the cases make, children first: what a case leaves behind is dropped before the next.
const TABLES: [&str; 8] = ["child", "parent", "every", "big", "alpha", "beta", "gamma", "d1_migrations"];

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (req.method(), segments.as_slice()) {
        (Method::Get, ["__conformance"]) => Response::ok(Case::ALL.iter().map(|case| case.name()).collect::<Vec<_>>().join("\n") + "\n"),
        (Method::Post, ["__conformance", name]) => match Case::named(name) {
            Some(case) => {
                let database = env.d1("DB")?;
                let drops: Vec<String> = TABLES.iter().map(|table| format!("DROP TABLE IF EXISTS {table}")).collect();
                database.exec(&drops.join("\n")).await?;
                case.run(&D1::new(database)).await;
                Response::ok(format!("ok {name}\n"))
            }
            None => Response::error(format!("no case {name}"), 404),
        },
        _ => Response::error("not found", 404),
    }
}
