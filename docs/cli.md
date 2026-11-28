# The `ink` command line

Status: **Accepted** (decision 0061). This is a design, not a description of what ships. Where it
says "today", it means the CLI as it is on `main`. Where it says "built", it names a decision or a
slice of section 17. `cli-preview.html` shows every mock-up here in colour, light and dark; open it
in a browser. Mock-ups are illustrative: names, numbers and ids are made up, and every code and
every piece of wording is taken from the catalog (`crates/core/diag/catalog.toml`) or from `crates/surface/cli/src` unless the
text says it is new.

Contents: 1 Principles, 2 Command tree, 3 Global options, 4 Help, 5 Colour and glyphs, 6 Output
grammar, 7 Mock-ups of every output, 8 Progress, 9 Errors, 10 `--json`, 11 Exit codes,
12 Completions and the manual, 13 Configuration and environment, 14 Accessibility, 15 What breaks,
16 The renderer, 17 Build order, 18 Open choices.

## 1. Principles

These come from the owner's brief, from `conventions.md` ("CLI output") and from reading the CLIs
named below. Each one is a rule a reviewer can check.

1. **Calm, plain, warm.** The product is a quiet publishing tool. Messages are sentences about the
   writer's folder, not stack traces about ink. Say "nothing was changed" when nothing was. No
   shouting, no exclamation marks, no emoji, no jokes. The voice of `crates/core/diag/catalog.toml` is the
   voice of the CLI.
2. **A result is data, everything else is talk.** The result goes to stdout and keeps one shape
   whether stdout is a terminal or a pipe. Progress, diagnostics, notices, change sets and
   hints go to stderr. `ID=$(ink build)` works and prints the deploy id and nothing else.
3. **One typed model per output, rendered once.** Every kind of output is a typed value that can
   be rendered as text through the theme and serialized as JSON. No command builds output with
   `format!`. Colour, width, glyphs and TTY handling live in one renderer (section 16).
4. **Nothing is silent.** A default that was chosen, a folder that was skipped, a cache that was
   cold: each is a notice, shown with `-v` and present in `--json` events. Nothing that cannot be
   undone is hidden by `-q` (section 3).
5. **Every failure has a code, a cause and a fix.** A usage error from the argument parser is
   rendered by the same code, not clap's.
6. **Colour never carries meaning alone.** Every colour sits beside a word or a glyph, so the
   output reads the same with `NO_COLOR`, on a printer, and to a screen reader.
7. **Structure is shown by nesting.** Groups in help, trees for decisions and causes, a rail for a
   ladder, aligned columns for lists. Indent two spaces per level. Never box a whole screen.
8. **No magic.** Any flag has an environment variable and a config key under one naming rule, with
   one precedence (section 13). Nothing depends on the time of day or on which terminal it is.

### What the references taught

| Source | Taken | Left |
|---|---|---|
| **cargo** | Right-aligned past-tense verb gutter (`Compiling`, `Finished`); `error[code]` blocks with a source snippet; `-q`, `-v`, `--color`; short `-h`, long `--help`; aliases `b`, `c` | Everything-on-stderr (cargo has no stdout result); free-form `note:` and `help:` footers |
| **jj** | Colour as named labels mapped once to styles (`colors.toml`), not colours at call sites; bold-only emphasis; global options under their own heading; hints written as the command to type; `-R` as the one place that says which repository | Revset and template languages; a graph in every listing |
| **uv** | Help sections for options (`Cache options`, `Global options`); environment variable shown beside each flag; `--no-progress`; dim for secondary text; quiet levels `-q`/`-qq` | Its long global-options block repeated everywhere |
| **vun** | One declarative command tree (`Command::group`, `Command::run`) that help, completions and the manual all read; examples as data (`Example { line, about }`); groups for rarely used commands | A tree in the Rust code instead of derive |
| **baudelaire** | A `Table` for `Examples`, `Exit codes` and `Environment` blocks appended under clap's own help, padded on unstyled width; `Heading` and `Literal` as the only two help styles; a `Marker` enum for glyphs; `--color` installed before clap writes help; `man` and `completions` commands | Its cyan and green literals (ink uses cyan for anything you can type, see section 5); progress bars drawn with a 256-colour track |
| **gh** | Titled command groups (`CORE COMMANDS`, `ADDITIONAL COMMANDS`); TTY checks per stream; an accessible mode | Noun-verb for everything, and upper-case group titles |
| **nextest** | Bold coloured status words in a fixed-width column; a summary line that repeats the verdict | Its dense per-test lines |
| **clig.dev** | Disable colour for non-TTY, `NO_COLOR`, `TERM=dumb`, per stream; no animation off a TTY; suggest the fix; keep stdout for data | Its optional pager advice |

## 2. Command tree

### The rule

**Verbs for what you do to the folder in front of you; a noun group when a noun has three or more
verbs or is not the folder.** That is the cargo and jj model (`ink build`, as `cargo build` and
`jj log`) with gh's nouns only where they earn their place (`ink deploy rollback`, as `jj bookmark
set` or `gh pr merge`). A group has no behaviour of its own: `ink deploy` prints its help.

Rejected: noun-verb everywhere (`ink site build`) puts a word before every daily command and
`ink` has one site per folder; verb-noun everywhere (`ink rollback deploy`) reads well but does
not group, so help cannot list "everything about deploys" in one place.

### The tree

```
ink
├─ Write
│  ├─ init                   link this folder to a site
│  ├─ check, c               read the folder, report diagnostics, write nothing
│  ├─ build, b               build into an output folder, print the deploy id
│  ├─ preview                build in memory, serve on 127.0.0.1
│  └─ serve                  serve a built deploy as production does
├─ Understand
│  ├─ explain [PATH]         the decisions behind the site or one file
│  │    └─ --why CODE        replay one decision's ladder
│  ├─ pin [PATH] CODE        write one decision's value into ink.toml
│  └─ select [PATH...]       list or choose what is published
├─ Publish
│  ├─ login, logout, whoami
│  ├─ key                    a group: list, add, revoke
│  ├─ abuse                  the operator's group: list reports, suspend, reinstate
│  ├─ domain                 a group: add, list, prove, remove custom domains
│  ├─ push                   send the folder, start a deploy
│  ├─ status                 what is live, what is building
│  ├─ watch [BUILD]          follow a build to its end
│  ├─ deploy                 a group
│  │    ├─ list              history, newest first       (alias: ink deploys)
│  │    ├─ show <DEPLOY>     log, diagnostics, decision changes
│  │    ├─ rollback <DEPLOY> serve an earlier deploy, pause new ones
│  │    └─ resume            deploy new pushes again
│  └─ export [DIR]           the site, or the last push's files, to a folder
└─ Tools
   ├─ completions <SHELL>
   ├─ man
   └─ help [COMMAND | exit | CODE]
