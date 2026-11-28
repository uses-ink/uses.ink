//! The shell programs that run on a host, and the calls that start them.
//!
//! A program is a fixed text given to `sh -c`; everything that varies reaches it as a positional
//! parameter, so no config value, path or cargo argument is ever part of shell syntax. A
//! [`Call`] is a program with its arguments, and the one place a command line for a host is
//! made.
//!
//! State lives under the host's directory `D` (the argument `dir`; a relative one is under
//! `$HOME`):
//!
//! - `D/slots/<n>.lock`: one lock per slot; a job's body holds it with `flock` for as long as its
//!   command runs, so a slot is free exactly when nothing runs in it. Only the body holds it: the
//!   command starts with the lock's descriptor closed, so a child that outlives the job cannot
//!   keep the slot, and the body kills the job's process groups (its own and `timeout`'s) when
//!   the command ends. `D/slots/<n>.holder` says `<body pid> <since>`.
//! - `D/queue/<stamp>-<job>`: a ticket per waiting job holding its pid; the oldest live ticket
//!   may take a slot, the others wait, and a ticket whose pid is gone is removed by whoever
//!   finds it. That is the whole queue: no daemon.
//! - `D/jobs/<job>/`: `cmd`, `body` (pid), `started`, `pid` (of the command), `events`, `log`,
//!   `rc` and `done` (`<rc> <seconds>`, written last).
//! - `D/locks/<workspace>.lock` and `D/<repo>/<workspace>/`: the synced trees.

use std::fmt;
use std::time::Duration;

use crate::host::transport::Cancel;
use crate::job::quote::Line;

/// A program that runs on a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Program {
    /// Prints what the host is doing: arch, cores, load, slots, queue, jobs.
    Probe,
    /// Starts a job's body and, unless detaching, follows it until it ends.
    Run,
    /// The job's body: waits for its turn, takes a slot, runs the command, records the result.
    Body,
    /// Blocks until a job is done, or a time passes.
    Wait,
    /// Prints a job's log, whole or its tail.
    Log,
    /// Makes a workspace's directory from the most recently used warm sibling by a reflink copy.
    Seed,
    /// Removes a workspace's directory, when it is inside the farm's.
    Remove,
    /// Lists the workspace directories of one repo and their sizes.
    List,
    /// Lists the files of one extension a job changed.
    Changed,
    /// Removes the state of jobs older than seven days.
    Sweep,
}

impl Program {
    /// Its name, in a test's message.
    #[cfg(test)]
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Run => "run",
            Self::Body => "body",
            Self::Wait => "wait",
            Self::Log => "log",
            Self::Seed => "seed",
            Self::Remove => "remove",
            Self::List => "list",
            Self::Changed => "changed",
            Self::Sweep => "sweep",
        }
    }

    /// Its text.
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Probe => PROBE,
            Self::Run => RUN,
            Self::Body => BODY,
            Self::Wait => WAIT,
            Self::Log => LOG,
            Self::Seed => SEED,
            Self::Remove => REMOVE,
            Self::List => LIST,
            Self::Changed => CHANGED,
            Self::Sweep => SWEEP,
        }
    }
}

/// The prefix of a line a program says about itself, as opposed to a job's output.
pub(crate) const MARK: &str = "@@farm:";

/// Whether a call keeps its standard input open for the client to speak on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stdin {
    /// Closed: the call needs nothing from the client.
    Null,
    /// Open until the call ends; the client may write `detach` to let the job outlive it.
    Held(Duration),
}

/// A program and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Call {
    pub(crate) program: Program,
    pub(crate) args: Vec<String>,
    pub(crate) stdin: Stdin,
    /// When set, the call is stopped and its host side with it.
    pub(crate) cancel: Option<Cancel>,
}

impl Call {
    pub(crate) fn new(program: Program, args: Vec<String>) -> Self {
        Self { program, args, stdin: Stdin::Null, cancel: None }
    }

    /// The same call with its input held open, for at most `until`.
    pub(crate) fn held(self, until: Duration) -> Self {
        Self { stdin: Stdin::Held(until), ..self }
    }

    /// The same call, stopped when `cancel` is set: the client ends, and the job on the host ends
    /// with it, as it does for a client that goes away.
    pub(crate) fn cancelled_by(self, cancel: Cancel) -> Self {
        Self { cancel: Some(cancel), ..self }
    }

