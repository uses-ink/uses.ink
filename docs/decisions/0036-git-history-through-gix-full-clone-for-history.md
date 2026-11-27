## 0036 Git history through gix: full clone for history, one walk for dates

Accepted, spike (`docs/research/gix-spike.md`). The `git` capability uses `gix` 0.91. When history is wanted it
clones in full; depth 1 plus `deepen` is for a snapshot first or a bounded window, never as the
road to full history. File dates come from one walk over history with a parallel tree diff per
commit (merges skipped, renames not followed), and files older than a shallow window get
`NotAfter`. **Why:** on tldr (40,944 files, 24,903 commits) the dates take about 2 s on 4
threads in 300 MiB; a direct full clone takes 7 s against 17 s for depth 1 plus a full deepen.
The 5 s budget holds for the dates, not for the fetch. gix has no blobless partial clone, so the
`filter=blob:none` idea in `research/architecture.md` is dropped. **Reversal:** gix gains
partial clone, or rename following turns out to matter for real sources (1.8% of tldr's files
were created by a pure move).

As built (2026-11-27), natively, before `git.fetch` and `ink:source-git` exist as components:
`host_native::Repository` reads a local repository at one commit (`HEAD`, or `--rev` as
`git rev-parse` takes it) into a `Snapshot` whose version is `Version::GitCommit`. The snapshot
is the commit's tree, never the working copy, so `ink build` in a repository builds what is
committed; it says so with `source.git.chosen`, and `--source folder` reads the files on disk
instead. The tree is read with the folder's budgets and refusals: links are resolved against
the tree (a link to a file is that file, to a folder is skipped, out of the tree is refused,
to nothing is `source.link.broken`), submodules are skipped and LFS pointers kept and said. A
file's `updated` is the newest commit by author time that touched its path and its `created`
the newest that added it, both `Known::Exact` from `Provenance::Commit`, so a file deleted and
added again was created when it came back. A shallow boundary gives the rest `NotAfter` from
`Provenance::GitHistory`; a change seen only in a merge is `Unknown { Merged }`. What the
history lacked is a `Lack` with a `Gap` (`Shallow`, or `Renames` with the count of files a
move created), said as a notice and shown by `ink explain`. gix is opened isolated (no user or
system configuration) with an 8 MiB delta-base cache per thread (as measured, below) and
without `pack-cache-lru-static`, whose `uluru` is MPL-2.0; `foldhash` and `zlib-rs` (Zlib) are
licence exceptions in `deny.toml`. Measured on tldr (40,943 files, 24,903 commits, full clone,
release build): the whole read, every blob and every date, takes 10.1 s on 1 thread, 3.3 s on 4
and 2.6 s on 8, in 150 MiB; it matches the spike's 732 files created by a move.

As measured (2026-11-27, performance pass, `cargo bench -p uses-ink-host-native` with
`INK_BENCH_REPO` naming tldr; the machine shared with other builds, so CPU counts are the firmer
numbers): the `core.deltaBaseCacheLimit=32m` override above never took effect. gix sets a
delta-base cache from it only under its `max-performance` feature, which pulls the MPL code, so
every tree was inflated from its pack again, and zlib was 68% of the profile. The cache is now set
on each thread's handle in code, 8 MiB each: the read with dates went from 4.6 s to 2.7 s on 4
threads and from about 170 to about 85 billion instructions, at 224 MiB peak instead of 156 MiB.
4, 8, 16 and 32 MiB all land at 2.7 s; 64 MiB is slower. An object cache on top of it made the
diffs slower, not faster. Of the 2.7 s, the tree read (every blob inflated and hashed) is 0.35 s
and the commit walk 0.2 s; the rest is the parallel tree diff, still mostly inflate. Left alone:
the diff algorithm (gix's tree diff is a merge of sorted entries, already linear), object lookups
(the pack index lookup is under 1%), and allocation (`memmove` is 8%, mostly delta application).
The folder walk reads a generated tree of 20,000 one-kilobyte notes in about 160 ms, almost all
of it system calls and BLAKE3; it stays on one thread, which keeps its order trivially fixed.