```

The seven commands that exist today keep their names. `deploys`, `rollback` and `resume` of
`research/flow.md` section 7 become `deploy list`, `deploy rollback` and `deploy resume`; `ink
deploys` stays as a hidden alias because it is what that document already tells people to type.

### Naming

- Commands are lower-case English verbs of one word. Groups are singular nouns. No hyphenated
  command names.
- A flag is one or two lower-case words with hyphens. A flag that takes a value names it in
  upper case (`--out <DIR>`).
- Aliases are only `b` and `c`, for the two commands typed most. They are shown in help.
- Diagnostic and decision codes are `noun.noun.noun` as the catalog has them.

### The folder is a global option, not an argument

Today `build`, `check` and `preview` take a positional `<DIR>`, `explain`, `pin` and `select` take
`-C, --dir` defaulting to `.`, and `serve` defaults to `.ink/out` relative to the working
directory. Three conventions for one idea. The design makes it one: **`-C <DIR>` is global,
defaults to `.`, and every command reads "the site's folder" from it.** It is cargo's `-C` and
git's. `ink serve` defaults its folder to `<site>/.ink/out`, so `ink -C garden serve` works. The
positional `<DIR>` of `build`, `check` and `preview` goes away (section 15).

### One shape for naming a decision

A decision is addressed as `[PATH] CODE`: a file's path and a code, or a site code alone. `ink pin`
takes both as positionals, as it does today. `ink explain` already uses its first positional for a
path, so the code rides on `--why`: `ink explain posts/a.md --why page.title`. Both read left to
right as "this file, this decision". `--why` takes exactly one value, the code; the path is the
positional it shares with plain `explain`. (Today `--why` takes one or two values; open choice 3.)
A code and a path never need telling apart by guessing: a path is whatever is not after `--why`
and not the last word of `pin`.

### Reading and writing

Every command that reads a folder shares two options, shown under `Source` in its help:
`--source <folder|git>` and `--rev <REV>`. They are not global because `login` and `deploy` have
no source. Commands that write something say so in their first help line and have `--dry-run`
(`pin` and `select` today).

## 3. Global options

Global options sit under one heading, `Global options:`, at the end of every help page, and are
accepted before or after the command (`ink -q build` and `ink build -q` are the same).

| Option | Meaning | Environment | Today |
|---|---|---|---|
| `-C, --dir <DIR>` | The site's folder, default `.` | `INK_DIR` | per command |
| `--json` | One JSON document on stdout; diagnostics, notices and progress as NDJSON on stderr | | yes |
| `-q, --quiet` (counted) | `-q` drops live progress, stage lines and notices; `-qq` leaves only errors and the exit code | | new |
| `-v, --verbose` (counted) | `-v` adds notices; `-vv` adds one line per file and timings | | `-v` only |
| `--color <WHEN>` | `auto`, `always`, `never` | `INK_COLOR` | new |
| `--no-progress` | Log lines only, no live redraw | `INK_NO_PROGRESS` | new |
| `-h`, `--help` | `-h` short, `--help` long | | yes |
| `-V`, `--version` | Version | | yes |

`--width <COLUMNS>` is not in the list. Width is read from the terminal, then `COLUMNS`, then
`INK_WIDTH`; tests and screenshots set `INK_WIDTH`. A flag for it would be a global option nobody
types (open choice 12 asks whether to add one).

### Verbosity levels

| Level | Shows |
|---|---|
| `-qq` | Errors, and the exit code |
| `-q` | The above, warnings, the result on stdout, and **the newly-public list of a change set** |
| default | The above, stage lines, the summary, the whole change set, live progress on a terminal |
| `-v` | The above, notices (`note[code]`) |
| `-vv` | The above, one line per file read, per image made, per page rendered, timings |

`-q` keeps the newly-public list because it is the one thing in a build that cannot be undone
(decision 0042: a page made public is out before anyone reads the log). The log is separate
(`docs/tracing.md`, decision 0065): `-v` keeps the meaning above for output, and its count is also
the weakest filter of the log (`-vv` is `info`, `-vvv` is `debug`). `INK_LOG` and `--log
<FILTER>` refine it per area, `--log-file <PATH>` writes JSON lines, and under `--json` the log
goes to a file only. `-q` does not silence a log asked for by name.

## 4. Help

Help is rendered by ink, not by clap. Clap parses; ink writes the help from the same command tree,
through the one renderer, the way baudelaire's `help.rs` does and vun's `Program` does.

### Layout

In order, each present only when it has content:

1. **About**: one sentence, then, for a command whose point a name does not give, a short
   paragraph (at most four lines, the guarantee it makes).
2. **Usage:** one line.
3. **Groups of commands** (root), or **Commands** (a group), or **option groups** (a leaf): a
   heading, then two aligned columns. Headings are bold. Names you can type are cyan bold.
4. **Global options:** always last of the options; in a leaf, short form by name only when the
   list would repeat more than six lines.
5. **Examples:** a two-column table, the line to type and what it does.
6. **Exit codes:** only the codes that command can end with.
7. **Environment:** only on `ink --help` and where the command reads variables of its own.

Short help (`-h`) stops after the options. Long help (`--help`) adds the paragraph, examples, exit
codes and environment. Column rule: the first column is as wide as its longest entry plus two,
measured on unstyled text; descriptions wrap at the terminal width, at most 100, with a hanging
indent. Flags that take a value show the default in brackets at the end of the description, in
the one form: `[default: .ink/out]`.

### `ink --help`

```text
Turn a folder of writing into a site, and publish it.

Usage: ink [OPTIONS] <COMMAND>

Write
  init         Link this folder to a site, without publishing it
  check, c     Read the folder and report what is wrong; writes nothing
  build, b     Build the folder into an output folder and print its deploy id
  preview      Build in memory and serve it on 127.0.0.1 while you write
  serve        Serve a built deploy the way production does

Understand
  explain      Show the decisions behind the site, or behind one file
  pin          Write one decision's value into ink.toml
  select       List what is published and why, or choose what to publish

Publish
  login        Sign in to uses.ink
  logout       Sign out
  whoami       Show who you are signed in as
  push         Send the folder to its site and start a deploy
  status       Show what is live and what is building
  watch        Wait for the next deploy of this commit and report it
  deploy       List, read, roll back and resume deploys
  export       Write the whole site, source and history, to a folder

Tools
  completions  Print a completion script for your shell
  man          Print the manual page
  help         Show this message, or the help of one command