    /// The argument vector that runs it with `sh`: `sh -c <text> sh <args>`.
    pub(crate) fn argv(&self) -> Vec<String> {
        let mut argv = vec!["sh".to_owned(), "-c".to_owned(), self.program.text().to_owned(), "sh".to_owned()];
        argv.extend(self.args.iter().cloned());
        argv
    }
}

impl fmt::Display for Call {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", Line(&self.argv()))
    }
}

/// Resolves a relative `dir` against `$HOME`; the first lines of every program.
macro_rules! home {
    ($var:literal) => {
        concat!("case $", $var, " in /*) ;; *) ", $var, "=$HOME/$", $var, " ;; esac\n")
    };
}

/// How many polls, 10 ms apart, `RUN` gives the body to write its pid before it reports `nobody`:
/// five seconds.
macro_rules! body_polls {
    () => {
        "500"
    };
}

const PROBE: &str = concat!(
    "dir=$1 want=$2\n",
    home!("dir"),
    "mkdir -p \"$dir/slots\" \"$dir/queue\" \"$dir/jobs\" || exit 70\n",
    r#"cores=$(nproc)
slots=$want
[ -n "$slots" ] || slots=$((cores / 6))
[ "$slots" -ge 1 ] || slots=1
echo "now $(date +%s)"
echo "arch $(uname -m)"
echo "cores $cores"
echo "load $(cut -d' ' -f1 /proc/loadavg)"
echo "slots $slots"
n=0
while [ "$n" -lt "$slots" ]; do
  if ( exec 9>>"$dir/slots/$n.lock"; flock -n 9 ); then echo "slot $n free"
  else
    echo "slot $n busy"
    holder=$(cat "$dir/slots/$n.holder" 2>/dev/null)
    if [ -n "$holder" ]; then
      set -- $holder
      if kill -0 "$1" 2>/dev/null; then echo "holder $n $holder alive"; else echo "holder $n $holder gone"; fi
    fi
  fi
  n=$((n + 1))
done
live=0
for t in $(ls "$dir/queue"); do
  if kill -0 "$(cat "$dir/queue/$t" 2>/dev/null)" 2>/dev/null; then live=$((live + 1)); else rm -f "$dir/queue/$t"; fi
done
echo "queued $live"
for name in $(ls -t "$dir/jobs" | head -n 40); do
  job=$dir/jobs/$name
  [ -d "$job" ] || continue
  shown=$(cat "$job/cmd" 2>/dev/null)
  body=$(cat "$job/body" 2>/dev/null)
  if [ -f "$job/done" ]; then echo "job $name done $(stat -c %Y "$job/done") $(cat "$job/done") $shown"
  elif [ -n "$body" ] && ! kill -0 "$body" 2>/dev/null; then echo "job $name lost $(stat -c %Y "$job/events") $shown"
  elif [ -f "$job/started" ]; then echo "job $name running $(cat "$job/started") $shown"
  else echo "job $name queued $(stat -c %Y "$job/events") $shown"; fi
done
"#
);

