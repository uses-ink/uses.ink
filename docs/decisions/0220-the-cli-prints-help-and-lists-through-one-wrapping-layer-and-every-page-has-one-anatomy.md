---
number: "0220"
title: "The CLI prints help and lists through one wrapping layer, and every page of help has one anatomy"
status: built
date: 2026-11-28
area: cli
supersedes: []
---

## 0220 The CLI prints help and lists through one wrapping layer, and every page of help has one anatomy

Built (2026-11-28). Decision 0061 gave the CLI one output grammar and one renderer, and left help as a
second renderer: a string painter (`Page`, `Paint`, `Face`) that built text by hand and handed it to the
terminal layer as pre-painted bytes. That split is why help and the other human output drifted apart. Help is
now lines of roles in the same `Doc` as every other block, and one wrapping layer serves help, key and value
blocks and lists.

### What was wrong

Captured from the real binary (a self-hosted server on a farm host, `ink` run with an empty environment).

- **A wall before the usage.** `ink serve --help` opened with a four line paragraph, then the usage, then
  about 400 lines of options. `ink push --help` put a five sentence paragraph above `Usage:`. The one line
  purpose and the usage were never next to each other.
- **No hierarchy.** `Options`, `Server` and `Arguments` had no colon, `Examples:`, `Exit codes:` and
  `Environment:` had one. Every long page repeated the full exit-code and environment tables, so the
  examples (the part a person came for) sat in the middle.
- **Unreadable option rows.** Defaults and variables were prose inside the description, wrapped through:
  `[default:` ended one line and `127.0.0.1:8080]` began the next. Placeholders were the argument's id in
  capitals (`<API_BODY>`, `<HOST_MODE>`), and the whole left column was one colour, flag and placeholder
  alike. Long descriptions sat in a 38 column sliver beside 30 column terms, and the 100 column ceiling was
  the only narrow rule.
- **Examples that were not examples.** Two columns, the line and its text, with the text in the same colour
  and no mark that one is a comment. Several did not parse: `ink org member list 0123abcd` fails because an
  id is 32 hex digits.
- **A footer that overflowed.** `Global options: -C, --json, -q, ...` was one unwrapped line of 96 columns
  on every page.
- **Two width rules.** Help read `COLUMNS` even on a pipe, every other block read the terminal and ignored
  it off a terminal.
- **A flat command list.** Thirteen unrelated verbs (sign in, keys, organizations, abuse, push, status,
  domains) sat under `Publish`; `serve` sat under `Write`.
- **The other human output.** `no organizations`, `no previews`, `no custom domains` stopped there, with
  no next step. `ink key list` with nothing to list printed a header row and nothing under it. Records
  (`whoami`, `org create`, `domain add`) did not wrap, so a long value ran off the line. Clap's own `Usage:` and
  `For more information, try '--help'.` footers still end every usage error, in quotes where ink writes
  backticks.
- **Three small faults found on the way, fixed with it.** `ink key revoke nosuch` reported
  `deploy.id.unknown` ("no deploy of this site starts with `nosuch`"), the wrong error domain for a key; it
  is now `auth.key.unknown` with a fix. `deploy.api.refused` repeated its own message as its only cause; the
  headline now names the status and the cause carries the code and message. `Followed  build X is succeeded`
  read as a sentence with no subject; it is `Followed  build X, now succeeded`.

### The anatomy

Every page, short and long, is these blocks in this order, each present only when it has content:

1. **Purpose**: one sentence. Backticked words are typed and are drawn as such.
2. **Usage**: `Usage: ink key add [OPTIONS]`. The command words are strong cyan, placeholders
   (`[OPTIONS]`, `<COMMAND>`, `[FOLDER]`) are cyan, so what is typed and what is filled in differ.
3. **Examples** (long form): two to four lines that parse, each with a comment after `# `. Commands of the
   tree are strong, flags are cyan, what is given to them is plain, the comment is dim. When every comment
   fits beside its command they line up; when one does not, each comment moves above its command, because
   a command is never broken across lines.
