//! Hosts that stop themselves when idle: how to wake one, what it costs, and what the farm
//! remembers of it while it sleeps.
//!
//! A host with `wake = "<command>"` that does not answer its probe is asleep, not down. Placement
//! counts the startup cost in its expected finish ([`Wake::cost`]); when it is chosen the farm
//! runs the command on this machine with the host name as the last argument, then waits up to
//! [`Wake::timeout`] seconds for the host to answer. A wake that fails is a typed error and the
//! host is left out of this job's placement.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::cache::Fault as Stored;
use crate::cli::Seconds;
use crate::cli::error::Error;
use crate::cli::name::HostName;
use crate::host::{Arch, Host, Load, Snapshot};

/// How to wake a host: the `wake`, `wake_timeout` and `wake_cost` keys of its `[[host]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wake {
    /// The command split on white space: no shell reads it. The host name is added as the last
    /// argument.
    words: Vec<String>,
    /// How long the command and the wait for the host together may take.
    pub(crate) timeout: Seconds,
    /// Seconds a start is expected to take, added to the host's expected finish.
    pub(crate) cost: Seconds,
}

impl Wake {
    pub(crate) const TIMEOUT: Seconds = Seconds(240);
    pub(crate) const COST: Seconds = Seconds(120);

    /// `None` when the command has no word.
    pub(crate) fn new(command: &str, timeout: Option<Seconds>, cost: Option<Seconds>) -> Option<Self> {
        let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        (!words.is_empty()).then(|| Self {
            words,
            timeout: timeout.unwrap_or(Self::TIMEOUT),
            cost: cost.unwrap_or(Self::COST),
        })
    }

    /// The command as shown, with the host name.
    pub(crate) fn shown(&self, host: &HostName) -> String {
        format!("{} {host}", self.words.join(" "))
    }

    /// Runs the command for `host` and waits for it to end, until `deadline`.
    pub(crate) fn run(&self, host: &HostName, deadline: Instant) -> Result<(), Fault> {
        let command = self.shown(host);
        let (program, rest) = self.words.split_first().ok_or(Fault::Blank)?;
        let mut child = Command::new(program)
            .args(rest)
            .arg(host.as_str())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| Fault::Spawn { command: command.clone(), source })?;
        let stderr = child.stderr.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut pipe, &mut bytes).ok();
                String::from_utf8_lossy(&bytes).into_owned()
            })
        });
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    child.kill().ok();
                    child.wait().ok();
                    return Err(Fault::Timeout { command, secs: self.timeout.0 });
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(source) => return Err(Fault::Wait { command, source }),
            }
        };
        let stderr = stderr.and_then(|reader| reader.join().ok()).unwrap_or_default();
        if status.success() { Ok(()) } else { Err(Fault::Exit { command, status, stderr }) }
    }
}

/// Why a wake failed.
#[derive(Debug)]
pub(crate) enum Fault {
    /// The command has no word.
    Blank,
    Spawn {
        command: String,
        source: std::io::Error,
    },
    Wait {
        command: String,
        source: std::io::Error,
    },
    /// The command ended without success.
    Exit {
        command: String,
        status: ExitStatus,
        stderr: String,
    },
    /// The command did not end in time.
    Timeout {
        command: String,
        secs: u64,
    },
    /// The command ended well and the host still did not answer in time.
    Silent {
        command: String,
        secs: u64,
        last: Box<Error>,
    },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => write!(out, "the wake command is empty"),
            Self::Spawn { command, .. } => write!(out, "`{command}` could not be started"),
            Self::Wait { command, .. } => write!(out, "waiting for `{command}` failed"),
            Self::Exit { command, status, stderr } => {
                write!(out, "`{command}` ended with {status}")?;
                match stderr.trim() {
                    "" => Ok(()),
                    said => write!(out, ": {said}"),
                }
            }
            Self::Timeout { command, secs } => write!(out, "`{command}` did not end within {secs} s"),
            Self::Silent { command, secs, .. } => {
                write!(out, "`{command}` ended, and the host still did not answer within {secs} s")
            }
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::Wait { source, .. } => Some(source),
            Self::Silent { last, .. } => Some(last.as_ref()),
            Self::Blank | Self::Exit { .. } | Self::Timeout { .. } => None,
        }
    }
}

/// What a host was when last up: placement needs it while the host sleeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Fact {
    host: String,
    arch: String,
    cores: u32,
    slots: u32,
}

impl Fact {
    /// The snapshot of the sleeping `host` as it was last seen, with a free slot.
    fn sleeping(&self, host: &Host, cost: Seconds) -> Snapshot {
        let arch: Arch = self.arch.parse().unwrap_or_else(|never: std::convert::Infallible| match never {});
        Snapshot::asleep(host, arch, (self.cores, self.slots), cost)
    }
}

impl Snapshot {
    /// A host that does not answer, as placement sees it: nothing running, `cost` to start.
    pub(crate) fn asleep(host: &Host, arch: Arch, shape: (u32, u32), cost: Seconds) -> Self {
        let (cores, slots) = shape;
        let arch = host.arch.clone().unwrap_or(arch);
        Self {
            name: host.name.clone(),
            golden: host.golden.unwrap_or(arch == Arch::REFERENCE),
            arch,
            cores,
            load: Load(0),
            slots: host.slots.unwrap_or(slots),
            busy: 0,
            queued: 0,
            now: 0,
            jobs: Vec::new(),
            holders: Vec::new(),
            asleep: Some(cost.0),
        }
    }
}