Global options:
  -C, --dir <DIR>     The site's folder [default: .]
      --json          One JSON document on stdout, diagnostics as NDJSON on stderr
  -q, --quiet...      Say less: -q drops progress, -qq leaves only errors
  -v, --verbose...    Say more: -v adds notices, -vv adds every file
      --color <WHEN>  auto, always or never [default: auto]
      --no-progress   Print log lines only, no live redraw
  -h, --help          Print help (-h is short, --help is long)
  -V, --version       Print the version

Examples:
  ink check                 Read this folder and list its problems
  ink build -o public       Build into ./public
  ink explain posts/a.md    Why one file looks the way it does
  ink select notes/trip.md  Publish one page of a vault

See ink help <command> for one command, and ink help exit for exit codes.
```

### A group: `ink deploy --help`

```text
List, read, roll back and resume the deploys of a site.

Usage: ink deploy [OPTIONS] <COMMAND>

Commands
  list      History, newest first (also: ink deploys)
  show      One deploy: its log, its diagnostics and what its decisions changed
  rollback  Make an earlier deploy the live one, and pause new deploys
  resume    Let the site deploy new pushes again after a rollback

Examples:
  ink deploy list --limit 5         The five newest deploys
  ink deploy show 5kq3v9m2tj7r      What that deploy built, and why it differs
  ink deploy rollback 4mq8c1d0wz5e  Serve the earlier deploy again

Global options: (as in `ink --help`)
Exit codes: (see `ink help exit`)
```

### A leaf: `ink build --help`

Option groups replace clap's single `Options` list: `Output`, `Source`, `Work`, `Verification`.
The group names are headings, so they are written once in the command tree and reused by help,
the manual and completions.

```text
Build the folder into an output folder and print its deploy id.

The folder is read, interpreted and rendered in full before the output folder is
touched. A build that fails, or warns under --strict, writes nothing: the previous
build stays as it was.

Usage: ink build [OPTIONS]

Output
  -o, --out <DIR>         Where to write the site [default: .ink/out]
      --base <URL>        The address the site is served at, for feeds and canonical links
      --strict            Exit 6 on warnings, and write nothing

Source
      --source <KIND>     folder reads the files on disk, git reads a commit
                          [default: git when .git exists, said as a notice]
      --rev <REV>         The commit to read, as `git rev-parse` takes it [default: HEAD]

Work
      --cache <DIR>       Where to keep work between builds [default: .ink/cache]

Verification
      --check             Build natively and through the wasm components; fail if they differ
      --components <DIR>  Components that replace the bundled ones (with --check)

Examples:
  ink build                      Build this folder into .ink/out
  ink build --strict             The same, but a warning stops the build
  ink -C garden build -o public  Build another folder into ./public

Exit codes:
  0  Built; warnings, if any, were printed
  1  The site has errors; nothing was written
  2  Wrong usage, or an output folder ink will not write into; nothing was changed
  6  --strict was given and there were warnings; nothing was written

Global options: -C, --json, -q, -v, --color, --no-progress (see `ink --help`)
```

`ink help exit` prints the exit-code table of section 11 and `ink help environment` prints the
environment table of section 13. Both exist so a table that applies to every command is written
once and found by name.

## 5. Colour and glyphs

### Roles, not colours

Code asks for a **role** (`Role::Good`), never a colour. The theme maps each role to one style.
This is jj's labels model, and it is why a `NO_COLOR` run, an HTML preview and a future light
theme are one change in one table. The set of roles is closed: adding one is a design change.

| Role | Style (16-colour) | Used for |
|---|---|---|
| `Heading` | bold | Section headings, the subject of a tree (a file path), the first line of a diagnostic |
| `Literal` | cyan, bold | Anything you can type: commands, flags, a `fix:` label and the command inside it |
| `Good` | green | Finished stage verbs, `+`, published, live, `✓` |
| `Warn` | yellow | Warnings, `~`, a change ink itself caused, a count of warnings |
| `Bad` | red | Errors, `-`, failed, `✗`, a count of errors |
| `Work` | cyan | Something in progress: a running stage, a building deploy, a spinner |
| `Id` | magenta | Things named to be found again: deploy ids, commit ids, pins |
| `Place` | blue | Where something is: `--> file:line:col`, the snippet gutter |
| `Code` | blue | A catalog code in a list (`page.title`) |
| `Quiet` | dim (SGR 2) | Evidence, units, tree rails, table headers, the second half of a line |
| `Plain` | default | Everything else |

"Strong" is a modifier, not a role: error and warning labels and the final `Built` are strong
(bold) in their role's colour.

One rule reads the table: **green is good news, yellow is look at this, red is stop, cyan is
something you can do or something running, magenta is a name to keep, blue is a place, dim is
the second line of the story.** Cyan for "what you type" differs from baudelaire (green) so that
green means one thing.

### The palette is the terminal's

ink asks for the eight normal ANSI colours and bold and faint. It asks for no bright colours, no
256-colour and no truecolour. That is what makes it work in dark, light and 16-colour terminals:
the person's own theme decides the hue, and that theme is already readable against its own
background. Three rules follow, each from a known failure:

- **Secondary text is faint (SGR 2), not "bright black".** Bright black is the background colour
  in Solarized Dark and is invisible there. jj avoids bright yellow for a similar reason
  (jj issue 528).
- **Bold carries emphasis where colour would be wrong**: headings are bold with no colour, so they
  work on any theme and in a monochrome log.
- **No colour is used as a background**, except by the person's selection.

The brand's own ink colours (`crates/core/token/token.toml`: pen red `#e8170b`, pencil `#5c5c5c`) live in the
HTML preview only, as the nearest readable stand-ins for the ANSI set. The CLI does not paint the
brand: a terminal is not a page.

### Glyphs

Two sets, chosen once: `unicode` (default) and `ascii` (`INK_GLYPHS=ascii`, `TERM=dumb`,
`TERM=linux`, or a locale that is not UTF-8). Glyphs are UI, so box drawing and the from-to mark are fine; prose
never uses arrows.

| Meaning | unicode | ascii | Role |
|---|---|---|---|
| Tree branch, last branch | `├─` `╰─` | `\|-` `` `- `` | Quiet |
| Rail (ladder, caused by) | `│` `╭─` | `\|` `.-` | Quiet |
| Live, finished ok | `●` `✓` | `*` `ok` | Good |
| Building, running | `◐` | `~` | Work |
| Failed | `✗` | `x` | Bad |
| Superseded, nothing, skipped | `·` | `.` | Quiet |
| Private (not published) | `○` | `o` | Quiet |
| Outranked rung | `–` | `-` | Quiet |
| Added, removed, changed | `+` `-` `~` | `+` `-` `~` | Good, Bad, Warn |
| From, to (value changes) | `→` | `->` | Plain |
| Progress bar | `━╸─` | `#>-` | Work |
| Truncation | `…` | `...` | Quiet |

