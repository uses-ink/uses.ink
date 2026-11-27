//! A farm over the scripted fake transport.

use super::*;
use crate::cli::Seconds;
use crate::host::Via;
use crate::job::Job;
use crate::job::Reuse;
use crate::place::Need;

pub(super) fn host(name: &str) -> Host {
    Host { name: name.parse().unwrap(), via: Via::Ssh("x".parse().unwrap()), ..Host::local() }
}

pub(super) fn probe(cores: u32, arch: &str) -> Reply {
    Reply::ok(&[
        "now 1000",
        &format!("arch {arch}"),
        &format!("cores {cores}"),
        "load 0.10",
        "slots 1",
        "slot 0 free",
        "queued 0",
    ])
}

/// Each test's own workspace directory, so tests that write files do not meet.
pub(super) static NEXT: AtomicU32 = AtomicU32::new(0);

pub(super) fn farm(members: Vec<Member<Fake>>) -> Farm<Fake, LeastLoaded> {
    Farm::<Fake, LeastLoaded>::with(members, Flags::default(), None)
}

impl Farm<Fake, LeastLoaded> {
    /// A farm over fakes, with these flags and, when given, a cache kept in this directory.
    pub(super) fn with(members: Vec<Member<Fake>>, flags: Flags, cache: Option<&std::path::Path>) -> Self {
        let settings = Settings::default();
        let cache = cache.map(|dir| {
            let inputs =
                crate::cache::inputs::Inputs::new(Arc::new(crate::cache::inputs::Walk), &settings.exclude).unwrap();
            Cache::at(dir, settings.limits, settings.memory, inputs)
        });
        Self::built(members, flags, cache)
    }

    pub(super) fn built(members: Vec<Member<Fake>>, flags: Flags, cache: Option<Cache>) -> Self {
        let tree = Tree {
            root: std::env::temp_dir().join(format!(
                "farm-run-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            repo: "r".parse().unwrap(),
            workspace: "w".parse().unwrap(),
        };
        Farm::new(Setup {
            fleet: Fleet::new(members),
            placement: LeastLoaded,
            console: Arc::new(Console::default()),
            flags,
            tree,
            settings: Settings::default(),
            cache,
        })
    }
}

/// A scratch directory and a script in it that records its arguments in `marker` and exits
/// with `code`.
pub(super) struct Wakeup {
    pub(super) dir: std::path::PathBuf,
}

impl Wakeup {
    pub(super) fn new(code: i32) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!(
            "farm-wake-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("wake.sh");
        let body = format!("#!/bin/sh\necho \"$@\" >> {}\nexit {code}\n", dir.join("marker").display());
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    pub(super) fn host(&self, name: &str) -> Host {
        let command = self.dir.join("wake.sh").display().to_string();
        Host { wake: Wake::new(&command, Some(Seconds(5)), Some(Seconds(120))), ..host(name) }
    }

    /// How many times the script ran, and with what.
    pub(super) fn woken(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("marker"))
            .map_or_else(|_| Vec::new(), |text| text.lines().map(str::to_owned).collect())
    }
}

impl Drop for Wakeup {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

pub(super) fn silent() -> Reply {
    Reply { lines: vec![], code: 255 }
}

pub(super) fn asking(need: Need) -> Job {
    Job {
        label: "x".to_owned(),
        words: vec!["true".to_owned()],
        need,
        env: Vec::new(),
        limit: None,
        reuse: Reuse::Never,
        key: history::Key::stage("x"),
    }
}
