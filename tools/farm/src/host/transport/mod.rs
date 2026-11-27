//! How a call reaches a host. [`Transport`] is the one seam: [`Local`] is this machine through
//! `sh`, [`Ssh`] is another through the system `ssh` and `rsync`, and the logic above them is
//! tested with a fake. Every process is started from an argument vector; the one shell line,
//! the remote command of `ssh`, is made of [`Quoted`](crate::job::quote::Quoted) words.

pub(crate) mod process;
pub(crate) mod program;

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::console::Console;
use crate::cli::name::Destination;
use crate::host::transport::process::Failure;
use crate::host::transport::program::{Call, Stdin};
use crate::job::quote::Line;

/// What a call left: how it ended and what it said on stderr.
#[derive(Debug)]
pub(crate) struct Done {
    pub(crate) status: ExitStatus,
    pub(crate) stderr: String,
}

impl Done {
    /// The call's result, or why it failed.
    pub(crate) fn ok(self) -> Result<Self, Failure> {
        if self.status.success() {
            Ok(self)
        } else {
            Err(Failure::Exit { status: self.status, stderr: self.stderr.into_bytes() })
        }
    }
}

/// A workspace to ship to a host.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Push<'a> {
    /// The workspace here.
    pub(crate) root: &'a Path,
    /// Its directory on the host, under the host's `dir`.
    pub(crate) tree: &'a str,
    /// The lock that keeps two syncs of it from overlapping.
    pub(crate) lock: &'a str,
    /// Names the repo keeps out of the sync, beside the built-in ones.
    pub(crate) exclude: &'a [crate::cli::name::Excluded],
}

/// Files to bring back from a host's tree into the workspace.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pull<'a> {
    pub(crate) root: &'a Path,
    pub(crate) tree: &'a str,
    /// Paths relative to both, one per line.
    pub(crate) files: &'a str,
}

/// A way to reach a host.
pub(crate) trait Transport {
    /// Runs a call, handing each line it prints to `lines`.
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure>;

    /// Makes the host's tree equal to the workspace. Nothing for a host that runs in place.
    fn push(&self, push: &Push<'_>) -> Result<(), Failure>;

    /// Copies the listed files from the host's tree into the workspace.
    fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure>;

    /// Whether the host runs in the workspace itself, with no copy.
    fn is_local(&self) -> bool;
}

/// What goes to a process's standard input.
#[derive(Debug, Clone, Copy)]
enum Feed<'a> {
    Nothing,
    /// Open until the call ends or the time is up, when `detach` is written and it is closed.
    Open(Duration),
    Text(&'a str),
}

impl From<Stdin> for Feed<'_> {
    fn from(stdin: Stdin) -> Self {
        match stdin {
            Stdin::Null => Self::Nothing,
            Stdin::Held(until) => Self::Open(until),
        }
    }
}

/// Starts processes, saying each command first when asked to.
#[derive(Debug, Clone)]
pub(crate) struct Exec {
    console: Arc<Console>,
}

/// How long a process gets to end after `detach`, before it is killed.
const GRACE: Duration = Duration::from_secs(10);

/// How long a wait with no deadline sleeps before it asks again: a call with no limit still
/// wakes once an hour, so a dead one is noticed.
const UNBOUNDED_WAIT: Duration = Duration::from_secs(3600);

/// How long `ssh` keeps a shared connection open after its last call, in seconds.
const CONTROL_PERSIST_SECS: u32 = 120;

/// `ssh`'s connect timeout, in seconds: a host that does not answer in this time is down.
const CONNECT_SECS: u32 = 8;

/// How often `ssh` sends a keep-alive on a quiet connection, in seconds.
const ALIVE_SECS: u32 = 15;

/// How many keep-alives may go unanswered before `ssh` gives the connection up.
const ALIVE_MISSES: u32 = 3;

impl Exec {
    pub(crate) fn new(console: Arc<Console>) -> Self {
        Self { console }
    }

    /// Runs `argv`, each line of its stdout into `lines`.
    fn spawn(&self, argv: &[String], feed: Feed<'_>, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        self.console.show(Line(argv));
        let (first, rest) =
            argv.split_first().ok_or_else(|| Failure::Spawn(std::io::ErrorKind::InvalidInput.into()))?;
        let mut command = Command::new(first);
        command.args(rest).stdout(Stdio::piped()).stderr(Stdio::piped());
        command.stdin(match feed {
            Feed::Nothing => Stdio::null(),
            Feed::Open(_) | Feed::Text(_) => Stdio::piped(),
        });
        let mut child = command.spawn().map_err(Failure::Spawn)?;
        if let Feed::Text(text) = feed {
            child.stdin.take().map_or(Ok(()), |mut stdin| stdin.write_all(text.as_bytes())).map_err(Failure::Stdin)?;
        }
        Follow::new(&mut child, feed).run(lines)
    }
}