/// The hosts with a wake command that were up once: `seen.json` of the cache directory.
#[derive(Debug, Clone)]
pub(crate) struct Seen {
    file: PathBuf,
}

impl Seen {
    pub(crate) fn at(dir: &Path) -> Self {
        Self { file: dir.join("seen.json") }
    }

    fn read(&self) -> Result<Vec<Fact>, Stored> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => serde_json::from_str(&text).map_err(|source| Stored::Json { path: self.file.clone(), source }),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(source) => Err(Stored::Io { path: self.file.clone(), source }),
        }
    }

    /// The snapshot of `host` as last seen; none if it never was.
    pub(crate) fn recall(&self, host: &Host, cost: Seconds) -> Result<Option<Snapshot>, Stored> {
        let facts = self.read()?;
        Ok(facts.iter().find(|fact| fact.host == host.name.as_str()).map(|fact| fact.sleeping(host, cost)))
    }

    /// Remembers what `snap` is, when that changed.
    pub(crate) fn note(&self, snap: &Snapshot) -> Result<(), Stored> {
        let fact =
            Fact { host: snap.name.to_string(), arch: snap.arch.to_string(), cores: snap.cores, slots: snap.slots };
        let mut facts = self.read()?;
        if facts.contains(&fact) {
            return Ok(());
        }
        facts.retain(|known| known.host != fact.host);
        facts.push(fact);
        let io = |source| Stored::Io { path: self.file.clone(), source };
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let text = serde_json::to_string(&facts).map_err(|source| Stored::Json { path: self.file.clone(), source })?;
        std::fs::write(&self.file, text).map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("farm-wake-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        /// An executable script that appends its arguments to `marker` and exits with `code`.
        fn script(&self, body: &str) -> String {
            use std::os::unix::fs::PermissionsExt as _;
            let path = self.0.join("wake.sh");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.display().to_string()
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn soon() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    #[test]
    fn the_command_runs_with_the_host_name_as_its_last_argument() {
        let dir = Dir::new("run");
        let marker = dir.0.join("marker");
        let script = dir.script(&format!("echo \"$@\" > {}", marker.display()));
        let wake = Wake::new(&format!("{script} --fast"), None, None).unwrap();
        let host: HostName = "farm-a".parse().unwrap();
        wake.run(&host, soon()).unwrap();
        assert_eq!(std::fs::read_to_string(marker).unwrap().trim(), "--fast farm-a");
        assert_eq!((wake.timeout, wake.cost), (Seconds(240), Seconds(120)));
        assert_eq!(wake.shown(&host), format!("{script} --fast farm-a"));
    }

    #[test]
    fn a_failing_command_is_a_typed_error_naming_it_and_its_status() {
        let dir = Dir::new("exit");
        let script = dir.script("echo no capacity >&2; exit 3");
        let wake = Wake::new(&script, None, None).unwrap();
        let error = wake.run(&"farm-a".parse().unwrap(), soon()).unwrap_err();
        let text = error.to_string();
        assert!(matches!(error, Fault::Exit { .. }), "{text}");
        assert!(text.contains(&script) && text.contains("exit status: 3") && text.contains("no capacity"), "{text}");
    }

    #[test]
    fn a_command_that_hangs_is_killed_at_the_deadline_not_waited_for() {
        let dir = Dir::new("hang");
        let script = dir.script("sleep 30");
        let wake = Wake::new(&script, Some(Seconds(1)), None).unwrap();
        let started = Instant::now();
        let error = wake.run(&"farm-a".parse().unwrap(), Instant::now() + Duration::from_millis(300)).unwrap_err();
        assert!(matches!(error, Fault::Timeout { .. }), "{error}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_missing_command_is_a_spawn_error_and_a_blank_one_is_no_wake() {
        let wake = Wake::new("/nonexistent/aws-wake", None, None).unwrap();
        let error = wake.run(&"farm-a".parse().unwrap(), soon()).unwrap_err();
        assert!(matches!(error, Fault::Spawn { .. }), "{error}");
        assert_eq!(Wake::new("  ", None, None), None);
    }

    #[test]
    fn what_was_seen_comes_back_as_a_sleeping_snapshot_and_an_unseen_host_is_none() {
        let dir = Dir::new("seen");
        let seen = Seen::at(&dir.0);
        let host = Host { name: "farm-a".parse().unwrap(), ..Host::local() };
        assert_eq!(seen.recall(&host, Seconds(120)).unwrap(), None);
        let mut up = Snapshot::asleep(&host, Arch::X86, (16, 2), Seconds(0));
        up.asleep = None;
        seen.note(&up).unwrap();
        seen.note(&up).unwrap();
        let back = seen.recall(&host, Seconds(90)).unwrap().unwrap();
        assert_eq!((back.cores, back.slots, back.asleep, back.golden), (16, 2, Some(90), true));
        assert_eq!(seen.read().unwrap().len(), 1);
    }
}