const RUN: &str = concat!(
    "mode=$1 text=$2 dir=$3 id=$4 slots=$5 limit=$6 tree=$7 mold=$8 shown=$9\nshift 9\n",
    home!("dir"),
    home!("tree"),
    r#"job=$dir/jobs/$id
mkdir -p "$dir/slots" "$dir/queue" "$job" || exit 70
: > "$job/events"; : > "$job/log"
lease=
[ "$mode" = held ] && lease=$$
exec 7<&0
setsid -f sh -c "$text" body "$dir" "$id" "$slots" "$limit" "$tree" "$mold" "$lease" "$shown" "$@" </dev/null >/dev/null 2>&1
if [ "$mode" = detach ]; then echo "@@farm:started"; exit 0; fi
n=0
while [ ! -s "$job/body" ] && [ "$n" -lt "#,
    body_polls!(),
    r#" ]; do sleep 0.01; n=$((n + 1)); done
bodypid=$(cat "$job/body" 2>/dev/null)
[ -n "$bodypid" ] || { echo "@@farm:nobody"; exit 70; }
tail -q -n +1 -f -s 0.05 --pid="$bodypid" "$job/events" "$job/log" &
follower=$!
( while IFS= read -r word; do
    if [ "$word" = detach ]; then : > "$job/detached"; kill "$follower" 2>/dev/null; exit 0; fi
  done
  kill "$follower" 2>/dev/null ) <&7 &
reader=$!
wait "$follower"
kill "$reader" 2>/dev/null
[ -f "$job/detached" ] && echo "@@farm:detached"
exit 0
"#
);

const BODY: &str = concat!(
    "dir=$1 id=$2 slots=$3 limit=$4 tree=$5 mold=$6 lease=$7 shown=$8\nshift 8\n",
    r#"job=$dir/jobs/$id
say() { printf '@@farm:%s\n' "$*" >> "$job/events"; }
printf '%s\n' "$$" > "$job/body"
printf '%s\n' "$shown" > "$job/cmd"
ticket=
now() { date +%s; }
gone() { [ -n "$lease" ] && [ ! -f "$job/detached" ] && ! kill -0 "$lease" 2>/dev/null; }
own=
[ "$(ps -o pgid= -p "$$" 2>/dev/null | tr -d ' ')" = "$$" ] && own=$$
reap() {
  trap '' TERM HUP INT
  groups="$own $(cat "$job/pid" 2>/dev/null)"
  for pg in $groups; do kill -TERM -- "-$pg" 2>/dev/null; done
  sleep 0.2
  for pg in $groups; do
    for p in $(pgrep -g "$pg" 2>/dev/null); do [ "$p" = "$$" ] || kill -KILL "$p" 2>/dev/null; done
  done
}
quit() {
  [ -f "$job/pid" ] && kill -TERM "$(cat "$job/pid")" 2>/dev/null
  reap
  [ -n "$ticket" ] && rm -f "$ticket"
  printf '143 0\n' > "$job/done"
  say aborted
  exit 143
}
trap quit TERM HUP INT
start=$(now)
stamp=$(date +%s%N)-$id
ticket=$dir/queue/$stamp
printf '%s\n' "$$" > "$dir/queue/.$stamp" && mv "$dir/queue/.$stamp" "$ticket"
slot=
last=-30
while [ -z "$slot" ]; do
  gone && quit
  behind=0
  for t in $(ls "$dir/queue"); do
    [ "$dir/queue/$t" = "$ticket" ] && break
    if kill -0 "$(cat "$dir/queue/$t" 2>/dev/null)" 2>/dev/null; then behind=$((behind + 1)); else rm -f "$dir/queue/$t"; fi
  done
  if [ "$behind" -eq 0 ]; then
    n=0
    while [ "$n" -lt "$slots" ]; do
      exec 9>>"$dir/slots/$n.lock"
      if flock -n 9; then slot=$n; break; fi
      n=$((n + 1))
    done
  fi
  [ -n "$slot" ] && break
  at=$(now)
  if [ $((at - last)) -ge 30 ]; then say "queued $behind"; last=$at; fi
  sleep 0.2
done
rm -f "$ticket"
ticket=
began=$(now)
printf '%s %s\n' "$$" "$began" > "$dir/slots/$slot.holder"
say "slot $slot $((began - start))"
echo "$began" > "$job/started"
cd "$tree" 2>/dev/null || { say nodir; printf '66 0\n' > "$job/done"; exit 66; }
[ -d target ] && touch target
PATH=$HOME/.cargo/bin:$PATH
export PATH
if [ "$mold" = mold ] && [ -z "${RUSTFLAGS-}" ]; then
  triple=$(rustc -vV | sed -n 's/^host: //p' | tr 'a-z-' 'A-Z_')
  [ -n "$triple" ] && export "CARGO_TARGET_${triple}_RUSTFLAGS=-C link-arg=-fuse-ld=mold"
fi
( timeout -k 15 "$limit" "$@" </dev/null >"$job/log" 2>&1 9>&- &
  echo $! > "$job/pid"
  wait $!
  echo $? > "$job/rc" ) &
t=10
while [ ! -f "$job/rc" ]; do
  gone && quit
  sleep "0.$(printf %03d "$t")"
  t=$((t * 2)); [ "$t" -le 250 ] || t=250
done
rc=$(cat "$job/rc")
reap
ran=$(($(now) - began))
printf '%s %s\n' "$rc" "$ran" > "$job/done"
say "done $rc $ran"
exit "$rc"
"#
);

const WAIT: &str = concat!(
    "dir=$1 id=$2 limit=$3\n",
    home!("dir"),
    r#"job=$dir/jobs/$id
[ -d "$job" ] || { echo "@@farm:nojob"; exit 0; }
waited=0 t=10
while [ ! -f "$job/done" ] && [ "$waited" -lt "$((limit * 1000))" ]; do
  body=$(cat "$job/body" 2>/dev/null)
  if [ -n "$body" ] && ! kill -0 "$body" 2>/dev/null && [ ! -f "$job/done" ]; then echo "@@farm:lost"; exit 0; fi
  sleep "0.$(printf %03d "$t")"
  waited=$((waited + t))
  t=$((t * 2)); [ "$t" -le 250 ] || t=250
done
if [ -f "$job/done" ]; then echo "@@farm:done $(cat "$job/done")"; else echo "@@farm:running"; fi
"#
);

const LOG: &str = concat!(
    "dir=$1 id=$2 tail=$3\n",
    home!("dir"),
    r#"job=$dir/jobs/$id
[ -f "$job/log" ] || { echo "@@farm:nojob"; exit 0; }
if [ -n "$tail" ]; then tail -n "$tail" "$job/log"; else cat "$job/log"; fi
"#
);

const SEED: &str = concat!(
    "dir=$1 repo=$2 ws=$3\n",
    home!("dir"),
    r#"base=$dir/$repo
mkdir -p "$base" "$dir/locks" || exit 70
exec 8>"$dir/locks/$ws.lock"
flock 8
[ -d "$base/$ws" ] && { echo "@@farm:seed present"; exit 0; }
rm -rf "$base/${ws:?}.seed"
# the sibling whose target/ a job used last (a job touches it as it starts), and only one that has
# built something: the newest directory by its own time is as often a copy that never built
newest=
for t in $(ls -td "$base"/*/target 2>/dev/null); do
  w=${t%/target}; w=${w##*/}
  case $w in *.seed) continue ;; esac
  if [ -d "$t/debug" ] || [ -d "$t/release" ]; then newest=$w; break; fi
done
if [ -z "$newest" ]; then mkdir -p "$base/$ws"; echo "@@farm:seed empty"; exit 0; fi
if cp -a --reflink=always "$base/$newest" "$base/$ws.seed" 2>/dev/null; then how=reflink
else
  rm -rf "$base/${ws:?}.seed"
  cp -a --reflink=auto "$base/$newest" "$base/$ws.seed" || exit 74
  how=copy
fi
mv "$base/$ws.seed" "$base/$ws" || exit 74
echo "@@farm:seed $how $newest"
"#
);