/// A running child being read until it ends.
struct Follow<'a> {
    child: &'a mut Child,
    deadline: Option<Instant>,
    detaching: Option<Instant>,
    /// The first thing that went wrong while following; it ends the call once the child is reaped.
    fault: Option<Failure>,
}

impl<'a> Follow<'a> {
    fn new(child: &'a mut Child, feed: Feed<'_>) -> Self {
        let deadline = match feed {
            Feed::Open(until) => Some(Instant::now() + until),
            Feed::Nothing | Feed::Text(_) => None,
        };
        Self { child, deadline, detaching: None, fault: None }
    }

    fn run(mut self, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        let (receiver, reading) = Self::lines(self.child.stdout.take());
        let errors = Self::errors(self.child.stderr.take());
        let mut writer = self.child.stdin.take();
        loop {
            match receiver.recv_timeout(self.next_wait()) {
                Ok(line) => lines(&line),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            // checked after every line too: a job that never goes quiet still has its time
            if self.due()
                && let Err(failure) = self.expire(&mut writer)
            {
                self.fail(failure);
            }
        }
        let read = reading.join().map_err(|_| Failure::Panicked { thread: "output" });
        let status = self.child.wait().map_err(Failure::Wait);
        let stderr = errors.join().map_err(|_| Failure::Panicked { thread: "errors" });
        if let Some(failure) = self.fault.take() {
            return Err(failure);
        }
        read?.map_err(Failure::Wait)?;
        Ok(Done { status: status?, stderr: stderr?.map_err(Failure::Wait)? })
    }

    /// Whether the time of the next step has come.
    fn due(&self) -> bool {
        self.detaching.or(self.deadline).is_some_and(|at| Instant::now() >= at)
    }

    /// The next step: say `detach` and close the input, or, when the grace is over, kill.
    fn expire(&mut self, writer: &mut Option<ChildStdin>) -> Result<(), Failure> {
        if let Some(mut input) = writer.take() {
            // the job outlives us from here: the host side sees `detach`, then the end of input
            if let Err(source) = input.write_all(b"detach\n") {
                // a child that has already ended has no one to tell
                if self.child.try_wait().map_err(Failure::Wait)?.is_none() {
                    return Err(Failure::Stdin(source));
                }
            }
            self.detaching = Some(Instant::now() + GRACE);
        } else if self.detaching.is_some() {
            self.stop();
            self.child.kill().map_err(Failure::Kill)?;
        }
        Ok(())
    }

    /// Ends the call with `failure`, unless one came first: the child is killed.
    fn fail(&mut self, failure: Failure) {
        self.stop();
        let killed = self.child.kill().map_err(Failure::Kill);
        self.fault.get_or_insert(failure);
        if let Err(failure) = killed {
            self.fault.get_or_insert(failure);
        }
    }

    /// No more steps are due.
    fn stop(&mut self) {
        self.deadline = None;
        self.detaching = None;
    }

    /// A thread that reads `stdout` line by line into the channel.
    fn lines(stdout: Option<ChildStdout>) -> (mpsc::Receiver<String>, thread::JoinHandle<std::io::Result<()>>) {
        let (sender, receiver) = mpsc::channel();
        let reading = thread::spawn(move || {
            let mut reader = BufReader::new(Self::source(stdout));
            let mut raw = Vec::new();
            while reader.read_until(b'\n', &mut raw)? > 0 {
                let line = String::from_utf8_lossy(&raw).trim_end_matches(['\n', '\r']).to_owned();
                raw.clear();
                if sender.send(line).is_err() {
                    break;
                }
            }
            Ok(())
        });
        (receiver, reading)
    }

    /// A thread that collects what `stderr` says, the first megabyte of it.
    fn errors(stderr: Option<ChildStderr>) -> thread::JoinHandle<std::io::Result<String>> {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            Self::source(stderr).take(1 << 20).read_to_end(&mut bytes)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
    }

    fn source(pipe: Option<impl std::io::Read + Send + 'static>) -> Box<dyn std::io::Read + Send> {
        match pipe {
            Some(pipe) => Box::new(pipe),
            None => Box::new(std::io::empty()),
        }
    }

    /// How long to wait for the next line: until the deadline, or the end of the grace after it.
    fn next_wait(&self) -> Duration {
        let limit = self.detaching.or(self.deadline);
        limit.map_or(UNBOUNDED_WAIT, |at| at.saturating_duration_since(Instant::now()).max(Duration::from_millis(10)))
    }
}

/// This machine: a call is `sh -c <program>`, a workspace is where it is.
#[derive(Debug, Clone)]
pub(crate) struct Local {
    exec: Exec,
}

impl Local {
    pub(crate) fn new(exec: Exec) -> Self {
        Self { exec }
    }
}

impl Transport for Local {
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        self.exec.spawn(&call.argv(), call.stdin.into(), lines)
    }

