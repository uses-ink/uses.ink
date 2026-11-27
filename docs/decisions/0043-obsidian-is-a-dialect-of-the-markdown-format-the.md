## 0043 Obsidian is a dialect of the markdown format; the engine resolves and embeds

Accepted. `ink:format-obsidian` (`plugin/format/obsidian`, its component in `guest/`) reads one
note through the markdown format's comrak setup and raw HTML allowlist (0027), as a dialect:
`format_markdown::dialect::Dialect` turns on the syntax comrak parses for it and says what its
own nodes mean, and `format_markdown::read_as` reads through the one builder, generic over the
dialect. Nothing in the dialect sees comrak. What it reads, as built:

- **Wikilinks** `[[Note]]`, `[[Note|text]]`, `[[Note#Heading]]`, `[[Note#^block]]`,
  `[[#Heading]]` become `LinkTarget::Unresolved` with the target as written (the last `#` part
  is the anchor), shown as Obsidian shows them (`Note > Heading`) when no `|` names the text.
- **Embeds**: `![[image.png]]` is an image whose alt text is the text after the `|` or the file
  name, a size there said (`embed.size.ignored`); `![[Note]]`, `![[Note#Heading]]` and
  `![[Note#^block]]` alone in a paragraph are the new `Block::Embed`; among other text they are
  the new `Inline::Embed`, which the engine turns into a `Block::Embed` between the paragraph's
  text before it and after it (blocks cannot sit inside text), and where only text fits (a
  heading, a table cell, a link's text) into a link, `embed.inline.linked`; other files (PDF,
  audio, video, canvas) are links (`embed.file.linked`).
- **Callouts**: comrak's Obsidian callouts, any kind (Obsidian's aliases mapped: `hint` is a tip,
  `tldr` an `abstract`), a title of inlines, and `Block::Callout::folding` (`Fixed`, `Open` for
  `+`, `Closed` for `-`), which the quiet theme writes as `<details>`. Metadata after `|` is said
  and dropped (`callout.metadata.ignored`).
- **Block ids**: `^id` ending a block, or alone on the line after one, is the new
  `Inline::Anchor`. Ids share a page's anchors with its headings; one whose slug is taken is
  `block.id.taken` and gets the next free one, so no link meant for the earlier anchor moves.
- **Comments** `%%x%%` and `%%` blocks are removed, as HTML comments are.
- **Tags** `#tag` and `#a/b` in the text (not in code or link text) are `Read::inline_tags`, each
  once by slug and with its own span, apart from the frontmatter's `Front::tags`. The engine
  joins them (below).
- **Aliases**: frontmatter `aliases` or `alias` fill the new `Front::aliases`.
- **Title**: a note's title is its file name, as Obsidian shows it, after a frontmatter `title`.
  The dialect says so (`Syntax::title`), and the format reports no first heading, so the
  existing title ladder (`Heading`, then the file name) finds none and falls to the name. The
  headings stay headings.

Obsidian lacks `^x^` superscript and definition lists, so they stay text. It also shows `~x~` as
written and strikes only `~~x~~`. comrak has no switch for that, so the dialect (`Extension::
DoubleStrike`) reads the source at each struck run and turns one that does not open with `~~`
back into the text it was; code spans and blocks never become a struck run, so they are
untouched.

