# Review fixes

Decisions 0058 and 0060 are logs of review fixes, not decisions. Their numbers stay reserved in
`docs/decisions/README.md`.

## 0058 Review fixes of the control plane and the edge: what a failure now does

Built (2026-11-27), from a read-only review of the control plane and the serving edge.

- **A job that ended stays ended.** `claim` moves a job to `running` only if it is `queued` or
  `running` and reads the change count: a redelivered request for a job that succeeded, failed
  or was refused is `Dropped::Settled`, acked, and the job is left as it was.
- **Every failure after the claim is an attempt.** A failed deploy record or log write calls
  `give_up` like a builder failure: released for [`Control::RETRY`], then the job ends `failed`
  at `ATTEMPTS`. The ack follows the job's end, so a crash between the two is redelivered and
  dropped as settled. If `give_up` itself fails, that error is the one returned.
- **Rollback moves the pointer only from what it read.** The update adds `deploy = <read>` to
  its guard, so the event's `previous` is the deploy the pointer left; a deploy that moved it in
  between makes `rollback` read again. `deploy` (publish) still reads the pointer outside its
  batch for the event's `previous`; the pointer and the history row take it from the row, only
  the event can name a stale one. Not changed.
- **Smaller.** Blob partial names count per process, not per `Directory`. The `sql` rule flags
  `custom` as a method call or on `ColumnType` and `ColumnDef` (its doc lists what it still
  misses). `Steps::apply` refuses a ledger that is not a prefix of the set (`Unapplied::Order`).
  The sqlite adapter fails a value kind it does not know (`Unsupported::Kind`) instead of
  binding NULL. An unreadable `.git` refuses the build as `SourceRepositoryUnreadable` instead
  of falling back to plain files. `Error::Missing`, `Unreadable::Url` and `Unreadable::Revision`
  keep their source.
- **Edge.** `Limits::connections` now counts accepted connections: the accept loop takes a slot
  before it accepts, so slow heads cannot use every descriptor. What it counted before, requests
  in flight, is `Limits::requests`. `Limits::chunk` is gone: the folder adapter's chunk is its
  own constant, and no adapter read the field. `Files::open` opens the file once, checks the
  handle is the file it inspected and reads through it (a replaced name is `Error::Swapped`).
  A site folder that is a link is refused in `Folders` mode. A manifest is read through a
  `Deploys::MANIFEST_LIMIT` of 16 MiB (`Error::Large`). `dns()` checks label bytes. `ink serve`
  failures that are not the preview are `serve.run.failed`. A `Host` is lowered, not refused,
  when it is not lower case; 0051 said it was refused.

**Why:** each was a way for a failure to leave a job running forever, a store to lie, or a
budget to be a number nothing read. **Reversal:** none expected; the connection default of 1024
follows the common soft descriptor limit and is the number to change if it bites.

## 0060 Review fixes: keys, panics, escapes, atomic writes, held nones

Accepted. A read-only review of the image work, the engine, the model and the formats gave these
changes, each with the test that failed first; what was refused is listed last.

- **Image cache key.** `host_image::Key` hashes the media type, since the decoder follows it (the
  same bytes named `.png` and `.jpg` are two images), and `build.rs` hashes every registry
  package reached from the crate through the lock's dependency graph, not only its direct
  dependencies: `cargo update -p rav1e` changes AVIF bytes and now changes every key. The key is
  wider than the codecs (the image crate also reaches the engine's dependencies), which costs a
  cold cache on a bump of any of them and nothing else.
- **A panic refuses the file.** A decoder or encoder that panics on one hostile file is caught at
  the step (`Reason::Panicked(Step)`) and refuses that file with `image.decode.failed` or
  `image.encode.failed`; the rest of the build goes on. A thumbhash that draws nothing is
  `Reason::Placeholder`, not a silent missing placeholder. The settings' clamps stay (0044).
- **Ladder wording.** The code was right and the doc was not: a step within a tenth of the
  image's width is left out and the last copy is at the widest step or the image's own width,
  whichever is narrower. The nine and the ten are named. The animated-image notice now says the
  original is served with its metadata, as the AVIF one does.
- **Links.** A wikilink anchor with no letter or digit is `link.anchor.missing`, not dropped. A
  file name holding a colon (`meet: notes.md`) is looked up before the text is taken for a URL
  scheme. Aliases are kept as written, so the letter for letter pass is.
- **Atomic writes.** `ink select` and `ink pin` write every file to `.name.ink-new` beside it
  (created exclusively, synced, with the target's permissions) and rename each over its target
  only when all are written, so a failed write leaves every file as it was; the renames are the
  one step a failure can split.
- **Git tree walk.** The walk keeps its own stack, so nesting as deep as the folder budget allows
  cannot overflow the thread's stack. A link to nothing stays a refusal (0036, tested): the
  folder source refuses it too.
- **Frontmatter none.** `Front` holds `date: false`, `updated: false` and `description: false`
  as `Some(None)` (WIT: `option<tuple<option<..>, span>>`), so the page is decided undated, never
  updated or without a summary, with frontmatter evidence, as `ink.toml`'s `date = false` is.
- **Terminal text.** Control characters in a name or message reach no terminal: `diag::Plain`
  shows them as `\u{..}`, `Terminal` and every text sink of the CLI go through it, and `pins`'s
  `# not pinned` comment stays one line.
- **Change attribution.** A format plugin is blamed only for a decision some side of which was
  read from the file's text (frontmatter, heading, inline, body). A record holding several
  decisions of one code and subject pairs the nth with the nth. A `site.format` error is no
  longer discarded. An `include` or `exclude` with a bad item applies none of it, as the
  `Config` doc says.
- **Smaller.** A quote opens an HTML attribute value only after `=`; a NaN length has no stroke
  count; a non-finite scalar given to a `freehand-web` export draws nothing; the `sql` rule reads
  `r#cust` and a statement behind `\n` or `(`, and its module doc lists what it cannot see.

Refused: the CMYK fixture does hold an Adobe marker (transform 0), so its colour is the
convention's own; the SQL rule already finds a keyword in a raw string and checks every literal of
`concat!` and `format!`; the clamps of `avif_speed` and `jpeg_quality` stay as 0044 says. Not
built: the `image.*` keys of `ink.toml` and the operator ceilings, which 0044 names as its next
step.