    fn push(&self, _: &Push<'_>) -> Result<(), Failure> {
        Ok(())
    }

    fn pull(&self, _: &Pull<'_>) -> Result<(), Failure> {
        Ok(())
    }

    fn is_local(&self) -> bool {
        true
    }
}

/// Another machine, through the user's `ssh` and `rsync`, with their config and agent.
#[derive(Debug, Clone)]
pub(crate) struct Ssh {
    exec: Exec,
    destination: Destination,
    control: Option<PathBuf>,
}

impl Ssh {
    /// How files are placed: compared by content, and written with the time of the write, not the
    /// sender's. Cargo calls a build fresh when its artifacts are newer than the sources. A tree
    /// seeded from a sibling holds artifacts newer than a rebased source whose own time is older,
    /// and `-a` alone would stamp the changed file with that old time. With these two flags a file
    /// whose bytes changed is newer than every artifact, and one that did not keeps its time.
    const PLACE: [&'static str; 2] = ["--checksum", "--no-times"];

    pub(crate) fn new(exec: Exec, destination: Destination) -> Self {
        // One connection per host is shared by every call, so a poll is a message, not a handshake.
        let control = Some(std::env::temp_dir().join("uses-ink-farm-%C")).filter(|path| {
            path.to_str().is_some_and(|text| !text.contains(char::is_whitespace) && !text.contains('\''))
        });
        Self { exec, destination, control }
    }

    /// `ssh`'s options, which `rsync -e` repeats.
    fn options(&self) -> Vec<String> {
        let mut words: Vec<String> = [
            "-o".to_owned(),
            "BatchMode=yes".to_owned(),
            "-o".to_owned(),
            format!("ConnectTimeout={CONNECT_SECS}"),
            "-o".to_owned(),
            format!("ServerAliveInterval={ALIVE_SECS}"),
            "-o".to_owned(),
            format!("ServerAliveCountMax={ALIVE_MISSES}"),
            "-o".to_owned(),
            "LogLevel=ERROR".to_owned(),
        ]
        .to_vec();
        if let Some(path) = self.control.as_ref().and_then(|path| path.to_str()) {
            words.extend([
                "-o".to_owned(),
                "ControlMaster=auto".to_owned(),
                "-o".to_owned(),
                format!("ControlPersist={CONTROL_PERSIST_SECS}"),
                "-o".to_owned(),
            ]);
            words.push(format!("ControlPath={path}"));
        }
        words
    }

    /// The argument vector that runs `call` over ssh: the remote command is one word, `sh -c
    /// <program> sh <args>`, every part quoted.
    pub(crate) fn argv(&self, call: &Call) -> Vec<String> {
        let mut argv = vec!["ssh".to_owned()];
        argv.extend(self.options());
        argv.extend(["-T".to_owned(), self.destination.to_string(), "--".to_owned(), Line(&call.argv()).to_string()]);
        argv
    }

    /// The `rsync` that makes the host's tree equal to the workspace.
    pub(crate) fn sync(&self, push: &Push<'_>) -> Vec<String> {
        let wrapped = format!("flock {} rsync", crate::job::quote::Quoted::new(push.lock));
        let mut argv = self.rsync();
        argv.extend(Self::PLACE.map(str::to_owned));
        argv.extend(["--delete".to_owned(), "--filter=:- .gitignore".to_owned(), format!("--rsync-path={wrapped}")]);
        argv.extend(crate::cache::inputs::BUILT_IN.map(|pattern| format!("--exclude={pattern}")));
        argv.extend(push.exclude.iter().map(|name| format!("--exclude={name}")));
        argv.push(format!("{}/", push.root.display()));
        argv.push(format!("{}:{}/", self.destination, push.tree));
        argv
    }

    /// The `rsync` that brings listed files back.
    pub(crate) fn fetch(&self, pull: &Pull<'_>) -> Vec<String> {
        let mut argv = self.rsync();
        // a file brought back is as new as the moment it arrives, like one edited here
        argv.push("--no-times".to_owned());
        argv.push("--files-from=-".to_owned());
        argv.push(format!("{}:{}/", self.destination, pull.tree));
        argv.push(format!("{}/", pull.root.display()));
        argv
    }

    fn rsync(&self) -> Vec<String> {
        let mut shell = vec!["ssh".to_owned()];
        shell.extend(self.options());
        vec!["rsync".to_owned(), "-a".to_owned(), "--quiet".to_owned(), "-e".to_owned(), shell.join(" ")]
    }
}

impl Transport for Ssh {
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        self.exec.spawn(&self.argv(call), call.stdin.into(), lines)
    }

