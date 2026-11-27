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
//! - `D/slots/<n>.lock`: one lock per slot; a job holds it with `flock` for as long as its
//!   command runs, so a slot is free exactly when nothing runs in it.
//! - `D/queue/<stamp>-<job>`: a ticket per waiting job holding its pid; the oldest live ticket
//!   may take a slot, the others wait, and a ticket whose pid is gone is removed by whoever
//!   finds it. That is the whole queue: no daemon.
//! - `D/jobs/<job>/`: `cmd`, `body` (pid), `started`, `pid` (of the command), `events`, `log`,
//!   `rc` and `done` (`<rc> <seconds>`, written last).
//! - `D/locks/<workspace>.lock` and `D/<repo>/<workspace>/`: the synced trees.

use std::fmt;
use std::time::Duration;

use crate::quote::Line;

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
    /// Makes a workspace's directory from the newest sibling by a reflink copy.
    Seed,
    /// Lists the `.snap` files a job changed.
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
}

impl Call {
    pub(crate) fn new(program: Program, args: Vec<String>) -> Self {
        Self { program, args, stdin: Stdin::Null }
    }

    /// The same call with its input held open, for at most `until`.
    pub(crate) fn held(self, until: Duration) -> Self {
        Self { stdin: Stdin::Held(until), ..self }
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
  if ( exec 9>"$dir/slots/$n.lock"; flock -n 9 ); then echo "slot $n free"; else echo "slot $n busy"; fi
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
while [ ! -s "$job/body" ] && [ "$n" -lt 50 ]; do sleep 0.1; n=$((n + 1)); done
bodypid=$(cat "$job/body" 2>/dev/null)
[ -n "$bodypid" ] || { echo "@@farm:nobody"; exit 70; }
tail -q -n +1 -f --pid="$bodypid" "$job/events" "$job/log" &
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
quit() {
  [ -f "$job/pid" ] && kill -TERM "$(cat "$job/pid")" 2>/dev/null
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
      exec 9>"$dir/slots/$n.lock"
      if flock -n 9; then slot=$n; break; fi
      n=$((n + 1))
    done
  fi
  [ -n "$slot" ] && break
  at=$(now)
  if [ $((at - last)) -ge 30 ]; then say "queued $behind"; last=$at; fi
  sleep 1
done
rm -f "$ticket"
ticket=
began=$(now)
say "slot $slot $((began - start))"
echo "$began" > "$job/started"
cd "$tree" 2>/dev/null || { say nodir; printf '66 0\n' > "$job/done"; exit 66; }
PATH=$HOME/.cargo/bin:$PATH
export PATH
if [ "$mold" = mold ] && [ -z "${RUSTFLAGS-}" ]; then RUSTFLAGS='-C link-arg=-fuse-ld=mold'; export RUSTFLAGS; fi
( timeout -k 15 "$limit" "$@" </dev/null >"$job/log" 2>&1 &
  echo $! > "$job/pid"
  wait $!
  echo $? > "$job/rc" ) &
while [ ! -f "$job/rc" ]; do
  gone && quit
  sleep 0.25
done
rc=$(cat "$job/rc")
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
n=0
while [ ! -f "$job/done" ] && [ "$n" -lt "$((limit * 4))" ]; do
  body=$(cat "$job/body" 2>/dev/null)
  if [ -n "$body" ] && ! kill -0 "$body" 2>/dev/null && [ ! -f "$job/done" ]; then echo "@@farm:lost"; exit 0; fi
  sleep 0.25
  n=$((n + 1))
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
newest=$(ls -t "$base" | grep -v '\.seed$' | head -n 1)
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

const CHANGED: &str = concat!(
    "dir=$1 id=$2 tree=$3\n",
    home!("dir"),
    home!("tree"),
    r#"cd "$tree" || exit 66
find . -path ./target -prune -o -type f -name '*.snap' -newer "$dir/jobs/$id/started" -print
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

    const ALL: [Program; 8] = [
        Program::Probe,
        Program::Run,
        Program::Body,
        Program::Wait,
        Program::Log,
        Program::Seed,
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

    #[test]
    fn a_call_is_sh_text_and_arguments() {
        let call = Call::new(Program::Log, vec!["d".to_owned(), "it's".to_owned()]);
        let argv = call.argv();
        assert_eq!(argv.first().map(String::as_str), Some("sh"));
        assert_eq!(argv.last().map(String::as_str), Some("it's"));
        assert!(call.to_string().ends_with(r" 'sh' 'd' 'it'\''s'"));
    }
}

