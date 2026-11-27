//! Another machine, through the system `ssh` and `rsync`.

use std::path::PathBuf;

use crate::cli::name::Destination;
use crate::host::transport::exec::Feed;
use crate::host::transport::process::Failure;
use crate::host::transport::program::Call;
use crate::host::transport::{Done, Exec, Pull, Push, Transport};
use crate::job::quote::Line;

/// How long `ssh` keeps a shared connection open after its last call, in seconds.
const CONTROL_PERSIST_SECS: u32 = 120;

/// `ssh`'s connect timeout, in seconds: a host that does not answer in this time is down.
const CONNECT_SECS: u32 = 8;

/// How often `ssh` sends a keep-alive on a quiet connection, in seconds.
const ALIVE_SECS: u32 = 15;

/// How many keep-alives may go unanswered before `ssh` gives the connection up.
const ALIVE_MISSES: u32 = 3;

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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::cli::console::Console;
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
}
