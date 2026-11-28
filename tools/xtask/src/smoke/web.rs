//! The requests of a stage, made with `curl` as a stranger would, to names that `--resolve` points
//! at the loopback address, so the run does not depend on how a host resolves `*.localhost`.

use std::path::Path;
use std::process::Command;

use super::Fault;
use crate::process::Process;

/// One answer: the status and the body.
#[derive(Debug)]
pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) body: String,
}

/// One request to a host name of the server on `port`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Call<'a> {
    pub(super) host: &'a str,
    pub(super) port: u16,
    pub(super) path: &'a str,
    /// The cookie file the request reads and writes, when it has a session.
    pub(super) jar: Option<&'a Path>,
}

impl<'a> Call<'a> {
    /// A request for `path` of `host`.
    pub(super) const fn new(host: &'a str, port: u16, path: &'a str) -> Self {
        Self { host, port, path, jar: None }
    }

    /// With the cookies of `jar`.
    pub(super) const fn with(self, jar: &'a Path) -> Self {
        Self { jar: Some(jar), ..self }
    }

    /// A GET.
    pub(super) fn get(self) -> Result<Reply, Fault> {
        self.send(&[])
    }

    /// A form POST of `fields`, the way the dashboard's own page posts it: from its own origin.
    pub(super) fn post(self, fields: &[(&str, &str)]) -> Result<Reply, Fault> {
        let mut extra = vec!["-H".to_owned(), format!("Origin: http://{}:{}", self.host, self.port)];
        for (name, value) in fields {
            extra.extend(["--data-urlencode".to_owned(), format!("{name}={value}")]);
        }
        self.send(&extra)
    }

    fn send(self, extra: &[String]) -> Result<Reply, Fault> {
        let Self { host, port, path, jar } = self;
        let url = format!("http://{host}:{port}{path}");
        let mut command = Command::new("curl");
        command
            .args(["-sS", "--max-time", "10", "--resolve"])
            .arg(format!("{host}:{port}:127.0.0.1"))
            .args(["-w", "\n%{http_code}"]);
        if let Some(jar) = jar {
            command.arg("-b").arg(jar).arg("-c").arg(jar);
        }
        command.args(extra).arg(&url);
        let line = format!("curl {url}");
        let text = Process::from(command).text(&[]).map_err(|source| Fault::Command { line: line.clone(), source })?;
        let (body, status) = text.rsplit_once('\n').unwrap_or_default();
        let status = status.trim().parse().map_err(|_| Fault::expected("curl's status line", "a number", status))?;
        Ok(Reply { status, body: body.to_owned() })
    }
}