    fn push(&self, push: &Push<'_>) -> Result<(), Failure> {
        self.exec.spawn(&self.sync(push), Feed::Nothing, &mut |_| {})?.ok().map(|_| ())
    }

    fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure> {
        self.exec.spawn(&self.fetch(pull), Feed::Text(pull.files), &mut |_| {})?.ok().map(|_| ())
    }

    fn is_local(&self) -> bool {
        false
    }
}

/// Either way of reaching a host: what a [`Fleet`](crate::host::fleet::Fleet) is made of.
#[derive(Debug, Clone)]
pub(crate) enum Link {
    Local(Local),
    Ssh(Ssh),
}

impl Transport for Link {
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        match self {
            Self::Local(local) => local.call(call, lines),
            Self::Ssh(ssh) => ssh.call(call, lines),
        }
    }

    fn push(&self, push: &Push<'_>) -> Result<(), Failure> {
        match self {
            Self::Local(local) => local.push(push),
            Self::Ssh(ssh) => ssh.push(push),
        }
    }

    fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure> {
        match self {
            Self::Local(local) => local.pull(pull),
            Self::Ssh(ssh) => ssh.pull(pull),
        }
    }

    fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::transport::program::Program;

    fn ssh() -> Ssh {
        let mut ssh = Ssh::new(Exec::new(Arc::new(Console::new(false))), "me@box.lan".parse().unwrap());
        ssh.control = Some(PathBuf::from("/tmp/c-%C"));
        ssh
    }

    #[test]
    fn the_remote_command_is_one_quoted_word() {
        let call = Call::new(Program::Log, vec!["/d ir".to_owned(), "it's; rm -rf /".to_owned()]);
        let argv = ssh().argv(&call);
        assert_eq!(argv.first().map(String::as_str), Some("ssh"));
        let at = argv.iter().position(|word| word == "--").unwrap();
        assert_eq!(argv.get(at - 1).map(String::as_str), Some("me@box.lan"));
        assert_eq!(argv.len(), at + 2, "everything after `--` is one word");
        let remote = argv.last().unwrap();
        assert!(remote.starts_with("'sh' '-c' '"), "{remote}");
        assert!(remote.ends_with(r" 'sh' '/d ir' 'it'\''s; rm -rf /'"), "{remote}");
        assert!(argv.iter().any(|word| word == "ControlPath=/tmp/c-%C"));
    }

    #[test]
    fn the_sync_is_a_delete_with_the_excludes_and_the_lock_around_the_remote_rsync() {
        let exclude: Vec<crate::cli::name::Excluded> =
            [".jj", ".git", ".impeccable"].iter().map(|name| name.parse().unwrap()).collect();
        let push = Push {
            root: Path::new("/w/farm"),
            tree: "farm/uses.ink/farm",
            lock: "farm/locks/farm.lock",
            exclude: &exclude,
        };
        let argv = ssh().sync(&push);
        assert_eq!(argv.first().map(String::as_str), Some("rsync"));
        for want in [
            "-a",
            "--checksum",
            "--no-times",
            "--delete",
            "--filter=:- .gitignore",
            "--exclude=target",
            "--exclude=.jj",
            "--exclude=.git",
            "--exclude=.env",
            "--exclude=*.pem",
            "--exclude=*.key",
            "--exclude=.impeccable",
        ] {
            assert!(argv.iter().any(|word| word == want), "{want}");
        }
        assert!(argv.contains(&"--rsync-path=flock 'farm/locks/farm.lock' rsync".to_owned()));
        assert_eq!(argv.get(argv.len() - 2).map(String::as_str), Some("/w/farm/"));
        assert_eq!(argv.last().map(String::as_str), Some("me@box.lan:farm/uses.ink/farm/"));
    }

    /// The real sync argv, pointed at two local directories: no `ssh`, no remote lock.
    fn local_sync(from: &Path, to: &Path) -> Vec<String> {
        let exclude: Vec<crate::cli::name::Excluded> =
            [".jj", ".git"].iter().map(|name| name.parse().unwrap()).collect();
        let push = Push { root: from, tree: "t", lock: "l", exclude: &exclude };
        let mut argv = ssh().sync(&push);
        argv.truncate(argv.len() - 2);
        let at = argv.iter().position(|word| word == "-e").unwrap();
        argv.drain(at..=at + 1);
        argv.retain(|word| !word.starts_with("--rsync-path"));
        argv.extend([format!("{}/", from.display()), format!("{}/", to.display())]);
        argv
    }

    fn stamp(path: &Path, secs: u64) {
        let at = std::time::UNIX_EPOCH + Duration::from_secs(secs);
        std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    fn mtime(path: &Path) -> std::time::SystemTime {
        path.metadata().unwrap().modified().unwrap()
    }

    #[test]
    fn a_synced_file_whose_bytes_changed_is_newer_than_the_target_and_one_that_did_not_keeps_its_time() {
        let base = std::env::temp_dir().join(format!("farm-sync-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let (from, to) = (base.join("from"), base.join("to"));
        std::fs::create_dir_all(to.join("target")).unwrap();
        std::fs::create_dir_all(&from).unwrap();
        // the host: a tree seeded from a sibling, its artifact built after its sources
        std::fs::write(to.join("lib.rs"), "old").unwrap();
        std::fs::write(to.join("same.rs"), "same").unwrap();
        std::fs::write(to.join("target").join("dep-info"), "built").unwrap();
        stamp(&to.join("lib.rs"), 500);
        stamp(&to.join("same.rs"), 600);
        stamp(&to.join("target").join("dep-info"), 2000);
        // the workspace after a rebase: other bytes in `lib.rs`, whose time is older than the artifact
        std::fs::write(from.join("lib.rs"), "new").unwrap();
        std::fs::write(from.join("same.rs"), "same").unwrap();
        stamp(&from.join("lib.rs"), 1000);
        stamp(&from.join("same.rs"), 1500);
        let argv = local_sync(&from, &to);
        let done = Command::new(argv.first().unwrap()).args(argv.iter().skip(1)).status().unwrap();
        assert!(done.success());
        assert_eq!(std::fs::read_to_string(to.join("lib.rs")).unwrap(), "new");
        assert!(mtime(&to.join("lib.rs")) > mtime(&to.join("target").join("dep-info")), "cargo would call it fresh");
        let kept = std::time::UNIX_EPOCH + Duration::from_secs(600);
        assert_eq!(mtime(&to.join("same.rs")), kept, "same bytes, same time: nothing rebuilds");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_fetch_reads_its_list_from_stdin_never_from_the_command_line() {
        let pull = Pull { root: Path::new("/w/farm"), tree: "farm/uses.ink/farm", files: "a.snap\nb.snap\n" };
        let argv = ssh().fetch(&pull);
        assert!(argv.contains(&"--files-from=-".to_owned()));
        assert!(!argv.iter().any(|word| word.contains("a.snap")));
    }

    fn exec() -> Exec {
        Exec::new(Arc::new(Console::new(false)))
    }

    fn sh(script: &str) -> Vec<String> {
        ["sh", "-c", script].map(str::to_owned).to_vec()
    }

    #[test]
    fn a_detach_that_cannot_be_written_to_a_live_child_is_a_typed_failure() {
        let done = exec().spawn(&sh("exec 0<&-; exec sleep 5"), Feed::Open(Duration::from_millis(30)), &mut |_| {});
        assert!(matches!(done, Err(Failure::Stdin(_))), "{done:?}");
    }

    #[test]
    fn stderr_that_is_not_utf8_is_kept_lossily_not_dropped() {
        let done = exec().spawn(&sh("printf 'a\\377b' >&2"), Feed::Nothing, &mut |_| {}).unwrap();
        assert_eq!(done.stderr, "a\u{fffd}b");
    }
}