/// Removes `D/<repo>/<ws>` and nothing else: the name must be one path segment, the directory a
/// real one (never a symlink), and its resolved path directly inside the resolved `D/<repo>`,
/// which is itself inside the resolved `D`. Anything else is refused and left alone. The
/// workspace's sync lock is held, so a sync in progress finishes first.
const REMOVE: &str = concat!(
    "dir=$1 repo=$2 ws=$3\n",
    home!("dir"),
    r#"base=$dir/$repo
target=$base/$ws
case $ws in ''|.|..|-*|*/*) echo "@@farm:refused $target"; exit 0 ;; esac
if [ -L "$target" ]; then echo "@@farm:refused $target"; exit 0; fi
[ -d "$target" ] || { echo "@@farm:absent"; exit 0; }
farm=$(cd "$dir" 2>/dev/null && pwd -P) || { echo "@@farm:refused $dir"; exit 0; }
root=$(cd "$base" 2>/dev/null && pwd -P) || { echo "@@farm:refused $base"; exit 0; }
real=$(cd "$target" 2>/dev/null && pwd -P) || { echo "@@farm:refused $target"; exit 0; }
case $root in "$farm"/*) ;; *) echo "@@farm:refused $root"; exit 0 ;; esac
[ "$real" = "$root/$ws" ] || { echo "@@farm:refused $real"; exit 0; }
mkdir -p "$dir/locks" || exit 70
exec 8>"$dir/locks/$ws.lock"
flock 8
kib=$(du -sk "$real" 2>/dev/null | cut -f1)
rm -rf "${real:?}" "${root:?}/${ws:?}.seed" || exit 74
echo "@@farm:removed ${kib:-0}"
"#
);

/// Prints a `copy <name> <kib>` line for each workspace directory of the repo whose name is one
/// the farm would have made, leaving out half-made `.seed` copies.
const LIST: &str = concat!(
    "dir=$1 repo=$2\n",
    home!("dir"),
    r#"base=$dir/$repo
[ -d "$base" ] || exit 0
for d in "$base"/*; do
  [ -d "$d" ] && [ ! -L "$d" ] || continue
  name=${d##*/}
  case $name in *.seed|.*|-*|*[!A-Za-z0-9._-]*) continue ;; esac
  kib=$(du -sk "$d" 2>/dev/null | cut -f1)
  echo "@@farm:copy $name ${kib:-0}"
