# Spike: gix clone, deepen and file dates

M0 spike 2 in `docs/roadmap.md`: "gix: shallow clone, deepen, per-file first and last commit
dates for 5,000 files under 5 s."

**Verdict: yes for the dates, no for the network.** Computing `created` and `updated` for every
file at HEAD from full history takes about 2 s on 4 threads for 40,944 files and 24,903 commits,
eight times the file count in the question. Getting that history over HTTPS takes longer than
5 s on its own: 7 s for a direct full clone of the test repo, 17 s for a depth-1 clone followed
by a full deepen. The 5 s budget holds for the work we control and cannot hold for the fetch.

## Method

- `gix` 0.91.0 (latest on crates.io on 2026-11-26), `default-features = false` plus
  `blocking-http-transport-reqwest-rust-tls`, `max-performance`, `sha1`, `blob-diff`. Bare
  clones, no checkout: the snapshot is the HEAD tree.
- Test repo: `https://github.com/tldr-pages/tldr`, HEAD `6a4e8b66b6b5`. 40,944 blobs at HEAD
  (markdown pages in about 40 languages), 24,903 commits, 817 of them merges, 74 MiB packed.
  `rust-lang/rust-by-example` is too small and `mdn/content` (525 MiB) was not worth the
  bandwidth for a spike.
- `spike-gix clone <url> <dir> [depth]`, `deepen <dir> <n|full>`, `dates <dir> [--threads N]
  [--first-parent]`, `all <url> <dir> <n|full|0>`. Release build, 14-core laptop, home fibre.
  The machine was shared with other builds (load average 4 to 10), so timings vary by about 50%.
- One walk over history, newest first, collects a job per commit: its tree, its parent's tree,
  its author time. Merge commits are skipped, root commits are diffed against the empty tree,
  shallow boundary commits are not diffed because their parents are missing. The jobs are diffed
  with `gix::diff::tree` (no rename tracking) on N threads, in blocks of 256 commits handed out
  through a counter. The hits are folded newest first: the first touch of a path is `updated`,
  the first addition is `created`.
- Correctness: 40 random paths checked against `git log --format=%at -- <path>` and
  `git log --diff-filter=A` on the same clone. 40 of 40 match.

## Numbers

Network, tldr:

| Step | Time |
|---|---|
| clone, depth 1 | 4.8 to 10.1 s (git CLI: 4.6 to 5.0 s) |
| deepen by 200 | 2.5 s |
| deepen by 1,000 | 3.3 s |
| deepen to full | 10.9 to 15.0 s |
| clone, full, no shallow step | 7.3 s |

Dates over full history (24,086 diffs), with a 32 MiB delta-base cache and no object cache per
thread:

| Threads | Dates | Peak RSS |
|---|---|---|
| 1 | 6.8 s | 160 MiB |
| 2 | 3.7 s | 200 MiB |
| 4 | 1.9 to 3.2 s | 275 MiB |
| 8 | 1.6 s | 420 MiB |
| 14 | 1.1 s | 620 MiB |

Listing the 40,944 HEAD files takes 10 to 60 ms; the walk that collects the jobs takes 0.2 s.
The diffs are the cost, and 50% of that is zlib inflating tree objects.

End to end on 4 threads:

| Run | Clone | Deepen | Dates | Total | Created exact / bound | Updated exact / bound |
|---|---|---|---|---|---|---|
| depth 1 | 10.1 s | none | 0.06 s | 10.2 s | 0 / 40,944 | 0 / 40,944 |
| depth 1, deepen 1,000 | 4.8 s | 3.3 s | 0.19 s | 8.4 s | 1,776 / 39,168 | 9,118 / 31,826 |
| depth 1, deepen full | 5.9 s | 10.9 s | 2.1 s | 18.9 s | 40,944 / 0 | 40,944 / 0 |

"Bound" is `NotAfter(boundary time)`: the file existed at the shallow boundary, so the true date
is at or before it. No file ended `Unknown`.

