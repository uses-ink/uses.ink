//! `release image`: the self-host image built, started as the guide says and probed. Nothing is
//! pushed. The container runs read-only with every capability dropped and `/data` a fresh volume,
//! and must answer the healthcheck's own request.

use std::fmt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{Fault, Release};
use crate::check::Step;
use crate::error::Error;
use crate::process::Process;
use crate::report::{Line, Report};

/// The image step.
#[derive(Debug, Clone, Copy)]
pub(super) struct Image;

/// The program that builds and runs containers on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Runtime {
    Docker,
    Podman,
}

/// One run: the runtime, the workspace, and the names it makes, so a rerun trips on nothing a
/// failed run left.
#[derive(Debug)]
struct Probe<'a> {
    runtime: Runtime,
    root: &'a Path,
    image: String,
    container: String,
    mount: String,
}

impl Runtime {
    /// In the order tried.
    const ALL: [Self; 2] = [Self::Docker, Self::Podman];

    const fn program(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }

    /// The first that answers `--version`.
    fn found() -> Option<Self> {
        Self::ALL.into_iter().find(|runtime| Process::from(runtime.command(&["--version"])).text(&[]).is_ok())
    }

    fn command(self, args: &[&str]) -> Command {
        let mut command = Command::new(self.program());
        command.args(args);
        command
    }

    /// The words of `build`. Podman writes OCI images, which drop the healthcheck, unless asked.
    const fn format(self) -> &'static [&'static str] {
        match self {
            Self::Docker => &[],
            Self::Podman => &["--format", "docker"],
        }
    }
}

impl<'a> Probe<'a> {
    /// What the image must declare as its user.
    const USER: &'static str = "65532:65532";

    /// How many one-second tries the container gets to answer.
    const TRIES: u64 = 30;

    fn new(runtime: Runtime, root: &'a Path) -> Self {
        let id = std::process::id();
        Self {
            runtime,
            root,
            image: format!("uses-ink-check:{id}"),
            container: format!("uses-ink-check-{id}"),
            mount: format!("uses-ink-check-{id}:/data"),
        }
    }

    /// Runs `args`, its stdout as text.
    fn text(&self, args: &[&str]) -> Result<String, Fault> {
        let line = format!("{} {}", self.runtime.program(), args.join(" "));
        Process::from(self.runtime.command(args)).text(&[]).map_err(|source| Fault::Command { line, source })
    }

    /// Whether `args` ended with success.
    fn passes(&self, args: &[&str]) -> bool {
        Process::from(self.runtime.command(args)).stdout(&[]).is_ok()
    }

    /// The image built from the `Dockerfile` of the root, its output shown.
    fn build(&self) -> Result<(), Fault> {
        let mut args = vec!["build"];
        args.extend(self.runtime.format());
        args.extend(["-t", &self.image, "."]);
        let line = format!("{} {}", self.runtime.program(), args.join(" "));
        let mut command = self.runtime.command(&args);
        command.current_dir(self.root);
        Process::from(command).status().map_err(|source| Fault::Command { line, source })
    }

    /// The image runs as the user it says and declares a healthcheck.
    fn declared(&self) -> Result<(), Fault> {
        let user = self.text(&["image", "inspect", "--format", "{{.Config.User}}", &self.image])?;
        if user.trim() != Self::USER {
            return Err(Fault::Config { field: "user", found: user.trim().to_owned(), wanted: Self::USER });
        }
        let check = self.text(&["image", "inspect", "--format", "{{json .Config.Healthcheck}}", &self.image])?;
        if !check.contains("/healthz") {
            return Err(Fault::Config {
                field: "healthcheck",
                found: check.trim().to_owned(),
                wanted: "a request for /healthz",
            });
        }
        Ok(())
    }

    /// The container started the way the guide starts it: read-only, no capability, a volume.
    fn start(&self) -> Result<(), Fault> {
        let mut args = vec!["run", "-d", "--name", &self.container, "--read-only", "--tmpfs", "/tmp"];
        args.extend(["--cap-drop", "ALL", "--security-opt", "no-new-privileges", "-v", &self.mount, &self.image]);
        args.extend(["serve", "--addr", "0.0.0.0:8080", "--data", "/data/state"]);
        self.text(&args).map(drop)
    }

    /// What the container said: a runtime's `logs` writes the container's standard error to its own.
    fn logs(&self) -> String {
        match self.runtime.command(&["logs", &self.container]).output() {
            Ok(done) => format!("{}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr)),
            Err(error) => format!("(the logs could not be read: {error})"),
        }
    }

    /// Asks the way the image's healthcheck does, until it answers or the tries are spent.
    fn answered(&self) -> Result<u64, Fault> {
        let ask = ["exec", &self.container, "wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8080/healthz"];
        for second in 0..Self::TRIES {
            if self.passes(&ask) {
                return Ok(second);
            }
            let state = self.text(&["inspect", "--format", "{{.State.Running}}", &self.container])?;
            if state.trim() != "true" {
                return Err(Fault::Exited { logs: self.logs() });
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        Err(Fault::Unready { seconds: Self::TRIES, logs: self.logs() })
    }

    /// Removes what the run made; a thing already gone is no failure.
    fn clean(&self) {
        self.passes(&["rm", "-f", &self.container]);
        self.passes(&["volume", "rm", "-f", &self.mount.replace(":/data", "")]);
        self.passes(&["rmi", "-f", &self.image]);
    }

    fn run(&self) -> Result<String, Fault> {
        self.build()?;
        self.declared()?;
        self.start()?;
        let seconds = self.answered()?;
        Ok(format!(
            "{} built {}, which runs as {} read-only and answered /healthz after {seconds} s",
            self.runtime.program(),
            self.image,
            Self::USER
        ))
    }
}

impl Step for Image {
    fn run(&self, root: &Path, report: &mut Report) {
        let Some(runtime) = Runtime::found() else {
            return report.fail(Error::Released { step: Release::Image, fault: Fault::NoRuntime });
        };
        let probe = Probe::new(runtime, root);
        let done = probe.run();
        probe.clean();
        match done {
            Ok(text) => report.line(Line::Released { step: Release::Image, text }),
            Err(fault) => report.fail(Error::Released { step: Release::Image, fault }),
        }
    }
}

impl fmt::Display for Image {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the image built, started read-only and probed")
    }
}
