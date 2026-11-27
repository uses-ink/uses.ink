//! A farm of this machine alone, in a temp dir: the real `sh`, `flock`, `timeout` and `tail`.

use super::*;

/// A farm of this machine alone, in a temp dir: the real `sh`, `flock`, `timeout` and `tail`.
pub(super) struct Rig {
    pub(super) root: std::path::PathBuf,
}

impl Rig {
    pub(super) fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("farm-rig-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("w")).unwrap();
        Self { root }
    }

    pub(super) fn farm(&self, slots: u32, flags: Flags) -> Farm<Link, LeastLoaded> {
        self.build(slots, flags, false)
    }

    /// A farm that keeps passes under the rig, reading the tree with a plain walk.
    pub(super) fn cached(&self, slots: u32, flags: Flags) -> Farm<Link, LeastLoaded> {
        self.build(slots, flags, true)
    }

    pub(super) fn build(&self, slots: u32, flags: Flags, cached: bool) -> Farm<Link, LeastLoaded> {
        let settings = crate::config::jobs::Settings::default();
        let cache = cached.then(|| {
            let inputs = cache::inputs::Inputs::new(Arc::new(cache::inputs::Walk), &settings.exclude).unwrap();
            Cache::at(&self.root.join("cache"), settings.limits, settings.memory, inputs)
        });
        let host = Host { slots: Some(slots), dir: self.dir(), ..Host::local() };
        let console = Arc::new(Console::default());
        let local = Link::Local(Local::new(Exec::new(Arc::clone(&console))));
        let tree = Tree { root: self.root.join("w"), repo: "r".parse().unwrap(), workspace: "w".parse().unwrap() };
        Farm::new(Setup {
            fleet: Fleet::new(vec![Member::new(host, local)]),
            placement: LeastLoaded,
            console,
            flags,
            tree,
            settings,
            cache,
        })
    }

    pub(super) fn dir(&self) -> host::HostDir {
        self.root.join("farm").to_str().unwrap().parse().unwrap()
    }

    pub(super) fn member(&self) -> Member<Link> {
        let console = Arc::new(Console::default());
        Member::new(Host { dir: self.dir(), ..Host::local() }, Link::Local(Local::new(Exec::new(console))))
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

pub(super) fn cargo_free(words: &[&str]) -> Vec<String> {
    // `sh -c` stands in for cargo so the programs run without a toolchain
    let mut args = vec!["sh".to_owned(), "-c".to_owned()];
    args.extend(words.iter().map(|word| (*word).to_owned()));
    args
}

pub(super) fn job(label: &str, script: &str) -> job::Job {
    job::Job {
        label: label.to_owned(),
        words: cargo_free(&[script]),
        need: place::Need::Any,
        env: vec![],
        limit: None,
        ceiling: None,
        reuse: job::Reuse::Never,
        key: place::history::Key::stage(label),
    }
}

/// A job that counts its runs in the rig's `count` file, then runs `more`.
pub(super) fn counting(rig: &Rig, more: &str) -> job::Job {
    let mut counted = job("count", &format!("echo x >> {}; {more}", rig.root.join("count").display()));
    counted.reuse = job::Reuse::Whole;
    counted
}

pub(super) fn runs(rig: &Rig) -> usize {
    std::fs::read_to_string(rig.root.join("count")).map_or(0, |text| text.lines().count())
}

/// One job through a new farm of the rig, as a new process would run it.
pub(super) fn again(rig: &Rig, job: &job::Job, flags: Flags) -> job::Finished {
    let farm = rig.cached(1, flags);
    let probed = farm.fleet_probe();
    farm.run(job, probed.first().unwrap(), job::Attach::Hold(Duration::from_secs(60))).unwrap()
}

pub(super) fn run_one(rig: &Rig, job: &job::Job, attach: job::Attach, slots: u32) -> job::Finished {
    let farm = rig.farm(slots, Flags::default());
    let probed = farm.fleet_probe();
    farm.run(job, probed.first().unwrap(), attach).unwrap()
}

pub(super) fn farm_tree(rig: &Rig) -> Tree {
    Tree { root: rig.root.join("w"), repo: "r".parse().unwrap(), workspace: "w".parse().unwrap() }
}

/// Polls `check` for up to five seconds.
pub(super) fn within(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..1000 {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}