### Why not a prefix glyph on every line

baudelaire marks every line with a glyph (`◆`, `✓`, `├─`). In a build that prints five lines that is
loud, and the glyph and the verb say the same thing twice. ink uses a **verb gutter** (cargo's) for
stages, where the word is the information, and glyphs only where there is no word: tree rails,
list state, the ladder.

## 6. Output grammar

Everything ink prints is one of ten **blocks**. A block is a typed value. It renders to text through
the theme and serializes to JSON through serde: the same value, two forms. `report.rs` already does
this for diagnostics (`Shown` is the text form and `Event` the JSON form of one `Diagnostic`); the
design makes that the rule for every block and names the pair.

| Block | Shape | Stream | JSON form |
|---|---|---|---|
| **Stage line** | Right-aligned past-tense verb (gutter 10, two spaces of margin), then a sentence | stderr | `uses.progress/v1` event (new) |
| **Diagnostic** | `error[code]: wording`, then location and snippet, then `fix:`, then `caused by` | stderr | `uses.event/v1` (built) |
| **Notice** | `note[code]: wording`, then `fix:` when there is one | stderr, `-v` | `uses.event/v1` (built) |
| **Error** | A diagnostic for a command that stopped: code, wording, cause chain, fix, state | stderr | `uses.error/v1` on stdout (built) |
| **Tree** | A subject, then rows joined by `├─` `╰─`; columns inside rows | stdout | the command's document |
| **Ladder** | A rail with numbered rungs and a mark for each | stdout | `uses.trace/v1` (built) |
| **Change set** | Headed sections (`newly public` first), each a column list, then a tree of decision changes | stderr | `changes` in `uses.build/v2` (built) |
| **Table** | Header (dim), columns, a state glyph column; footer count | stdout | the command's document |
| **Record** | Keys (dim) aligned, then values | stdout | the command's document |
| **Result** | One bare line, such as a deploy id | stdout | the command's document |

### Rules every block obeys

- **Columns align on unstyled text.** Padding is computed from the text without escapes, and
  glyphs count as one column. The table is the only place alignment is done.
- **A flexible column absorbs width.** The widest column of a table or tree (a path, an evidence
  sentence) is the one that is cut, with `…` in its middle for paths and its end for prose, and
  only on a terminal. Piped output and `INK_WIDTH=0` never cut.
- **Counts follow the thing counted**, in parentheses, in the role of the heading: `newly public (2)`.
- **A list that is cut says so**: `... and 33 more; --all lists them`. Never silently.
- **Headers appear on a terminal only.** A table on a pipe has no header and one row per line, so
  `ink deploy list | grep failed` works. A machine reads `--json`.
- **Times are UTC, ISO 8601 without seconds** (`2026-11-27 14:03Z`), as `flow.md` section 7 says;
  There is no local-time option yet. No relative times ("2 hours ago"): they change between two
  runs of the same command (open choice 9).
- **Paths are written as the person gave them**, relative to the folder, `/`-separated. Routes
  start with `/`. Ids are written whole.
- **Prose wraps at the terminal width, at most 100; code and snippets never wrap.**

### Streams

stdout carries the result and nothing else. stderr carries every block that is not the result.
`ink build` prints its stage lines, diagnostics and change set on stderr and the deploy id on
stdout. `ink explain`, `select`, `status` and `deploy list` have no result line: the document is
their result, so it is on stdout. (Today `ink build` writes the change set to stderr as an
"aside" and `ink explain` writes its tree to stdout. That is kept.)

## 7. Mock-ups of every output

All blocks below are plain text. Roles are in `cli-preview.html`. The stage gutter is 10 wide plus
a margin of 2.

### `ink build`: success

The stage lines and the change-set heading are on stderr, the last line is stdout. On a terminal
the stage lines are drawn live (section 8) and these are what remains.

```text
        Read  214 files from git 9f3c2a1b4d7e
  Understood  61 pages, 312 decisions
    Rendered  61 pages, 118 images in 1.4 s (97 reused, 21 made)
     Changes  first build in .ink/out
       Built  187 files in .ink/out, 1.9 s
5kq3v9m2tj7r
```

### `ink build`: with a warning and a change set

A diagnostic prints where it is found, between stage lines, with a blank line before and after.
`newly public` heads the change set, and is the part `-q` keeps. The full change set is written
when it is more than a screen; the mock cuts it.

```text
        Read  214 files from git 9f3c2a1b4d7e
  Understood  61 pages, 312 decisions

warning[link.target.ambiguous]: `[[trip]]` could be any of notes/trip.md, archive/trip.md
  --> journal/2026-01-04.md:9:12
   |
 9 | We left on [[trip]] day, early.
   |            ^^^^^^^^
   |
  fix: write the path of the one you mean

    Rendered  61 pages, 118 images in 1.4 s (97 reused, 21 made)
     Changes  2 newly public, 1 no longer public, 1 moved, 5 decisions changed

newly public (2)
  + notes/trip.md        /notes/trip/
  + notes/trip/day-1.md  /notes/trip/day-1/
  ... and 3 more; the full change set is in `ink build --json`

       Built  187 files in .ink/out, 1.9 s, 1 warning (ink check lists it again)
5kq3v9m2tj7r
```

### `ink check`: diagnostics

Errors come before warnings in each file, files in the order the folder lists them. A diagnostic
that has a span shows its line through `annotate-snippets`, styled by the theme; one that has none
(a collision across files) shows its fix directly. The closing line takes the colour of the
worst thing it counted.

```text
error[page.route.collision]: notes/a.md, notes/a/index.md and notes/a/README.md all route to /notes/a/
  fix: pin a different `page.slug` on all but one

warning[frontmatter.value.invalid]: the frontmatter's `date` takes a date, so it is ignored
  --> posts/b.md:3:7
   |
 3 | date: sometime in May
   |       ^^^^^^^^^^^^^^^
   |
  fix: change the value at the marked place, or remove the key

     Checked  61 pages, 312 decisions: 1 error, 2 warnings
```

A clean run:

```text
     Checked  61 pages, 312 decisions: no errors, no warnings
```

A notice (shown with `-v`; one line of wording, `fix:` when there is one):

```text
note[source.git.chosen]: the folder is a git repository, so the site is read from its commit 3f9a2c1e, not from the files on disk
  fix: pass `--source folder` to read the files as they are, uncommitted changes included
```

### An error with a cause chain and a fix

A command that stops prints one error block, then exits 2 (or 1 for a content refusal). Four
parts, always in this order: the line (`error[code]: wording`), `caused by` with a rail down the
causes outermost first, `fix:`, and a **state line** saying what the run left behind. The state
line is typed: `nothing was changed`, `the output folder is partly written: build again`, or, for a
command that stopped after the site queued its build, `build X is queued on the site and keeps
going`.

```text
error[output.manifest.invalid]: the earlier build's manifest .ink/out/ink.json does not parse
  caused by
  ╰─ expected `,` or `}` at line 1 column 8210
  fix: build into an empty folder with -o, or remove .ink/out yourself
  nothing was changed
```