4. **Commands** (a group) or the six task groups of the root: Write, Understand, Publish, Account,
   Operate, Tools. An aligned table of names and one line summaries.
5. **Arguments**, then **Options** and the option groups (Server, Limits, Account, ...). The term is the
   flag and its placeholder (`-o, --out <DIR>`), the meaning wraps with a hanging indent. A default is the
   dim atom `[default: x]`, a variable the dim atom `[env: INK_X]`, a required option the yellow atom
   `[required]`; an atom moves to the next line whole and is never split.
6. **Notes** (long form): the longer paragraph of the command, moved below the options.
7. **Exit codes** and **Environment** (long form), only what that command can end with or reads.
8. **A footer table**: `See also` and `Docs` (long form), `Global` (the global flags, pointing at
   `ink --help`), and in short form `More`, which names the `--help` that adds the rest.

`ink help <command>` and `<command> --help` are one page. `-h` stops after the options. Long text stays in
the tree (`Entry.detail`) and in `docs/cli.md`, and the footer points at the docs.

### Width and colour

Help wraps at the width of the stream between 40 and 100 columns, at 80 when the stream has no limit (a pipe
or `INK_WIDTH=0`), and at the `INK_WIDTH` it is given first, then the terminal's. Colour follows the stream's
`Look`: `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`, `--color`, a terminal or not. There is one
new role, `Value`, for placeholders. A term wider than 45 percent of the line, or fewer than 34 columns left
beside the terms, writes each meaning below its term, indented six.

### The layer

`output/term/layout/flow.rs` has `Line::wrap` (greedy, breaks between words, keeps an `atom` whole), `Paragraph`,
`Defs` (terms and meanings; `Record` is now a `Defs`, so key and value blocks wrap), `Samples` (lines to type
with comments) and `Finished` (a page laid out at a width of its own). `output/term/layout/listed.rs` has `Listed`
(rows under a dim header) and `Empty` (what a list says when it has no rows, and a command that makes the
first one). The six lists (`key`, `org`, `domain`, `repo`, `preview`, `abuse`) use them. `Styled`, `Paint`,
`Face` and `Io::painted` are gone. Help is `grammar/help`, with the tree (`Entry.see`, `Tree::walk`) as its
data and `Tree::DOCS` (the repository's `docs/cli.md`) as the one link.

### The gate

`grammar/help/golden.rs` walks the whole tree with `Tree::walk` (every verb at every depth, which the old
integration test did not) and fails when:

- a page differs from its golden in any of four forms: long plain at 80, long with colour (an escape written
  `\e`), long at 60 columns, short plain (`fixture/help/`, 4 forms for each of 52 pages, plus the exit and
  environment tables);
- a verb has no summary, or more than four or no example, or an example without a comment, or an example that
  does not parse with the real clap command, or one that is about another command, or a `See also` that names
  no command;
- an option has no description, or a description mentions `[default` anywhere but at its end;
- a line of any page is wider than its page at 60 and 80 columns (a command in `Examples` is exempt, because
  it is never broken) or has a dash or arrow glyph in prose;
- a line of `command/` or `selfhost/` holds a string with a sentence after a sentence: output text is typed,
  one sentence at a time.

`tests/help.rs` keeps the end to end checks through the real binary (`help X` equals `X --help`, no stderr).

### Not done

Placeholders are still the argument's id in capitals (`<API_BODY>`) for about forty server flags, and
the quotes of clap's usage footer; both need edits in every argument declaration and in `Failure::usage`.
Success lines (`Linked`, `Built`) and the `status` and `init` records were left as they are.

**Why:** a page of help that is a wall of text is the same defect as a status that is a wall of text, and the
fix is one layer that wraps, aligns and colours, with a gate that makes a new verb arrive with its example.
**Reversal:** none; the roles and the anatomy are the contract of `docs/cli.md` section 4 and can be redrawn
there with the goldens.
