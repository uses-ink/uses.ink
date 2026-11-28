//! A case that runs in workerd: build, measure, serve, call, report.

use std::time::Instant;

mod loopback;
mod session;

use self::loopback::{Body, Call, Reply, Stream};
use self::session::Session;
use super::case::Case;
use super::fault::Fault;
use super::node::{Crate, Packages, Shell};
use crate::report::{Line, Report};

/// One case, run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Served<'a> {
    pub(super) case: Case,
    pub(super) shell: &'a Shell,
}

/// The calls of a case: the running Worker and where its lines go.
struct Exchange<'a> {
    case: Case,
    session: &'a mut Session,
    report: &'a mut Report,
}

impl Exchange<'_> {
    /// Says a line.
    fn say(&mut self, text: impl Into<String>) {
        self.report.line(Line::Worker { case: self.case, text: text.into() });
    }

    /// Sends `call`, says its status and time, and returns the reply whatever it is.
    fn send(&mut self, call: &Call) -> Result<Reply, Fault> {
        let started = Instant::now();
        let reply = self.session.address.send(call)?;
        self.say(format!("{} {} -> {} in {} ms", call.method, call.path, reply.status, started.elapsed().as_millis()));
        Ok(reply)
    }

    /// Sends `call` and says the lines of its answer. A status other than 200 and a line that
    /// starts with `fail` are faults.
    fn call(&mut self, call: &Call) -> Result<Reply, Fault> {
        let reply = self.send(call)?;
        let text = reply.text();
        text.lines().for_each(|line| self.say(format!("  {line}")));
        if reply.status != 200 {
            return Err(Fault::Status {
                path: call.path.clone(),
                status: reply.status,
                body: text,
                log: self.session.log(),
            });
        }
        match text.lines().find(|line| line.starts_with("fail ")) {
            Some(line) => Err(Fault::Refused { path: call.path.clone(), line: line.to_owned() }),
            None => Ok(reply),
        }
    }
}

impl Served<'_> {
    /// Builds the Worker, starts it, makes the calls of the case.
    pub(super) fn run(&self, report: &mut Report) -> Result<(), Fault> {
        Packages { shell: self.shell }.install()?;
        let krate = Crate { shell: self.shell, dir: self.case.dir() };
        let started = Instant::now();
        let size = krate.build()?;
        report.line(Line::Worker {
            case: self.case,
            text: format!(
                "built in {} s: index_bg.wasm {} bytes raw, {} bytes gzip -9 ({:.2} MiB, {:.2} MiB)",
                started.elapsed().as_secs(),
                size.raw,
                size.gzip,
                Mebi::of(size.raw),
                Mebi::of(size.gzip)
            ),
        });
        report.line(Line::Worker { case: self.case, text: format!("wrangler deploy --dry-run: {}", krate.upload()?) });
        let mut session = Session::start(&krate, self.case.port())?;
        let mut exchange = Exchange { case: self.case, session: &mut session, report };
        let outcome = match self.case {
            Case::Api => Self::api(&mut exchange),
            Case::Store => Self::store(&mut exchange),
            Case::Db
            | Case::Blob
            | Case::Queue
            | Case::Secret
            | Case::Mail
            | Case::Nonce
            | Case::Watch
            | Case::Edge
            | Case::Rig => Self::cases(&mut exchange, self.case.suite()),
            Case::Probe | Case::Lock | Case::Migrate | Case::Migrations | Case::App => Ok(()),
        };
        if outcome.is_err() {
            let log = exchange.session.log();
            exchange.say(format!("workerd output, last lines:\n{log}"));
        }
        outcome
    }

    /// Spike (a): start, a sign-in verified, and a registration that is expected to stop the Worker.
    fn api(exchange: &mut Exchange<'_>) -> Result<(), Fault> {
        exchange.call(&Call::get("/ping"))?;
        exchange.call(&Call::get("/ping"))?;
        exchange.call(&Call::get("/passkey"))?;
        // the registration is the finding, not a failure of the run (decision 0096)
        let trapped = exchange.send(&Call::get("/passkey/register"))?;
        trapped.text().lines().for_each(|line| exchange.say(format!("  {line}")));
        let log = exchange.session.log();
        exchange.say(format!("workerd output after it:\n{log}"));
        Ok(())
    }

    /// Spikes (b) and (c).
    fn store(exchange: &mut Exchange<'_>) -> Result<(), Fault> {
        exchange.call(&Call::get("/d1"))?;
        let probe = Stream { total: 1 << 20, piece: 64 << 10, seed: 1 };
        exchange.call(&Call::put("/r2/direct/chunked", Body::Chunked(probe)))?;
        exchange.call(&Call::put("/r2/direct/fixed", Body::Fixed(probe)))?;
        for (index, (total, piece)) in
            [(1 << 10, 1 << 10), (3 << 20, 64 << 10), (20 << 20, 64 << 10)].into_iter().enumerate()
        {
            let stream = Stream { total, piece, seed: 10 + index as u64 };
            let hash = stream.hash();
            exchange.call(&Call::put(format!("/r2/{hash}"), Body::Chunked(stream)))?;
            let back = exchange.send(&Call::get(format!("/r2/{hash}")))?;
            let same = blake3::hash(&back.body).to_hex().to_string() == hash;
            exchange.say(format!(
                "  read back {} bytes, hash {}",
                back.body.len(),
                if same { "equal" } else { "DIFFERENT" }
            ));
            if !same || back.status != 200 {
                return Err(Fault::Refused {
                    path: format!("/r2/{hash}"),
                    line: "the object read back differs".to_owned(),
                });
            }
        }
        let wrong = Stream { total: 12 << 20, piece: 64 << 10, seed: 99 };
        let claimed = Stream { seed: 98, ..wrong }.hash();
        let refused = exchange.send(&Call::put(format!("/r2/{claimed}"), Body::Chunked(wrong)))?;
        exchange.say(format!("  {}", refused.text().trim()));
        let absent = exchange.send(&Call::get(format!("/r2/{claimed}")))?;
        if refused.status != 409 || absent.status != 404 {
            return Err(Fault::Refused {
                path: format!("/r2/{claimed}"),
                line: format!(
                    "a mismatch must answer 409 and store nothing, got {} and {}",
                    refused.status, absent.status
                ),
            });
        }
        Ok(())
    }

    /// The conformance cases of the Worker, one call each.
    fn cases(exchange: &mut Exchange<'_>, suite: Option<&str>) -> Result<(), Fault> {
        let listed = exchange.call(&Call::get("/__conformance"))?;
        let names: Vec<String> = listed
            .text()
            .lines()
            .filter(|line| suite.is_none_or(|suite| line.split('/').next() == Some(suite)))
            .map(str::to_owned)
            .collect();
        let mut failed = Vec::new();
        let mut absent = 0_usize;
        for name in &names {
            match exchange.call(&Call::post(format!("/__conformance/{name}"))) {
                // a case the adapter cannot offer is said so and is not a pass
                Ok(reply) if reply.text().starts_with("not offered") => absent += 1,
                Ok(_) => {}
                Err(fault) => {
                    exchange.say(format!("FAILED {name}"));
                    failed.push(fault);
                }
            }
        }
        exchange.say(format!("{} cases, {} failed, {absent} not offered", names.len(), failed.len()));
        match failed.into_iter().next() {
            Some(first) => Err(first),
            None => Ok(()),
        }
    }
}

/// Bytes as mebibytes.
struct Mebi;

impl Mebi {
    #[expect(clippy::cast_precision_loss, reason = "a size for people to read")]
    fn of(bytes: usize) -> f64 {
        bytes as f64 / 1_048_576.0
    }
}