The engine resolves, since a format reads one file. The link machinery is the existing one,
extended for every format: after the page-relative path, its extension and a folder's index, a
name is looked for anywhere under the root, letter for letter and then in any case, by file
name, stem or alias; a path with a `/` names the file whose path ends with it. This is
Obsidian's shortest-path resolution: a unique name or partial path resolves, several are
`link.target.ambiguous`, one unpublished is `link.target.unpublished`. The `Settings` stage reads
`attachmentFolderPath` from `.obsidian/app.json` under the root (`/`, `./`, `./name`, `name`;
`vault.settings.invalid` when it does not read); a name several files share settles on the one
in the attachment folder for the linking note, said as `link.target.attached`. Obsidian itself
never consults the setting to resolve; this is ours, and the notice keeps it from being a silent
guess. The same stage, run before `file.ignore`, reads the templates folder: `folder` in
`.obsidian/templates.json`, used while `.obsidian/core-plugins.json` (a list of the plugins that
are on, or a map of each to whether it is) has the templates plugin on or is absent; files under
it are never pages (`Folder<Templates>`, evidence naming the folder, in any case). With no
folder set, or the plugin off, the conventional `templates/` at the top stands, as before. Each of
the three files that does not read is `vault.settings.invalid` naming that file. The `Embeds` stage fills each embed of a published page with the page's blocks, the
section under the heading (to the next heading of its level or above), or the innermost block
holding the block id (the block before, for an id alone on its line). What is shown keeps
pointing at its own page: anchor links become links to it, its block ids are removed and its
footnote references dropped (`embed.footnote.dropped`). A page holds an `id` once, so a heading
it shows gets the next free `name-1`, `name-2` when the page, or an embed before it, already
has its anchor; the order is the document's, so the ids are the same on every build, and a link
inside the shown part still points at the embedded page's own anchor. An embed reaching a page and part it is
already inside is `embed.target.cycle`, one naming no part is `embed.anchor.missing`, past 64 per
page (embeds inside embeds counted) is `embed.budget.count`; each is shown as a link.

**Selection** follows the source's: `ink` reads a folder holding `.obsidian/` at its top as a
vault through `ink:format-obsidian`, said as `format.obsidian.chosen`, as a folder holding
`.git` is read as a repository (`source.git.chosen`). The vault layout (`Folder<Obsidian>`)
already publishes nothing until it is selected. The choice is the decision `site.format`, whose
value is a format plugin's id (`ink:format-obsidian`, `ink:format-markdown`) and whose ladder is
`Pin`, then `Folder<Obsidian>` (evidence `Marker { path: ".obsidian" }`), then the markdown
format. A pin is written `[site] format = "ink:format-obsidian"` and said as `format.pin.chosen`;
a vault found by its marker is `format.obsidian.chosen`; the default is silent. A pin wins over
the marker either way, so a vault can be read as plain markdown and a folder without
`.obsidian/` as a vault. An id naming a format this ink does not hold stops the command
(`format.plugin.unknown`). The format is read before anything else, so the host decides it
(`engine::Chosen`) and builds the engine over the format it names; the decision joins the
engine's with `Understood::with`, so `ink explain`, `--why` and `ink pin` treat it as any other,
and it is in the build's record. `ink build --check` builds a vault through
`format-obsidian.wasm`, and `cli/fixture/site/vault` proves both builds match.

**Tags** are the one decision both sources feed, since Obsidian shows a note's frontmatter tags
and its inline tags together. A ladder takes the first value, so `page.tags` is `Pin`, `Inline`,
`Front`, default. `Inline` (evidence `Evidence::Inline { path, span }`, the first tag only the
text said) testifies only when the text adds a tag the frontmatter lacks, and then its value is
the frontmatter's tags followed by the text's new ones, each slug once. When the text adds
nothing, `Front` decides as before, with the frontmatter as evidence. So a decision is one
value with one evidence, and the evidence is the least stated source the value depends on: the
frontmatter's alone, or the text where the text added to it (`Basis::Inferred`). A pin states
every tag, and the text does not add to it. `ink explain --why page.tags` shows the `Inline` rung
first, its joined value, and the `Front` rung beneath it as outranked. The rung stands before
`Front` because it completes `Front`'s value, not because the text outranks the frontmatter.

**Why:** one parser, one allowlist and one set of node rules for both formats; resolution in the engine, where every page is known, so both formats and both
builds share it. **Reversal:** a second dialect whose syntax comrak cannot parse, which would
need a parser of its own.