Cache sizes at 4 threads (`SPIKE_PACK_CACHE_MB`, `SPIKE_OBJ_CACHE_MB`):

| Delta-base cache | Object cache | Dates | Peak RSS |
|---|---|---|---|
| 0 | 0 | 3.8 s | 117 MiB |
| 0 | 32 MiB | 2.4 s | 275 MiB |
| 32 MiB | 0 | 1.9 s | 275 MiB |
| 96 MiB | 32 MiB | 2.6 s | 730 MiB |

The defaults gix picks without configuration (a 64-entry static delta cache, our first try at a
32 MiB object cache) ran the single-thread pass in 15 s instead of 6.6 s.

## What it takes

- Full history: clone in full directly. A depth-1 clone plus a full deepen costs 17 s against
  7 s, because the server builds a second pack against the shallow boundary.
- Bounded history: depth 1 then `Shallow::Deepen(n)` works and is cheap; the dates of the files
  outside the window come out as bounds, as `Known<T>` expects.
- A 4-thread builder with about 300 MiB is enough for a repo this size. Memory grows with
  threads (one delta cache per thread), not with files.
- Speedups not tried: skipping commits once every path has both dates (does not help with full
  history, since the oldest files need the root commits), commit-graph changed-path Bloom
  filters (a fresh clone has no commit-graph, and `gix-commitgraph` 0.43 neither writes one
  nor reads Bloom filters),
  restricting the diff to the source root.

## Gotchas

- **A reused `gix::diff::tree::Recorder` gives wrong results silently.** Clearing `records` is
  not enough; its internal path queue carries over, and the next diffs report almost nothing.
  The run was 5 times faster and 40,936 of 40,944 files came out unknown. Make one per diff.
- **A bare clone persists no fetch refspec**, same as git. A later `deepen` fails with "Cannot
  perform a meaningful fetch operation without any configured ref-specs" until the branch is
  named again (`+refs/heads/main:refs/heads/main`).
- **Shallow semantics:** a shallow boundary commit keeps its stored parents but the rev walk
  reports none, so it looks like a root. Diffing it against the empty tree would date every
  file to the boundary. Boundary commits must be looked up in `repo.shallow_commits()` and
  skipped. `Shallow::undo()` is `DepthAtRemote(i32::MAX)`, not a separate unshallow command.
  A shallow repo also disables the commit-graph in the walk.
- **Renames are not followed.** 732 of 40,944 files (1.8%) got their `created` date from a
  commit that moved the exact same blob from another path; with renames followed they would be
  older. Rename tracking (`tree_with_rewrites`) needs blob similarity for edited moves and was
  not measured. `UnknownReason` in `docs/architecture.md` has no `Renamed`; the research draft
  does.
- **Merges:** skipping merge commits gives the same answer as `git log -- <path>` on the
  sampled paths. `--first-parent` instead attributes a merged branch to the merge commit's time;
  on tldr every date stays exact, the move count goes from 732 to 730 and the speed is the
  same; dates were not compared one by one.
- **Dates are author time.** The walk sorts by committer time; the fold re-sorts by author time,
  because rebased history has the two disagree.
- **No partial clone.** `gix-protocol` can send a `filter` argument, but `gix` exposes no
  `filter=blob:none` on fetch and its object store has no promisor support. The blobless clone
  that `docs/research/architecture.md` suggests is not available through gix 0.91.
- gix network calls are `bisync`: with a blocking transport feature they are plain functions.
- `prepare_clone_bare(...).fetch_only(...)` needs `Discard` progress and an interrupt flag even
  when neither is wanted.

## Running

```
cargo build --release
./target/release/spike-gix all https://github.com/tldr-pages/tldr.git /some/dir/tldr full --threads 4
SPIKE_DUMP=1 ./target/release/spike-gix dates /some/dir/tldr   # one line per file
```

The numbers above were produced from a copy of this directory under `~/.cache` because the
scratch volume holding the jj workspace was over its disk quota; the source is identical.