Two causes:

```text
error[serve.edge.invalid]: the edge could not be built
  caused by
  ├─ the host limits do not describe a server
  ╰─ the request budget (0 B) is below the response budget minimum (4 KiB)
  fix: raise the budget, or leave it out to use the default
  nothing was changed
```

A mistyped code gets the closest ones, from the catalog:

```text
error[decision.code.unknown]: no decision has the code `page.titel`
  caused by
  ╰─ the closest codes are page.title and page.tags
  fix: ink explain lists every decision of the site with its code
```

A usage error, rendered by ink (clap parses, ink words it):

```text
error[cli.usage]: unexpected argument `--wy`
  fix: a similar argument exists: --why

Usage: ink explain [OPTIONS] [PATH]

For more information, try ink explain --help.
```

### `ink explain`: a site

A tree per subject. The source comes first because everything below depends on what it had. Each
row is the catalog code, the value in that code's words and the evidence, which is dim unless the
site's own pin chose the value (magenta, so a pin is seen at a glance). `--all` lists decisions
made by default; without it, hiding them is 0042's remaining step and is shown here as designed.

```text
garden  the files on disk, 214 files, 61 pages

the source
├─ version  the files on disk
├─ has      snapshot, watch
╰─ lacks    history, so page.date is not taken from it

site
├─ site.title    Quiet garden         from the first heading of README.md, line 1
├─ site.format   ink:format-obsidian  because .obsidian/app.json is there
╰─ site.home     README.md            by default

posts/a.md
├─ page.publish  published            by default
├─ page.title    A walk in the rain   from the first heading of posts/a.md, line 1
├─ page.date     2026-01-04           from the file's modification time
╰─ page.slug     walk                 pinned in ink.toml, line 14

58 more files. ink explain <path> shows one; --all lists the decisions made by default.
```

### `ink explain posts/a.md`: one file

```text
posts/a.md  a page, one of 61
├─ page.publish  published           by default
├─ page.title    A walk in the rain  from the first heading of posts/a.md, line 1
├─ page.date     2026-01-04          from the file's modification time
╰─ page.slug     walk                pinned in ink.toml, line 14

ink explain --why posts/a.md page.date    replays one decision, rung by rung
```

### `ink explain --why`: the ladder

One decision, replayed. The rail runs down the rungs in order of authority. A mark in the first
column says what each rung did: `·` found nothing, `✓` was chosen, `–` was outranked (a bound
that never wins is worded `bound`). The last line is the command that writes the value down.

```text
page.title on posts/a.md
╭─ A walk in the rain, from the first heading of posts/a.md, line 1: rung 3 of 5
│
│ · 1  pin          nothing
│ · 2  frontmatter  nothing
│ ✓ 3  heading      A walk in the rain  chosen
│ – 4  filename     walk                outranked
│ – 5  default      Untitled            outranked
╰─ to pin it: ink pin posts/a.md page.title
```

### `ink select`

