---
number: "0061"
title: "The `ink` CLI has one output grammar, one renderer and a grouped command tree"
status: accepted
date: 2026-11-26
area: cli
supersedes: []
---

## 0061 The `ink` CLI has one output grammar, one renderer and a grouped command tree

Accepted by the owner after reviewing `docs/cli.md` (with `docs/cli-preview.html`); every open choice takes the recommendation in that document.
The CLI is redesigned on the model of cargo (verb gutter, `error[code]` blocks, `-q`/`-v`/`--color`),
jj (colour as named roles, global options under their own heading), vun and baudelaire (one
declarative command tree that help, completions and the manual all read; help written by ink, not
clap). In short:

- **Grammar.** Verbs for what you do to the folder in front of you (`build`, `check`, `explain`),
  a noun group where a noun has three or more verbs (`deploy list|show|rollback|resume`). `-C
  <DIR>` is one global option; the positional `<DIR>` of `build`, `check` and `preview` goes. A
  decision is addressed `[PATH] CODE`: `ink pin posts/a.md page.title`, `ink explain posts/a.md
  --why page.title`. Help has titled command groups (Write, Understand, Publish, Tools).
- **Output.** Ten typed blocks (stage line, diagnostic, notice, error, tree, ladder, change set,
  table, record, result), each both rendered as text through the theme and serialized as JSON: the
  `Shown`/`Event` pair of `report.rs` becomes the rule. One module owns colour, width, glyphs and
  TTY handling; no command builds output with `format!`. Results are stdout in one shape; the rest
  is stderr.
- **Colour.** A closed set of roles mapped once to the 8 ANSI colours, bold and faint, with no
  background, bright, 256 or truecolour, so the person's terminal theme decides the hue. Meaning
  also sits in a word or glyph; `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb` and non-TTY
  are honoured per stream.
- **Errors.** Every failure shows `error[code]`, a `caused by` rail, a typed `fix` and a `state`
  line; `uses.error/v1` gains `fix` and `state`. Snippets stay on `annotate-snippets`, styled.
- **Kept.** Every schema (`uses.build/v2`, `uses.check/v1`, `uses.explain/v2`, `uses.trace/v1`,
  `uses.select/v1`, `uses.error/v1`, `uses.event/v1`), the exit codes 0 to 6, nothing silent. New:
  `-q`, `--color`, `--no-progress`, `INK_*` variables with one precedence, `uses.progress/v1`
  events, `ink completions` (nushell included), `ink man`, `ink help exit`.

Refines `research/flow.md` section 7 (names `deploys`, `rollback`, `resume` become `deploy ...`,
and the `--json-stream` flag is dropped in favour of events under `--json`) and the "CLI output"
rules of `conventions.md`; it supersedes no decision. **Why:** today three conventions name the
site's folder, the same two global flags repeat under every command, errors are plain strings
without a fix, and the M3 verbs would each add a fourth output style. **Reversal:** a renderer that
costs more than it clarifies, or a grammar the M3 verbs do not fit: keep the typed blocks and the
colour roles, and go back to clap's help and the per-command folder. The open choices are the
last section of `docs/cli.md`.

**Amendment, slice 1 and 3 (the output layer).** `cli/src/term` is built on `anstyle` and `anstream`
(already in the tree through clap) with `unicode-width` and `terminal_size`; the live line is a small
typed meter (`Meter`, driven by the `clock` port) instead of `indicatif`, which is not in the tree
and cannot target the CLI's `&mut dyn Write` streams. Nothing in `term` reads the process:
`Surroundings` is what the caller says the process is (`main` passes the real one, a capture passes
none), so goldens do not change with the machine. Fixes of errors are the CLI's own typed `Remedy`,
not the diag catalog's `Fix`, which words problems of a folder, not of a command. `State` is typed
(`unchanged`, `partial` with the path, `unknown`) and an error that wrote part of a file says so.
Progress is stage lines and, for image work, a count between batches the size of the machine's
parallelism (`images.rs`); the engine's one-call `Images::prepare` has no finer hook, so a page
count, bytes of a fetch and the per-image `Made`/`Reused` lines of `-vv` need an engine port and are
left. The `[ui]` table of `~/.config/ink/config.toml` (slice 2's `Settings`) feeds the same decision, below the environment variables of `term` only when it says `always` or `never`. Not done in this slice: `INK_TIMES`, prose wrapping, `... and N
more` cuts, hiding default decisions, and the table header (all need a flag or a verb of the
command tree).

Build note (slice 2, the command tree): `cli/src/tree` declares every command once as typed values
(group, aliases, wording, examples, exit codes, environment). It decorates the clap command the
argument types derive, and `help`, `completions` (bash, zsh, fish, elvish, nushell, powershell) and
`man` read it, so a command is in all four places or a test fails. Global `-C`, `-q`, `--color`
and `--no-progress` with `INK_*` variables and one precedence (flag, environment, `~/.config/ink/config.toml`,
default) are in. `ink deploy list|show|rollback|resume` talk to the edge API over plain
HTTP (there is no TLS in the tree, so `https://` is refused with a typed error), with exit codes 3,
4 and 5 for sign-in, network and conflict. `init`, `login`, `logout`, `whoami`, `push`, `status`,
`watch` and `export` are listed in help as not built yet and end with exit 2. Dynamic completion of
codes and paths (`CompleteEnv`) is not built.