done
"#
);

const CHANGED: &str = concat!(
    "dir=$1 id=$2 tree=$3 pattern=$4\n",
    home!("dir"),
    home!("tree"),
    r#"cd "$tree" || exit 66
find . -path ./target -prune -o -type f -name "$pattern" -newer "$dir/jobs/$id/started" -print
"#
);

const SWEEP: &str = concat!(
    "dir=$1\n",
    home!("dir"),
    r#"[ -d "$dir/jobs" ] || { echo "@@farm:swept 0"; exit 0; }
count=0
for old in $(find "$dir/jobs" -mindepth 1 -maxdepth 1 -type d -mtime +7); do
  rm -rf "$old" && count=$((count + 1))
done
find "$dir/queue" -mindepth 1 -maxdepth 1 -type f -mtime +7 -delete 2>/dev/null
echo "@@farm:swept $count"
"#
);

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    const ALL: [Program; 10] = [
        Program::Probe,
        Program::Run,
        Program::Body,
        Program::Wait,
        Program::Log,
        Program::Seed,
        Program::Remove,
        Program::List,
        Program::Changed,
        Program::Sweep,
    ];

    #[test]
    fn every_program_is_valid_shell() {
        for program in ALL {
            let status = Command::new("sh").arg("-n").arg("-c").arg(program.text()).status().unwrap();
            assert!(status.success(), "{}", program.name());
        }
    }

    /// A scratch farm directory with a repo `r` in it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let base = std::env::temp_dir().join(format!("farm-seed-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&base).ok();
            std::fs::create_dir_all(base.join("farm").join("r")).unwrap();
            Self(base)
        }

        fn ws(&self, name: &str) -> std::path::PathBuf {
            self.0.join("farm").join("r").join(name)
        }

        /// A workspace with a source file, and a built `target/` when `built`, used at `used`.
        fn make(&self, name: &str, built: bool, used: u64) {
            let ws = self.ws(name);
            std::fs::create_dir_all(ws.join("src")).unwrap();
            std::fs::write(ws.join("src").join("lib.rs"), name).unwrap();
            if built {
                std::fs::create_dir_all(ws.join("target").join("debug")).unwrap();
                let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(used);
                std::fs::File::open(ws.join("target")).unwrap().set_modified(at).unwrap();
            }
        }

        fn seed(&self, name: &str) -> String {
            let out = Command::new("sh")
                .args(["-c", Program::Seed.text(), "sh"])
                .arg(self.0.join("farm"))
                .args(["r", name])
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap().trim().to_owned()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn a_new_workspace_is_seeded_from_the_warm_sibling_used_last_not_the_newest_directory() {
        let scratch = Scratch::new("pick");
        scratch.make("old", true, 1_000);
        scratch.make("recent", true, 5_000);
        scratch.make("never-built", false, 0); // the newest directory, with nothing to reuse
        let said = scratch.seed("fresh");
        assert!(said == "@@farm:seed reflink recent" || said == "@@farm:seed copy recent", "{said}");
        assert_eq!(std::fs::read_to_string(scratch.ws("fresh").join("src").join("lib.rs")).unwrap(), "recent");
        assert!(scratch.ws("fresh").join("target").join("debug").is_dir(), "the target came with it");
        assert!(!scratch.ws("fresh.seed").exists(), "no half-made copy is left");
        assert_eq!(scratch.seed("fresh"), "@@farm:seed present");
    }

    #[test]
    fn with_no_warm_sibling_the_workspace_starts_empty_and_a_half_made_seed_is_never_a_source() {
        let scratch = Scratch::new("cold");
        scratch.make("idle", false, 0);
        scratch.make("half.seed", true, 9_000);
        assert_eq!(scratch.seed("fresh"), "@@farm:seed empty");
        assert!(scratch.ws("fresh").is_dir());
        assert!(!scratch.ws("fresh").join("src").exists());
    }

    #[test]
    fn a_call_is_sh_text_and_arguments() {
        let call = Call::new(Program::Log, vec!["d".to_owned(), "it's".to_owned()]);
        let argv = call.argv();
        assert_eq!(argv.first().map(String::as_str), Some("sh"));
        assert_eq!(argv.last().map(String::as_str), Some("it's"));
        assert!(call.to_string().ends_with(r" 'sh' 'd' 'it'\''s'"));
    }
}