```text
garden  a vault: a page is private until it is selected

published (3 of 38)
  ● notes/trip.md          publish: true in the frontmatter, line 2
  ● notes/trip/day-1.md    matched by `notes/trip/**` in ink.toml, line 7
  ● README.md              pinned in ink.toml, line 4

private (35)
  ○ journal/2026-01-04.md  nothing selected it
  ○ journal/2026-01-05.md  nothing selected it
    ... and 33 more; --all lists them
```

After a write:

```text
    Selected  1 page
  + notes/trip.md  publish: true added to the frontmatter

Nothing is public until you build and push: ink build shows it as newly public.
```

### `ink pin`

```text
      Pinned  page.slug of posts/a.md in ink.toml, line 14
  @@ ink.toml
   [pages."posts/a.md"]
  +page.slug = "walk"
```

With `--dry-run`:

```text
   Would pin  page.slug of posts/a.md; ink.toml is unchanged
  @@ ink.toml
   [pages."posts/a.md"]
  +page.slug = "walk"
```

### The change set

Newly public first, then no longer public, then moved, then decision changes grouped by subject
with a cause for each. The cause is one of the six typed ones of 0042 (`content`, `config`, `rules`,
`plugin`, `ability`, `engine`); `engine` is yellow because ink caused the change and it is a
warning locally.

```text
Changes  against the previous build in .ink/out, deploy 4mq8c1d0wz5e

newly public (2)
  + notes/trip.md        /notes/trip/
  + notes/trip/day-1.md  /notes/trip/day-1/

no longer public (1)
  - drafts/idea.md       /drafts/idea/    the address is no longer served

moved (1)
  ~ posts/a.md           /a/ → /posts/a/  the old address is no longer served

decisions changed (5)
posts/b.md
├─ page.title    Notes from the river → River notes  content
╰─ page.slug     notes-from-the-river → river-notes  content
notes/trip.md
╰─ page.publish  private → published                 config, ink.toml line 7
posts/c.md
╰─ page.date     2026-01-04 → 2026-01-05             engine: a change ink caused

2 newly public, 1 no longer public, 1 moved, 5 decisions changed
```

The same when nothing changed:

```text
Changes  against the previous build in .ink/out, deploy 4mq8c1d0wz5e
no change
```

### `ink init`, `ink push`, `ink watch` and `ink export`

Built (decision 0077). The result is on stdout, the stages on stderr.

```text
$ ink init --handle garden
  site     q3x9m2tj7r5kw8va
  handle   garden
  api      https://api.uses.ink
  file     .uses/site
  result   made, and linked to this folder

$ ink push
      Read  214 files, 3.1 MB
      Sent  12 files
    Queued  build 7a1f09c2e4b8, 12 sent, 202 already held
  Followed  build 7a1f09c2e4b8 is running
     Built  deploy 5kq3v9m2tj7r is live: 61 files, 2.4 MB
5kq3v9m2tj7r...  (the full deploy id, alone on stdout)

$ ink export out
  Exported  61 files, 2.4 MB of site into out
```

`push` stops with the fix `ink deploy resume` when the deploy is recorded and not live (exit 5), with
the first problem of the build's log when the site does not build (exit 1), and with `the site holds
N files of this push: run it again` when the upload was cut off. `watch` prints the same stages and
the same stdout. A folder with no `.uses/site` is exit 2 with the fix `ink init`.

### `ink status` and `ink deploy list`

`status` is a record; `deploy list` is a table. Both are results, so both are on stdout.

```text
garden  https://garden.uses.ink

  site     q3x9m2tj7r5kw8va
  state    active
  created  2026-11-02 09:00Z
  source   upload: ink push sends the folder
  health   building
  live     4d4d4d4d4d4d  since 2026-11-20 10:30Z
  latest   7a1f  running  upload, requested 2026-11-27 08:00Z
  pending  7a1f  running  upload, requested 2026-11-27 08:00Z
  domains  not listed here: `ink domain list` shows them
```

What is built today is the block above (`uses.status/v1`); the warnings row and a stage in the build
line wait for the server to keep a build's stages while it runs.

```text
  deploy        when               source            pages  state
◐ 7tw2c9d4h8mn  2026-11-27 14:21Z  git b81e04d2a7c9      -  building
● 5kq3v9m2tj7r  2026-11-27 14:03Z  git 9f3c2a1b4d7e     61  live
✗ 3mj0w2n8c5ab  2026-11-26 09:40Z  git 71de0c4b9a3f      -  failed: 1 error
· 4mq8c1d0wz5e  2026-11-25 18:12Z  git e2a7f60c1d58     58  superseded

4 of 31 deploys; --limit shows more
```

## 8. Progress

Long operations are image work, a fetch and a build. Three rules:

1. **Progress is stderr, and only on a terminal.** On a pipe, with `--no-progress`, with
   `TERM=dumb`, or under `-q`, there is no redraw: the same stage lines print once, when each
   stage ends.
2. **No flash.** A live bar appears after 0.5 s of work. A fast build prints its finished lines and
   nothing else, so a one-page site does not flicker.
3. **A stage has one shape.** `Reading`, `Understanding`, `Rendering`, `Writing` are typed stages
   (`Stage`), shared by the live view, the log and the `uses.progress/v1` event. A count of
   units (`41/118 images`) and the current item are the only variable parts.

On a terminal a running stage is one line, redrawn in place, with the bar in `Work`; a stage that
ends becomes its finished line in `Good`. Several workers in image work do not get several lines:
the line shows the count, and the current item is the most recently started one.

```text
(each redraw replaces the last line; the finished lines stay)
        Read  214 files from git 9f3c2a1b4d7e
  Understood  61 pages, 312 decisions
   Rendering  ━━━━━━━━━━╸─────────────  41/118 images  photos/trip-04.jpg
```

In a log (a pipe, CI, `--no-progress`) a stage that takes more than five seconds prints a
heartbeat line every five seconds, so a log never looks hung, and then its finished line:

```text
        Read  214 files from git 9f3c2a1b4d7e
  Understood  61 pages, 312 decisions
   Rendering  118 images, 41 done, 6 s
   Rendering  118 images, 97 done, 11 s
    Rendered  61 pages, 118 images in 13.4 s (97 reused, 21 made)

```

Under `--json` the same moments are NDJSON events on stderr: `{"schema":"uses.progress/v1",
"stage":"rendering","done":41,"total":118}`, at most two per second. The fetch of a remote source
uses the same shape with `stage: "fetching"` and byte counts when known.

`-vv` adds one `Made` or `Reused` line per image and a `Rendered` line per page, so the live bar
is the quiet view of the same events.

## 9. Errors

Four layers, one look.

| Layer | Example | Code | Exit |
|---|---|---|---|
| Content diagnostic | a link that points at nothing | catalog code (`link.target.missing`) | 1 (errors), 6 (`--strict` warnings) |
| Refusal before any write | an output folder ink will not clear | `output.folder.source` | 2 |
| Failure of the machine | a disk that is full | `output.file.unwritable` | 2 |
| Usage | an argument that does not exist | `cli.usage` | 2 |

Each is printed as the block of section 7, built from one typed value with four fields: **what**
(the sentence), **causes** (the `source()` chain, outermost first), **fix** (a typed `Fix`, the one
`diag` already has, now also given to every `Error` variant) and **state** (what the run left
behind). Today the fix is baked into the sentence of a refusal (`...; nothing was changed. Choose
an empty folder outside the site with -o`); the design pulls it out so text shows it on its own
line and `uses.error/v1` gains `fix` and `state` fields beside `what` and `causes`.

Rules:

- The first line is `error[code]: sentence`, the sentence reads without the code, and the code is
  what you search for and what `--json` names. A code is always shown, also in a notice.
- A cause is a sentence of its own, not a repeat of the line above.
- The fix is an instruction, and when it is a command it is the command to type, in `Literal`.
- If the cause is the person's folder, the sentence names the file. If it is ink's, it says so and
  says to report it, as the `engine.*` entries do.
- A refusal says `nothing was changed` and means it: refusals happen before any write.
- Snippets use `annotate-snippets`, which the CLI already depends on (`report.rs`, with
  `Renderer::plain()`). The design drives its styled renderer from the theme's roles (it takes
  `anstyle` styles) instead of adding `ariadne`, which would be a second snippet renderer for no
  gain: ariadne's coloured labels and multi-line arrows are richer, and ink's diagnostics have
  one span each.
- A panic is a bug, and prints `error[cli.panic]` with the location and a request to report it,
  and exits 101 as Rust does. It is not in the documented set.
- Interrupting (Ctrl-C) stops at the next safe point, says `Stopped: nothing was written` and exits
  130 (open choice 10).

## 10. `--json`

Unchanged in shape; extended in coverage. The contract:

- stdout holds **one complete JSON document**, on success and on failure. A failure prints
  `uses.error/v1` instead of the command's schema.
- stderr holds **NDJSON events**: diagnostics as `uses.event/v1`, notices with `-v`, and progress as
  `uses.progress/v1` (new). Every event has a `schema` field.
- No colour, no glyphs and no wrapping reach JSON. Paths that are not UTF-8 use U+FFFD.
- A command that writes nothing to stdout with `--json` has a bug.

Schemas, kept exactly: `uses.build/v2`, `uses.check/v1`, `uses.explain/v2`, `uses.trace/v1`,
`uses.select/v1`, `uses.error/v1`, `uses.event/v1`. New: `uses.progress/v1`, `uses.push/v1`,
`uses.watch/v1`, `uses.status/v1`, `uses.link/v1` (init) and `uses.export/v1`. A command not yet
built names its schema when it is built, as `uses.<command>/v1`.

**Where text and JSON share one model.** Each block type implements two traits: `Render` (it
produces styled lines for the renderer) and `Serialize` (serde). A command builds its blocks and
hands them to one `Emit`, which chooses by flag: text through the renderer, JSON through serde. The
rule that keeps them honest is a test per block: the text of a block holds every field the JSON
holds, checked on the same fixture value. Adding a field to a block without rendering it, or
rendering a value that JSON lacks, fails that test.

`flow.md` section 7 names `--json-stream` for streaming commands, while the code streams events on
stderr under plain `--json`. The design keeps the code's way: one flag, a document on stdout, events
on stderr (open choice 6).

## 11. Exit codes

The closed set stays. This table is printed by `ink help exit` and each command's help lists the
ones it can end with.

| Code | Meaning |
|---|---|
| 0 | Done. Warnings, if any, were printed. |
| 1 | Content error: the build or check failed on diagnostics, or `build --check` found the two builds differ. Nothing changed live; `ink build` wrote nothing. |
| 2 | Usage error: a folder that is missing or cannot be listed, a path given wrong, an output folder `ink build` will not write into, or a confirmation was needed and not given. |
| 3 | Not signed in, or access to a source was lost. |
| 4 | Network or service unavailable after retries, a build that failed, or a wait that ran out. Safe to retry. |
| 5 | Conflict: site paused after a rollback (a push whose deploy is recorded and not live), a mount conflict, an unpushed commit. |
| 6 | `--strict` was given and there were warnings. Nothing was written. |

101 (a panic) and 130 (an interrupt) are not in the set; see section 9 and open choice 10.

## 12. Completions and the manual

`ink completions <SHELL>` prints a script for bash, zsh, fish, elvish, powershell and **nushell**
(`clap_complete` and `clap_complete_nushell`). `ink man` prints a roff page (`clap_mangen`), as
baudelaire's `man` does. Both are generated from the one command tree that also writes help, so
a new command or option is in all four places or none.

Static scripts complete commands and flags. Values that need the folder complete dynamically
through `clap_complete`'s `CompleteEnv`, behind `COMPLETE=<shell> ink`:

| Value | Completes from |
|---|---|
| a decision `CODE` | the catalog, with its one-line wording as the description |
| `PATH` for `explain`, `pin`, `select` | the files of the folder `-C` names |
| `DEPLOY` for `deploy show` and `rollback` | the site's recent deploys (M3; offline, none) |
| `--source`, `--color`, `--host-mode` | the enum values with their help |

Completion never writes, never prompts and never exceeds 200 ms; it prints nothing rather than an
error. Completion scripts are not themselves coloured.

## 13. Configuration and environment

### Three places, one precedence

| Place | Holds | Example |
|---|---|---|
| `ink.toml` in the site | The site: pins, selection, theme tokens, the image ladder (decision 0044 bucket 1) | `include = ["notes/**"]` |
| `~/.config/ink/config.toml` (`$XDG_CONFIG_HOME`) | The person's terminal preferences and, in M3, the account | `[ui] color = "never"` |
| Environment and flags | One run | `INK_COLOR=never`, `--color never` |

**Precedence: flag, then environment, then `~/.config/ink/config.toml`, then default.** `ink.toml`
never holds a terminal preference: a site is shared, a terminal is not. Operator limits (0044
bucket 2) are flags and the host's own config, not `ink.toml`.

### `[ui]` keys

| Key | Flag | Environment | Values |
|---|---|---|---|
| `color` | `--color` | `INK_COLOR` | `auto`, `always`, `never` |
| `glyphs` | | `INK_GLYPHS` | `unicode`, `ascii` |
| `progress` | `--no-progress` | `INK_NO_PROGRESS` | `live`, `plain` |
| `width` | | `INK_WIDTH` | a number, `0` for no limit |

### Colour decision

Decided per stream, so `ink build | less` keeps colour on stderr. First match wins:

1. `--color always` or `never`.
2. `INK_COLOR`.
3. `NO_COLOR` set and not empty: off.
4. `CLICOLOR_FORCE` set and not `0`: on.
5. `CLICOLOR=0`: off.
6. `TERM=dumb`: off.
7. The stream is a terminal: on. Otherwise off.

The same decision, written once in `Terminal`, also picks glyphs (an `ascii` fallback when the
locale is not UTF-8 or `TERM=dumb`) and progress (live only on a terminal that is not `dumb`).
`--color` is read from the raw arguments before clap parses, so `ink --color never --help` is
plain, as baudelaire's `Color::declared` does.

### The environment ink reads

| Variable | Decides |
|---|---|
| `INK_COLOR`, `INK_GLYPHS`, `INK_NO_PROGRESS`, `INK_WIDTH` | The `[ui]` keys above |
| `INK_DIR` | `-C` |
| `INK_LOG`, `INK_LOG_FILE`, `INK_LOG_FORMAT`, `INK_LOG_SPANS`, `INK_LOG_TIMINGS` | The log (`docs/tracing.md`): `--log`, `--log-file`, and the `[log]` keys `filter`, `file`, `format`. `RUST_LOG` is not read |
| `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE` | Colour, as above |
| `TERM`, `COLUMNS`, `LANG`, `LC_ALL` | Dumb terminal, width, whether the locale is UTF-8 |
| `INK_COMPONENTS` | The folder the build script reads components from (0054) |
| `EDITOR`, `VISUAL` | What `ink login` or a future prompt opens, if one ever does |

`ink help environment` prints this table. A variable ink does not read is not documented.

## 14. Accessibility

- **Meaning never rests on colour.** Every status is a word (`live`, `failed`, `warning`) as well
  as a colour and a glyph. The test: strip all styling and the output still says everything.
- **Plain mode.** `INK_COLOR=never INK_GLYPHS=ascii INK_NO_PROGRESS=1` gives ASCII only, no redraw, no
  escape codes. `TERM=dumb` implies all three. One name for it is open (choice 12 mentions
  `INK_PLAIN`).
- **No motion off a terminal, and none a screen reader would re-read.** Live redraw uses carriage
  return and erase-line on one line only; a reader that announces every change would repeat the
  stage, so `--no-progress` is documented as the setting for them.
- **Reading order.** A block reads top to bottom: what, where, why, what to do. Glyph-only
  columns come before their word, never after.
- **Contrast.** Only the person's own ANSI colours are used, with no background colours (section
  5); the HTML preview was checked against WCAG AA for the 16 roles in light and dark.
- **Width.** A narrow terminal (40 columns) cuts the flexible column and wraps prose; it never
  breaks a code or a path in the middle of a word without an ellipsis.
- **No emoji, no flashing, no sound.**

## 15. What breaks

Before 1.0 nothing here is public, so nothing needs a shim. The list is the migration.

| Today | Design | Why |
|---|---|---|
| `ink build <DIR>`, `check <DIR>`, `preview <DIR>` | `ink -C <DIR> build` or run from the folder | One folder convention |
| `-C, --dir` on `explain`, `pin`, `select` | Global `-C` | Same |
| `ink serve [FOLDER]` defaulting to `.ink/out` | default `<site>/.ink/out`; `[FOLDER]` kept | `-C` applies |
| `--json` and `-v` listed under every command | Under `Global options:` | Said once |
| `ink explain --why PATH CODE` (one or two values) | `ink explain [PATH] --why CODE` | Same address shape as `pin` |
| `error: ...` and `  caused by: ...` | `error[code]: ...`, rail, `fix:`, state | Code, cause and fix on every failure |
| `note[code]: wording (subject)` | `note[code]: wording`, then `fix:` | Same block as an error |
| `ink check` prints `61 pages, 312 decisions, 1 errors, 2 warnings` | `Checked  61 pages, 312 decisions: 1 error, 2 warnings` | Verb gutter, plural |
| `Renderer::plain()` for snippets | styled renderer driven by the theme | Colour in one place |
| no `-q`, no `--color` | both | `conventions.md` and clig.dev |
| `uses.error/v1`: `exit`, `error`, `what`, `causes`, `diagnostic` | plus `fix`, `state` | Additive |

Exit codes, schemas and JSON stay compatible. Tests that pass `<DIR>` positionally change with the
CLI in the same commit.

## 16. The renderer

One module, `crates/surface/cli/src/output/term`, owns every byte written to a terminal. It is built; the table is the design's sketch, and the code may name things differently (`crates/surface/cli/src/output/term/mod.rs` is the source):

| Type | Is |
|---|---|
| `Role` | The closed enum of section 5 |
| `Theme` | `Role` to `anstyle::Style`; the glyph set; one value per run; a plain one for `never` |
| `Terminal` | What the run detected once: colour and glyphs per stream, width, live or plain progress |
| `Block` | The ten blocks of section 6 as a closed enum, or one trait object each |
| `Render` | `fn render(&self, into: &mut Doc)`: a block produces roles and text, never escapes |
| `Doc` | Lines of `Span { role, text }`; knows nothing of a terminal |
| `Table`, `Tree`, `Rail`, `Gutter` | The layout types of section 6; the only code that pads |
| `Emit` | Takes blocks, writes text (through `Terminal`) or JSON (through serde), to the right stream |
| `Help` | Renders help from the one command tree |

`Doc` is data, so three writers sit on it: a terminal writer (ANSI through `anstream`, which
strips for non-TTY), a plain writer (tests), and an **HTML writer** that is how `cli-preview.html`
would be made from the real renderer instead of by hand (open choice 11). The existing wrappers
(`Line`, `Head`, `Step`, `Chain`, `Shown`, `Noted`, `Listed`) become `Render` impls: their wording
stays, their `write!` calls become spans.

Dependencies, all already in the tree except one: `anstyle` and `anstream` (clap's own, so no
new weight), `annotate-snippets` (kept, styled), `clap_complete`, `clap_mangen`, and `indicatif`
for the live line (baudelaire uses it; wrapped so a `Stage` and a `Role` drive it).

Tests: a golden plain-text snapshot of every block at widths 40, 80 and 100 with `NO_COLOR`, an
ANSI snapshot per role table, one JSON-versus-text parity test per block (section 10), and the
existing CLI tests unchanged but for section 15's changes.

## 17. Build order

Four slices, each green on `farm check` before the next. None changes a schema or an exit code.

1. **The renderer and the diagnostics.** `term` with `Role`, `Theme`, `Terminal`, `Doc`, the plain
   and ANSI writers; `--color`, `-q`, `INK_*`; diagnostics, notices and errors through it; `fix`
   and `state` on every `Error`; the styled snippets. Snapshot tests.
2. **Command tree, help, completions and the manual.** One declarative tree; `Help`; global `-C`,
   groups; `ink completions`, `ink man`, `ink help exit`. The breaks of section 15.
3. **Output blocks.** Stage lines, change set, explain tree and ladder, select list, pin result;
   `Checked`/`Built`/`Stopped` verbs. Progress with the live line and the plain log.
4. **M3 verbs** as they land (`status`, `deploy list`, `watch`, `push`) use the blocks as they are:
   a verb that needs a new kind of block adds it to section 6 first.

## 18. Open choices

The owner decides; each has a recommendation.

1. **The folder as a global `-C`**, removing the positional `<DIR>` of `build`, `check` and
   `preview`. The cost is `ink -C garden build` where `ink build garden` is today. *Recommend yes:*
   one convention, and the daily case is running inside the folder.
2. **Hybrid grammar** (verbs plus a `deploy` group) versus all verbs. *Recommend the hybrid*:
   it matches flow.md's names and only `deploy` needs a noun.
3. **How a decision is named**: `explain [PATH] --why CODE` and `pin [PATH] CODE` (design), or
   `explain [PATH] [CODE]` where a code means the ladder. *Recommend the design*: the second needs
   to guess whether `site.title` is a file.
4. **Progress style**: cargo's verb gutter (design) or baudelaire's glyph on every line. *Recommend
   the gutter*: a build prints five lines and a verb is the information.
5. **Should `-q` keep the newly-public list?** *Recommend yes*, it is the irreversible one.
6. **Events**: keep NDJSON on stderr under `--json` (design) or add `--json-stream` as flow.md
   says. *Recommend keeping the code's behaviour* and updating flow.md.
7. **Aliases `b` and `c`.** *Recommend yes*, only those.
8. **Dependencies**: `anstyle`/`anstream` and `indicatif` (design) or `owo_colors` as baudelaire.
   *Recommend anstyle*: styles are data, so the HTML writer and the theme table need no second
   model, and clap already pulls it in.
9. **Times in listings**: UTC ISO (design, matches flow.md) or relative. *Recommend ISO in
   tables*; relative time in prose is a lie that changes.
10. **Exit 130 on interrupt, 101 on panic**, outside the closed set 0 to 6. *Recommend yes, and
    document them as the shell's and Rust's own.*
11. **Generate `cli-preview.html` from the renderer's HTML writer** and keep it in the repo as the
    golden-snapshot viewer. *Recommend yes, in slice 3*; until then it is written by hand.
12. **A `--width` flag and an `INK_PLAIN` shortcut.** *Recommend neither for now*: `INK_WIDTH`
    and the three variables of section 14 are enough, and each flag is a global option.
13. **`ink help <code>` as a long explanation of a catalog entry** (as `rustc --explain`).
    *Recommend yes, after slice 2*; the catalog already holds wording and fix.
14. **The bare deploy id on a terminal's stdout.** Design prints it undecorated everywhere.
    *Recommend keeping it bare*: decoration on a terminal only would make stdout depend on the
    terminal, which principle 2 forbids.
