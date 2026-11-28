//! A case that runs in workerd: build, measure, serve, call, report.

use std::time::Instant;

use super::build::Crate;
use super::case::Case;
use super::fault::Fault;
use super::loopback::{Body, Call, Loopback, Reply, Stream};
use super::npm::Packages;
use super::session::Session;
use super::shell::Shell;
use crate::report::{Line, Report};

/// One case, run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Served<'a> {
    pub(super) case: Case,
    pub(super) shell: &'a Shell,
}

/// A line the Worker or the run says, in the report.
struct Say<'a> {
    case: Case,
    report: &'a mut Report,
}

impl Say<'_> {
    fn line(&mut self, text: impl Into<String>) {
        self.report.line(Line::Worker { case: self.case, text: text.into() });
    }
}

impl Served<'_> {
    /// Builds the Worker, starts it, makes the calls of the case.
    pub(super) fn run(&self, report: &mut Report) -> Result<(), Fault> {
        Packages { shell: self.shell }.install()?;
        let krate = Crate { shell: self.shell, dir: self.case.dir() };
        let mut say = Say { case: self.case, report };
        let started = Instant::now();
        let size = krate.build()?;
        say.line(format!(
            "built in {} s: index_bg.wasm {} bytes raw, {} bytes gzip -9 ({:.2} MiB, {:.2} MiB)",
            started.elapsed().as_secs(),
            size.raw,
            size.gzip,
            Mebi::of(size.raw),
            Mebi::of(size.gzip)
        ));
        say.line(format!("wrangler deploy --dry-run: {}", krate.upload()?));
        let mut session = Session::start(&krate, self.case.port())?;
        let outcome = self.calls(&mut session, &mut say);
        if outcome.is_err() {
            let log = session.log();
            say.line(format!("workerd output, last lines:\n{log}"));
        }
        outcome
    }

    fn calls(&self, session: &mut Session, say: &mut Say<'_>) -> Result<(), Fault> {
        let address = session.address;
        match self.case {
            Case::Api => {
                self.timed(&address, &Call::get("/ping"), say, session)?;
                self.timed(&address, &Call::get("/ping"), say, session)?;
                self.timed(&address, &Call::get("/passkey"), say, session)?;
                Ok(())
            }
            Case::Store => self.store(session, say),
            Case::Db => self.cases(session, say),
            Case::Probe | Case::Lock => Ok(()),
        }
    }

    /// One call, its answer's lines said, a status other than 200 a fault, a `fail` line a fault.
    fn timed(&self, address: &Loopback, call: &Call, say: &mut Say<'_>, session: &mut Session) -> Result<Reply, Fault> {
        let started = Instant::now();
        let reply = address.send(call)?;
        let text = reply.text();
        say.line(format!("{} {} -> {} in {} ms", call.method, call.path, reply.status, started.elapsed().as_millis()));
        text.lines().for_each(|line| say.line(format!("  {line}")));
        if reply.status != 200 {
            return Err(Fault::Status { path: call.path.clone(), status: reply.status, body: text, log: session.log() });
        }
        match text.lines().find(|line| line.starts_with("fail ")) {
            Some(line) => Err(Fault::Refused { path: call.path.clone(), line: line.to_owned() }),
            None => Ok(reply),
        }
    }

    /// Spikes (b) and (c).
    fn store(&self, session: &mut Session, say: &mut Say<'_>) -> Result<(), Fault> {
        let address = session.address;
        self.timed(&address, &Call::get("/d1"), say, session)?;
        let probe = Stream { total: 1 << 20, piece: 64 << 10, seed: 1 };
        self.timed(&address, &Call::put("/r2/direct/chunked", Body::Chunked(probe)), say, session)?;
        self.timed(&address, &Call::put("/r2/direct/fixed", Body::Fixed(probe)), say, session)?;
        for (index, (total, piece)) in [(1 << 10, 1 << 10), (3 << 20, 64 << 10), (20 << 20, 64 << 10)].into_iter().enumerate() {
            let stream = Stream { total, piece, seed: 10 + index as u64 };
            let hash = stream.hash();
            self.timed(&address, &Call::put(format!("/r2/{hash}"), Body::Chunked(stream)), say, session)?;
            let back = self.timed(&address, &Call::get(format!("/r2/{hash}")), say, session)?;
            let again = blake3::hash(&back.body).to_hex().to_string();
            say.line(format!("  read back {} bytes, hash {}", back.body.len(), if again == hash { "equal" } else { "DIFFERENT" }));
            if again != hash {
                return Err(Fault::Refused { path: format!("/r2/{hash}"), line: "the object read back differs".to_owned() });
            }
        }
        let wrong = Stream { total: 12 << 20, piece: 64 << 10, seed: 99 };
        let claimed = Stream { seed: 98, ..wrong }.hash();
        let refused = address.send(&Call::put(format!("/r2/{claimed}"), Body::Chunked(wrong)))?;
        say.line(format!("wrong hash: PUT answered {}: {}", refused.status, refused.text().trim()));
        let absent = address.send(&Call::get(format!("/r2/{claimed}")))?;
        say.line(format!("wrong hash: GET of the claimed name answered {}", absent.status));
        if refused.status != 409 || absent.status != 404 {
            return Err(Fault::Refused {
                path: format!("/r2/{claimed}"),
                line: format!("a mismatch must answer 409 and store nothing, got {} and {}", refused.status, absent.status),
            });
        }
        Ok(())
    }

    /// The conformance cases of the Worker, one call each.
    fn cases(&self, session: &mut Session, say: &mut Say<'_>) -> Result<(), Fault> {
        let address = session.address;
        let listed = self.timed(&address, &Call::get("/__conformance"), say, session)?;
        let names: Vec<String> = listed.text().lines().filter(|line| !line.is_empty()).map(str::to_owned).collect();
        let mut failed = Vec::new();
        for name in &names {
            let call = Call::post(format!("/__conformance/{name}"));
            if let Err(fault) = self.timed(&address, &call, say, session) {
                say.line(format!("FAILED {name}"));
                failed.push(fault);
            }
        }
        say.line(format!("{} cases, {} failed", names.len(), failed.len()));
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
