# uses.ink: flows, states and copy

> **Status.** Pre-decision design record of flows, states and copy. The `ink` command is now specified in `docs/cli.md` (decision 0061), sign-in in `docs/auth.md` (0068), and the dashboard stack is decision 0053; where they differ from this file, they win.

Scope: what a person does, what they see, and what the service says back. No visual
design. Copy in `code blocks` or quotes is the exact proposed text. Placeholders are
`<like-this>`. Claims about other products that I did not check against a live source
are marked `[unverified]`.

Audience: developers who write in a repo, and note-takers who keep an Obsidian vault
or a notes folder. Positioning: zero config, no lock-in. The CLI is first-class for the
first group; the browser and an Obsidian plugin are first-class for the second (2.6).
Both paths end in the same pipeline, the same diagnostics and the same `ink.toml`.

## 0. Prior art: what to take and what to leave

| Product | Take | Leave |
|---|---|---|
| Old uses.ink (`legacy`, `astro`) | Zero setup: a repo named `blog` with a `README.md` was a blog. README as home page. Auto-generated index when a folder has no README. Dates and authors from git. | Config through the URL (`<user>.uses.ink/<repo>@<branch>/<path>`): unguessable, unshareable, no single canonical address. Rendering anyone's public repo without their consent. Random joke 404 and 500 messages (`MESSAGES_404`). A `uses.ink.json` per directory, so config is scattered. "You will need to add a link to the new post somewhere": a new post that appears nowhere is a silent failure. |
| GitHub Pages | Branch and folder choice is explicit and shown. Custom domain check with a visible "DNS check successful" state `[unverified wording]`. | Jekyll as an implicit build step that changes output without saying so (`_` files dropped silently is the classic trap). Build failures reported by email with no file or line. |
| Vercel / Netlify import | Repo picker after one OAuth hop, framework inferred and shown as an editable "Framework preset", deploy log streamed live, every deploy immutable with its own URL, instant rollback, branch previews, commit status on the PR. | Asking for build command and output directory up front. Team/plan prompts during import. Upsell interstitials. |
| bearblog | No JS for readers, tiny pages, RSS by default, honest "what it does not do" page `[unverified]`. | Editing in a web textarea: our people already have an editor. |
| Mataroa | Export in plain formats, one-click account deletion that deletes `[unverified]`. Plain copy. | Nothing specific. |
| Write.as | Reading view focused on text, federation as an opt-in. | Anonymous-by-default framing does not fit a repo-owned blog. |
| Ghost | Clean RSS, newsletter-ready post model, good default theme. | Admin as a CMS with members, tiers and analytics on the home screen. |
| Hashnode GitHub backup | Proves developers want the repo as the source of truth. | The direction: Hashnode is primary and the repo is a backup copy. Ours is the reverse. |
| Quartz | Reads an Obsidian vault as is: wikilinks, embeds, backlinks, callouts, tags, graph. Its handling of links to unpublished notes (rendered as text) is the right default. | You clone a template repo, run a Node build, and configure in TypeScript (`quartz.config.ts`). Upgrades are merges into your fork. A note-taker who does not use a terminal cannot start. |
| Obsidian Publish | Publish from inside the editor with one command. A change list before each publish (new, changed, deleted notes) that the person ticks. Per-note selection, so a private vault can publish five notes. Backlinks and graph built only from published notes. | Paid add-on for basic publishing `[unverified price]`. Selection state lives in Obsidian's servers and the app, not in the notes, so it cannot be reviewed in git or moved elsewhere. Hosting and theme are closed; leaving means rebuilding. |
| Digital Garden (Obsidian plugin, oleeskild) | Opt-in per note with a frontmatter flag (`dg-publish: true`), so the decision travels with the note and survives moving tools. Publishing from a command inside Obsidian. | The setup: create a GitHub account, fork a template repo, connect Vercel or Netlify, paste a fine-grained token into the plugin `[unverified exact steps]`. Five accounts and tokens for a note-taker is the failure we are fixing. Prefixed keys (`dg-*`) tie notes to the tool. |
| Flowershow | Renders Obsidian-flavoured markdown (wikilinks, callouts, embeds) from a GitHub repo, with a hosted option and an Obsidian plugin `[unverified current feature set]`. Shows that "vault in git, rendered without a build step" has demand. | Relies on GitHub as the only way in for hosted sites `[unverified]`; config in a `config.json` the person writes up front. |
| Leaflet / WhiteWind (ATproto) | Content as records in the author's own repo, readable by other apps. | Editor-in-the-browser as the only way in `[unverified for Leaflet]`. |

The pattern that wins: Vercel's import loop (one hop, inferred settings shown and
editable, immutable deploys, commit status) applied to content instead of code, with
bearblog's reader side.

## 1. Jobs and principles

### Jobs

| # | Job | Done when |
|---|---|---|
| J1 | "Turn the markdown I already have into a public blog without learning a static site generator." | A URL exists that renders my files, with an index and a feed, and I wrote no config. |
| J2 | "Publish by doing what I already do: commit and push." | A push updates the site, and I can see that it did, from the terminal or the forge. |
| J3 | "Know exactly what is public." | I can list every file that is published, every file that is not, and why. |
| J4 | "Own the address and the exit." | My own domain works, and I can leave with my content and the built site with nothing lost. |
| J5 | "Fix it when it looks wrong without guessing." | Every wrong title, date or missing image points at the decision or file that caused it and the line that fixes it. |
| J6 | "Share part of my vault without moving it or learning git." | Selected notes are online with their links, embeds and backlinks working, published from Obsidian or a browser, and the rest of the vault stays private. |

J3 matters most for J6: a vault is private by default in the person's head, so the
vault path asks before it publishes anything (2.6). This is the one place P4 bends.

### Principles

Each principle is testable: a reviewer can point at a screen or output and say pass or
fail.

| # | Principle | Test | Do | Don't |
|---|---|---|---|---|
| P1 | **Show the decision, its evidence, and its override.** Every inferred value carries where it came from. | Any inferred field on any screen has a "from" line and a config key. | `Date 2025-03-14, from first commit a1b2c3d. Override: date: in frontmatter.` | `Date 2025-03-14` with no source. |
| P2 | **A missing capability is a sentence, not a gap.** | For each capability a source lacks, one line on the Site screen names the consequence. | `This source has no history. Dates come from frontmatter; 4 posts have none and are undated.` | Silently stamping undated posts with the import time. |
| P3 | **Errors are diagnostics: code, what, why, fix.** | Every error in UI, CLI and commit status has a stable code that resolves at `uses.ink/d/<code>`. | `image.missing posts/rust.md:12 references ./img/ferris.png, which is not in the snapshot. Add the file or fix the path.` | `Build failed. Please try again later.` |
| P4 | **Publish first, explain after, never block on review.** The first publish does not wait for the person to approve inferences, but it never publishes anything J3 would surprise them with. | First publish happens in one click from repo choice; the "What we understood" screen opens next to the live URL. | Publish, then show 31 decisions with 2 flagged. | A 5-step wizard asking for posts folder, date format and theme. |
| P5 | **Nothing is published that is not content.** Default excludes are listed, not implied. | The file list shows every excluded path with its rule. | `LICENSE excluded by rule repo-meta.` | Publishing `notes/salary.md` because it was markdown. |
| P6 | **The terminal is a peer.** Anything the dashboard does, the CLI does, with the same words and codes. | Diff the strings: UI and CLI share one copy table. | `ink status` and the Site screen print the same three facts. | A setting that exists only behind a dashboard toggle. |
| P7 | **Facts over reassurance.** State what happened and its size; no adjectives about how well it went. | No copy contains "successfully", "awesome", "all set", "just", or an exclamation mark. | `Published 23 pages in 4.1 s. Deploy d_7f3k.` | `Your blog is live! 🎉` |
| P8 | **State never by colour alone.** | Every status has a word and a glyph shape; screenshots pass in greyscale. | `failed` with a cross glyph and the word. | A red dot. |
| P9 | **Readers need nothing.** No JS, no cookies, no account for public pages. | Load any public page with JS disabled; feeds validate; Lighthouse a11y 100 on default theme. | Footnotes as anchors, code copy as a progressive enhancement. | Comments widget or analytics script by default. |
| P10 | **Every action is reversible or says that it is not.** | Destructive actions name what goes and what stays, in one confirm. | `Deletes 3 deploys and the handle cstef. Your repository is not touched.` | `Are you sure?` |

## 2. Primary flow

### Target

- **Live blog in under 60 s** from landing on `uses.ink` for a repo of 200 markdown
  files on the GitHub path. Budget: 15 s of human clicking, 10 s of GitHub's screens,
  under 20 s p95 first build.
- **N = 4 clicks** on the GitHub path, counting clicks on GitHub's own screens and
  excluding GitHub sign-in or 2FA if the person is logged out there. Zero typing when
  the GitHub login is free as a handle and one candidate repo exists.

| Path | Clicks | Typing | Time target |
|---|---|---|---|
| GitHub (also Codeberg/Forgejo, GitLab) | 4 | none | < 60 s |
| Bare git URL | 4 plus one commit or DNS record | URL, email | < 60 s to a private preview; publish when proof lands |
| `ink` CLI push | 0 clicks after install | 2 commands | < 30 s from `ink push` |
| Folder or vault upload in the browser | 5 (incl. the magic link) | email | < 60 s; first publish waits on the note selection (2.6) |
| Obsidian plugin | 4 inside Obsidian after install | none | < 60 s after install |

### 2.1 Landing: `uses.ink`

What the person sees, top to bottom. This is the order it is used, so the actions are
first and the explanation is under them.

```
uses.ink

Point it at markdown you already keep. Get a site at <you>.uses.ink.

From a repository
[ Continue with GitHub ]  [ Codeberg ]  [ GitLab ]
Other git URL: [ https://...                ] [ Preview ]

From a folder or Obsidian vault
[ Choose a folder ]   (nothing is uploaded until you pick what to publish)
In Obsidian: install the "uses.ink" community plugin.
In a terminal: uv tool install uses-ink   (or: cargo install uses-ink)   then  ink push

What it does
- Renders the markdown files in one repository, branch, folder or vault.
- Understands Obsidian links, embeds, callouts and backlinks.
- Infers home page, posts, titles, dates and drafts, and shows each guess.
- Rebuilds on push. Feeds, custom domains, no JavaScript for readers.

What it does not do
- Run your build. No Hugo, Jekyll or npm step; it reads files, not output.
- Host apps, forms, comments or analytics.
- Edit your content. It never writes to your repository.

Example: cstef.uses.ink, built from github.com/cstef/blog.  [ See what we understood for it ]
Self-host: one binary, same features. docs/self-host
```

Behind it: nothing. No cookie is set until sign-in.

Note on "never writes to your repository": the GitHub App asks for `contents: read`,
`metadata: read`, `checks: write` (commit status). Checks are not repository content.
The landing copy is accurate; the permission screen says it again (2.2).

### 2.2 GitHub path

| Step | Click | Screen | What happens behind |
|---|---|---|---|
| 1 | 1 | Landing, `Continue with GitHub` | Redirect to GitHub App install-and-authorize URL with `state`. |
| 2 | 2 | GitHub: "Install uses.ink", person picks "Only select repositories" and chooses one | GitHub screen. We preselect nothing; GitHub defaults to "All repositories" `[unverified]`, so our step-1 page footer says: `On the next screen, choose "Only select repositories". uses.ink reads only what you select.` |
| 3 | 3 | GitHub: `Install & Authorize` | GitHub returns installation id and user token. We create the identity, reserve the handle = GitHub login (lowercased), list the selected repos. |
| 4 | 4 | uses.ink: Publish screen, `Publish` | Snapshot, Interpret, Render, Publish. |

Step 4 screen when one repo was selected (the common case):

```
Publish github.com/cstef/blog

Address   cstef.uses.ink                         [change]
Branch    main  (default branch)                 [change]
Folder    /     (repository root)                [change]

Found 41 markdown files. 23 will be published, 18 excluded.
Home page   README.md
Posts       posts/  (19 files with dates)
Drafts      2, not published  (draft: true)
[ show all files ]

[ Publish ]

uses.ink can read this repository and set commit status on it. It cannot write to it.
```

Values shown here come from an Interpret pass on a snapshot fetched during the
redirect back; the page is server-rendered and shows `Reading github.com/cstef/blog,
3,412 files...` with a no-JS meta refresh every 2 s until it is ready (typically under
3 s).

If several repos were selected, a radio list sorted by: name `blog`, then most `.md`
files, then most recent push. The top one is preselected, so the click count stays 4.

`[change]` on Address opens an inline field. Handle rules shown under it:
`3 to 32 characters: a-z, 0-9, hyphen. Not starting or ending with a hyphen.`

After `Publish`, the same URL shows progress, one line per pipeline stage, each with
its own time. Without JS it refreshes; with JS it streams.

```
Publishing deploy d_7f3k from main @ a1b2c3d

  done   Snapshot     3,412 files, 4.2 MB             0.9 s
  done   Interpret    23 pages, 31 decisions          0.2 s
  ...    Render       14 of 23 pages
         Publish

```

Then:

```
cstef.uses.ink is live.   [ Open ]   [ Copy URL ]

Published 23 pages in 4.1 s. Deploy d_7f3k from main @ a1b2c3d.
New pushes to main publish automatically.

31 decisions were inferred. 2 need a look:
  - 4 posts have no date in frontmatter and no commit older than the import. Using commit dates.
  - posts/old/ has 3 files without titles. Using file names.
[ Review what we understood ]
```

"cstef.uses.ink is live." is the one sentence that states the outcome; it is a fact,
not a celebration.

Failure states on the GitHub path:

| Code | Trigger | Copy | Fix offered |
|---|---|---|---|
| `auth.denied` | Person cancels on GitHub | `GitHub did not grant access, so nothing was created.` | `[ Try again ]` |
| `auth.no_repos` | Installed with zero repos selected | `uses.ink was installed on GitHub with no repositories selected, so there is nothing to read.` | `[ Choose repositories on GitHub ]` deep link to installation settings |
| `handle.taken` | Login already a handle | `cstef is taken on uses.ink. Pick another; your GitHub login is unchanged.` Suggestions: `cstef-blog`, `cstef-gh` | inline field |
| `handle.reserved` | Name in reserved list (`www`, `api`, `docs`, `admin`, ...) | `<name> is reserved for uses.ink itself. Pick another.` | inline field |
| `source.empty` | No markdown files on the branch | `main has no .md files. uses.ink renders markdown; it does not run builds.` | `Use another branch`, `Use another folder`, link to "what counts as content" |
| `source.too_large` | Snapshot over limit | `This repository is 2.1 GB. The limit is 1 GB per snapshot. Pick a folder to narrow it.` | `[ Choose folder ]` |
| `source.build_output` | Repo looks like Hugo/Jekyll/Astro source (has `config.toml` plus `content/`, or `_config.yml`) | Warning, not error: `This looks like a Hugo site. uses.ink reads content/ directly and does not run Hugo, so shortcodes render as text.` | `[ Use content/ ]` `[ Publish as is ]` |
| `forge.unavailable` | GitHub API 5xx or rate limit | `GitHub did not answer (503). Retrying in 10 s, attempt 2 of 5.` Then: `GitHub is still not answering. Nothing was published. We will retry when it does and email you.` | `[ Retry now ]` |
| `render.failed` | A page cannot render | Partial publish is refused. `Not published: 1 page failed. posts/x.md:40 unclosed code fence opened at line 12.` Previous deploy stays live (none on first run). | `[ Open file on GitHub ]` |

Decision: **a deploy is all or nothing.** One broken page fails the deploy and leaves
the previous one live. Warnings (missing alt text, undated post) do not fail it.

### 2.3 Codeberg/Forgejo and GitLab

Same four steps with OAuth instead of a GitHub App. Differences, shown on the Publish
screen rather than hidden:

| Forge | Webhook | Commit status | Note on screen |
|---|---|---|---|
| Codeberg / Forgejo | Created by us via API with the OAuth token | Yes, commit status API | `Push updates arrive by webhook, added to the repository as "uses.ink".` |
| GitLab | Created by us | Yes, commit status API | Same. |
| Self-hosted Forgejo/GitLab | Same, after the instance URL is entered | Yes | `Instance: git.example.org` field before OAuth (one extra typed field, clicks still 4). |

Creating a webhook is a write to repository settings. The permission note on these
forges reads: `uses.ink can read this repository, set commit status, and add one
webhook. It cannot change files.`

### 2.4 Bare git URL path

| Step | Click | Screen | Behind |
|---|---|---|---|
| 1 | | Landing, paste `https://git.example.org/me/notes.git` | |
| 2 | 1 | `Preview` | Anonymous shallow clone into a sandbox. No account yet. |
| 3 | | Preview screen (same layout as 2.2 step 4, minus Address) | Interpret and Render to a private preview URL valid for 24 h. |
| 4 | 2 | `Publish this` | Asks for an account: email field. |
| 5 | 3 | `Send link` | Magic link email. Person clicks it (click 4, in the mail client). |
| 6 | | Prove control screen | Waits for proof. |

The preview exists so the person sees their content rendered before any account
exists, which is the fastest honest demo. It is unlisted, `noindex`, and expires.
Copy on the preview banner:

```
Preview, not published. Expires 2026-11-27 15:40 UTC.
Only people with this link can see it. It is not indexed.
```

Prove control screen:

```
Prove you control git.example.org/me/notes

Pick one. Both prove the same thing.

1. Commit a file (works for any host)
   Add this file to main and push:
     path     .well-known/uses-ink
     content  uses-ink-verify=7Qm2-kd81-Zp0x

2. Add a DNS record (if you own git.example.org)
     type   TXT
     name   _uses-ink.git.example.org
     value  uses-ink-verify=7Qm2-kd81-Zp0x

Checking every 30 s for 1 h. Last check 15:41:02: file not found on main.
[ Check now ]
```

Behind: the file check is a fresh snapshot of the branch; the DNS check queries
authoritative servers, not a cache, and says which server answered.

Capability line on publish, because a bare remote has no webhook or commit status
API we can use:

```
This source has no push notifications. uses.ink checks for new commits every 5 min.
To publish on push instead, add this webhook to your host:
  POST https://uses.ink/hook/s_91ka   secret: shown once, [reveal]
Build status cannot be sent back to this host. See it with `ink status` or here.
```

Failure states:

| Code | Trigger | Copy | Fix |
|---|---|---|---|
| `source.unreachable` | DNS, TCP or TLS failure | `Could not reach git.example.org: connection refused on port 443.` | `Check the URL. For SSH-only hosts, use the CLI: ink push.` |
| `source.auth_required` | 401/403 on clone | `git.example.org asked for credentials. Private bare remotes need a deploy key.` | `[ Add a deploy key ]` (see 5.6) |
| `source.not_git` | URL serves HTML | `https://git.example.org/me/notes is a web page, not a git remote. Try the URL ending in .git.` | Field stays filled |
| `source.ref_missing` | Default branch empty | `The remote has no default branch. Name one: [ main ]` | inline |
| `verify.file_mismatch` | File present, wrong content | `.well-known/uses-ink on main contains "uses-ink-verify=old1", expected "uses-ink-verify=7Qm2-kd81-Zp0x".` | copy button |
| `verify.dns_mismatch` | Different TXT | `_uses-ink.git.example.org has TXT "x". Expected "uses-ink-verify=7Qm2-kd81-Zp0x". Answer from ns1.example.org.` | |
| `verify.timeout` | 1 h with no proof | `Stopped checking after 1 h. Nothing was published. The preview stays until 15:40 tomorrow.` | `[ Check again ]` restarts |

### 2.5 `ink` CLI path

```
$ ink push
Not signed in. Open https://uses.ink/device and enter WXKD-3QPL
Waiting for approval... approved as cstef (github).

No site linked to this folder.
Create cstef.uses.ink from /home/cstef/notes? [Y/n]
```

Device flow (OAuth device authorization grant) works over SSH and in containers. The
prompt is skipped with `--yes` and refused when stdin is not a TTY and `--yes` is
absent (`cli.needs_confirmation`, exit 2).

```
Linked /home/cstef/notes to site cstef.uses.ink (written to .uses/site).
Reading 41 files (4.2 MB)... uploading 41 new.
  done   Snapshot   41 files                        0.6 s
  done   Interpret  23 pages, 31 decisions, 2 warnings
  done   Render     23 pages                        1.8 s
  done   Publish    deploy d_7f3k                   0.3 s

https://cstef.uses.ink  deploy d_7f3k  4.1 s

warnings
  date.undated  posts/a.md  no date in frontmatter; this source has no history
  date.undated  posts/b.md  no date in frontmatter; this source has no history
Run `ink explain` to see all 31 decisions.
```

Behind: content-addressed upload (only new blobs go up). The CLI source has `snapshot`,
and `history` only when the folder is a git or jj working copy, in which case dates
come from the local log and the CLI says `history from local git log (main)`. A plain
folder has no history and the warning says so.

`.uses/site` holds only the site id, no credential. Credentials live in the OS keyring,
with a file fallback that the CLI announces: `No keyring found. Token stored in
~/.config/uses/token (mode 600).`

### 2.6 Publishing a vault or notes folder

#### What changes when the source is a vault

A source is treated as a vault when it has a `.obsidian/` folder, or when more than a
fifth of its links are wikilinks. The detection is a decision like any other:
`Shape: vault (found .obsidian/)`, override `shape = "blog"`.

| | Repo of posts (default) | Vault |
|---|---|---|
| What is published on first run | Everything not excluded by a rule, then reviewed after (P4) | **Nothing until the person picks.** The first publish shows the note list and waits. |
| Selection rule after the first run | Rules (posts folder, drafts, excludes) | The notes chosen, recorded in the vault (see below), plus a rule for new notes |
| Site shape | Blog: home, dated posts, feed | Garden when no folder has dated notes: home note, pages, backlinks, tag pages, recently changed list. Blog shape when a dated folder exists. Both feeds are generated either way (recently changed for gardens). |
| Link syntax | Markdown links | Read from `.obsidian/app.json`: `useMarkdownLinks`, `newLinkFormat`, `attachmentFolderPath` |

Why a vault waits for a selection: a vault holds journals, work notes and people's
names next to the three notes meant for sharing. Publishing the lot and asking after
breaks J3 on the first click. Repos of posts were written to be read; vaults were not.

#### Where the selection lives

The selection must survive leaving uses.ink (no lock-in), so it is stored **in the
vault**, never only on our side. Two forms, both read, the first one wins:

1. Frontmatter on each note: `publish: true` or `publish: false`. The key is plain
   `publish`, which Obsidian Publish also reads `[unverified]`, so notes already
   marked for Obsidian Publish come over with no edits. No `uses-` prefix.
2. Patterns in `ink.toml`: `include = ["Projects/Public/**"]`, `exclude = ["Journal/**"]`.

The browser and plugin pickers write one of these, and say which before writing:
`Selected 12 notes. This adds "publish: true" to their frontmatter.` or, if the person
prefers not to touch notes: `Saved as 3 include patterns in ink.toml at the vault root.`
Default: frontmatter, because it moves with the note when files are renamed.

New notes default to unpublished in a vault (`new_notes = "private"`). The Site screen
says so: `New notes are not published until marked publish: true.`

#### Path A: browser, no install

For a note-taker who has a folder and a browser and nothing else.

| Step | Click | Screen | Behind |
|---|---|---|---|
| 1 | 1 | Landing, `Choose a folder` | `<input type=file webkitdirectory>`. Without JS: a plain multi-file input with the note `Your browser can only send files, not folders. Zip the folder and choose the zip.` |
| 2 | 2 | Browser's folder dialog, `Upload` (the browser asks to confirm) | Files are read **locally**; the page lists them. Nothing is sent yet. |
| 3 | 3 | Note picker, `Publish 12 notes` | Only the picked notes and the attachments they embed are uploaded. |
| 4 | 4 | Email field, `Send link` | Magic link. |
| 5 | 5 | Magic link in the mail client | Handle picked (prefilled from the vault folder name if free), deploy runs. |

Note picker, step 3:

```
Your vault: Garden  (1,284 notes, 312 attachments, found .obsidian/)

Nothing is uploaded until you choose. Pick notes or folders to publish.

[ ] Journal/                 611 notes
[ ] Work/                    402 notes
[x] Public/                   11 notes
[x] Home.md                          used as home page
    Already marked publish: true in frontmatter: 1 (Public/Reading list.md)

12 notes selected. They link to 9 notes you did not select; those links will show as
plain text, not links. [ show which ]
They embed 7 attachments, which will be uploaded with them.

[ Publish 12 notes ]
```

The 9-notes line is the important one: it names the leak surface (link text from
private notes appears in published notes as plain text) before anything leaves the
machine.

A browser upload is a CLI source without the CLI: capability `snapshot` only, no
history, no watch. The Site screen says: `This site was uploaded from a browser. It
changes only when you upload again or connect the Obsidian plugin.` The upload screen
offers the two ways to stay in sync: the plugin, or putting the vault in git.

#### Path B: Obsidian plugin (everyday path for note-takers)

The plugin is a thin client over the same API as the CLI: it uploads, shows the same
diagnostics with the same codes, and keeps no settings of its own beyond the sign-in.

| Step | Click | Where | Behind |
|---|---|---|---|
| 0 | | Settings, Community plugins, install and enable "uses.ink" | Obsidian's own flow, not counted. |
| 1 | 1 | Ribbon icon or command `uses.ink: Publish` | Opens the plugin pane. |
| 2 | 2 | `Sign in` | Opens `uses.ink/device` in the browser with the code prefilled; email or forge sign-in. |
| 3 | 3 | Note picker (same as Path A, rendered in the pane) | Shows notes already marked `publish: true` as selected. |
| 4 | 4 | `Publish 12 notes` | Writes the frontmatter flags, uploads, shows deploy progress in the pane. |

Everyday use: the pane lists `Changed since last publish: 2 edited, 1 new (not
selected)` with a `Publish changes` button, the same change list Obsidian Publish
shows. A status bar item reads `uses.ink: published 14:02` or `uses.ink: 2 changes not
published`, in words, not a coloured dot. Right-click on a note: `Publish note` /
`Unpublish note`. Diagnostics open the note at the line.

Auto-publish on save is off by default, because notes are saved every few seconds and a
half-written note would go live. Knob in `ink.toml`: `publish.on = "save"` with a
debounce, shown in the pane when on.

#### Path C: vault already in git

Many vaults are synced with the Obsidian Git plugin to GitHub or similar. This is the
GitHub path (2.2) with the vault rules applied: the Publish screen becomes the note
picker, and publishing commits nothing; the selection is written to frontmatter only if
the person asks the plugin or CLI to (the dashboard never writes to the repository,
see Q10). From the dashboard, the selection of a git vault is therefore by pattern:

```
github.com/me/vault looks like an Obsidian vault (.obsidian/ found).
Nothing is published until you choose.

Notes already marked publish: true: 0.
Choose folders: [x] Public/  [ ] Journal/  [ ] Work/

The dashboard cannot write to your repository. To keep this choice, add to ink.toml:
  include = ["Public/**"]
[ Copy ]   [ Publish now with this selection ]
```

`Publish now` works before the line is committed; the Site screen then shows
`Selection is held by uses.ink until ink.toml includes it. Commit the line above to
make it part of your vault.` This is the one setting held outside the source, and it is
labelled as such until the commit lands.

#### Path D: CLI in a vault

```
$ cd ~/Garden && ink push
Found .obsidian/: treating this folder as a vault.
Nothing is published until you choose. 1,284 notes, 1 marked publish: true.

Publish which? Enter paths or globs, empty to stop:
> Public/**
12 notes selected (11 from Public/**, 1 from frontmatter).
They link to 9 unselected notes; those links render as plain text (ink explain --links).
Record the selection as: [f]rontmatter on 12 notes, [t]oml include pattern, [c]ancel? t
Wrote include = ["Public/**"] to ink.toml.
  done   Snapshot   12 notes, 7 attachments    0.4 s
  ...
https://garden-me.uses.ink  deploy d_2k9q  3.0 s
```

Non-interactive: `ink push --include 'Public/**' --record toml --yes`.

#### Why the non-terminal path is two things, not one

| Option | Verdict |
|---|---|
| Desktop app | No. A second editor-adjacent app to install for something Obsidian already hosts. |
| Browser upload only | Not enough: one-shot, no sync, re-upload is a chore. Kept as the no-install first run. |
| Obsidian plugin only | Excludes notes folders kept in other editors (Logseq exports, iA Writer, plain folders). |
| **Browser upload for the first run, Obsidian plugin for the loop** | Chosen. Both speak the same API as the CLI, so nothing is plugin-only. Other editors use the browser path or git. |

## 3. The "What we understood" screen

URL: `/<site>/understood`. CLI: `ink explain`. Same data, same order.

### Layout

One table per kind of decision, ordered by how often each one turns out wrong
(dates and titles first, theme last). Each row: what, value, from, override. Flagged
rows on top of their table with the word `check` and a glyph.

```
What we understood                        deploy d_7f3k, main @ a1b2c3d

Source       github.com/cstef/blog, branch main, folder /
Capabilities snapshot, history, watch (GitHub App webhook)
             not private (public repository)

Summary: 23 pages published, 18 files excluded, 31 decisions, 2 to check.
[ Show only decisions to check ]   [ Copy config that pins all of this ]
```

#### Files

```
Published (23)           Rule
README.md                home-page
about.md                 page
posts/rust-allocators.md post (in posts/)
...
Excluded (18)            Rule
LICENSE                  repo-meta: LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, CHANGELOG, SECURITY
.github/**               dotfiles
drafts/next.md           draft: folder named drafts
posts/wip.md             draft: frontmatter draft: true
node_modules/**          vendored
notes/2023.md            outside posts/, not linked from any page   check
```

The `notes/2023.md` row is the J3 case: markdown that is neither a post nor linked
from a page. **Decision: orphan pages are not published by default** and are flagged.
This is the one default that departs from "render everything" and the reason is
privacy; a repo of notes often has files nobody meant to publish. Override:
`include = ["notes/**"]`. In a vault the orphan rule is replaced by the selection
(2.6): unselected notes are never published, linked or not.

#### Pages and posts

| Decision | Inferred value | From | Override |
|---|---|---|---|
| Home page | `README.md` | First of `index.md`, `README.md`, `readme.md` at the folder root | `home = "about.md"` |
| Posts folder | `posts/` | Folder with the most files that have a date (19 of 19) | `posts = "writing/"` |
| Post order | date, newest first | Default | `order = "title"` |
| Index page | Generated list under the home page content | Home page has no list of posts | `index = "none"` or `index = "page:archive.md"` |
| Nav | `About` (about.md) | Top-level pages that are not posts, alphabetical | `nav = [{ title = "About", path = "about.md" }]` |

#### Per-post fields

Shown as one table with a row per post; columns Title, Date, Updated, Author, each cell
with a short source tag. Hovering is not needed: the tag is text.

```
Post                     Title                      Date                       Updated
posts/rust-allocators.md Rust allocators (h1)       2025-03-14 (first commit)  2025-06-02 (last commit)
posts/typst.md           Typst for notes (front)    2024-11-01 (front)         2025-01-09 (last commit)
posts/old/x.md           x (file name)  check       2023-02-10 (first commit)  same
```

Tag vocabulary, fixed and documented:

| Tag | Meaning |
|---|---|
| `front` | frontmatter field |
| `h1` | first level-1 heading, which is then not repeated in the body |
| `file name` | file name, hyphens to spaces, first letter capitalised |
| `first commit` / `last commit` | from history; commit id on click |
| `file name date` | `2025-03-14-slug.md` pattern |
| `undated` | none of the above; post sorts last and shows no date |

Date order of evidence: frontmatter `date`, then file name date, then first commit
that added the file (following renames), then undated. Never the import time.

Authors: frontmatter `author`, then commit author name, then the handle. If commits
have several authors, the first one, and the row says `3 authors in history, showing
first`.

#### Drafts

Rules shown with counts: `draft: true` or `published: false` in frontmatter; any path
segment named `drafts`; file names starting with `_`. Each rule can be turned off.
Drafts are built into branch previews (4.3), never into the live site.

#### Images and links

| Decision | Value | From | Override |
|---|---|---|---|
| Relative images | resolved against the file, served from the snapshot | Default | none needed |
| Missing images | 1: `posts/rust.md:12 ./img/ferris.png` | `image.missing` | fix the path |
| Remote images | left remote, 3 hosts listed | Default | `images.remote = "proxy"` |
| Links to `.md` files | rewritten to page URLs | Default | none |
| Broken internal links | 2, listed with file and line | `link.broken` | fix the link |

#### Links, embeds and backlinks (vaults and wikilink sources)

Shown whenever the source uses wikilinks, not only for detected vaults.

| Decision | Inferred value | From | Override |
|---|---|---|---|
| Wikilinks `[[Note]]`, `[[Note\|alias]]`, `[[Note#Heading]]`, `[[Note#^block]]` | resolved, 214 found | Detected syntax | `links.wiki = false` |
| Resolution | by shortest unique path, as Obsidian does: `[[Ideas]]` finds `Public/Ideas.md` | `.obsidian/app.json` `newLinkFormat: shortest` | `links.resolve = "relative"` or `"absolute"` |
| Ambiguous names | 2: `[[Index]]` matches `Public/Index.md` and `Work/Index.md`; picked the published one | `link.ambiguous` | write the path: `[[Public/Index]]` |
| Aliases | 6 notes have `aliases:`; links to an alias resolve to the note | frontmatter | none |
| Links to unpublished notes | 9, rendered as plain text, no link, no title lookup | Vault default | `links.unpublished = "drop"` removes the text too |
| Links to missing notes | 3, rendered as plain text, flagged | `link.missing` | create the note or fix the name |
| Note embeds `![[Note]]` | 4, inlined as a quoted block with a link to the source note; only if that note is published | Detected syntax | `embeds.notes = "link"` (show a link instead) |
| Embed of an unpublished note | 1: `Public/Weekly.md:5 embeds Journal/2026-11-20`; **not inlined**, shown as `Embedded note not published.` | `embed.unpublished` | publish the note or remove the embed |
| Heading and block embeds `![[Note#H]]`, `![[Note#^id]]` | inlined section only | Detected syntax | same |
| Attachment embeds `![[photo.png]]`, `![[paper.pdf]]`, `![[clip.mp4]]` | 7, looked up in `Attachments/` then by name; images inline, PDF and media as links with size | `.obsidian/app.json` `attachmentFolderPath` | `attachments = "Files/"` |
| Image size `![[photo.png\|300]]` | width honoured | Detected syntax | none |
| Backlinks | listed at the end of each page, **from published notes only**, with the linking sentence | Vault default | `backlinks = false` |
| Graph view | not generated | Not supported (needs JS) | none; see "does not do" |
| Callouts `> [!note]` | rendered as labelled asides, label is text so it reads without colour | Detected syntax | none |
| Tags `#tag` and `tags:` | tag pages at `/tags/<tag>`, 14 tags; nested `#a/b` as `/tags/a/b` | Detected syntax | `tags = false` |
| Highlights `==x==`, comments `%%x%%` | highlights as `<mark>`; **comments stripped and never published** | Obsidian syntax | none |
| Dataview, Templater, Excalidraw, `.canvas` | not run; Dataview blocks shown as their source with a note; `.canvas` and Excalidraw files excluded | `syntax.unsupported`, rule `unsupported-format` | none; listed in "does not do" |
| Math `$x$`, Mermaid | math rendered to MathML at build; Mermaid rendered to SVG at build | Detected syntax | `math = false` |

The privacy rule the vault rows share: **nothing from an unpublished note reaches a
published page by inference.** Not its title (a link renders its own written text,
which is already in the published note), not its embedded body, not a backlink from it.
`%%comments%%` count as unpublished text.

The Files table gets a third group for vaults, `Reachable but not selected (9)`, listing
the private notes that selected notes link to, so the person can decide one by one.

#### Overriding

Each "Override" cell is a copy button plus the exact line. There is no override form in
the dashboard. **Decision: overrides live only in the config file in the source**, so
the repo stays the single source of truth and the CLI and dashboard cannot disagree.
The screen says so once at the top: `To change a decision, add the line shown to
ink.toml in your repository and push. uses.ink does not store settings outside your
source, except the address and domain.`

The exceptions (address, custom domain, which source and branch) are account settings,
because they decide where the content is read from and served, not how it is read.

#### The pinning snippet

`[ Copy config that pins all of this ]` produces a file that reproduces the current
decisions exactly, so a later change in inference rules cannot move anything. It
includes only the values that were inferred, with the source as a comment.

```toml
# ink.toml, generated from deploy d_7f3k on 2026-11-26.
# Every line pins a value uses.ink inferred. Delete a line to let it infer again.

home  = "README.md"            # first of index.md, README.md
posts = "posts/"               # 19 of 19 dated files
order = "date"
index = "generated"
nav   = [{ title = "About", path = "about.md" }]

[drafts]
frontmatter = true             # draft: true, published: false
folders     = ["drafts"]
underscore  = true

[dates]
from = ["front", "file name", "first commit"]

exclude = ["notes/**"]         # orphan pages, not linked from any page

[[pages]]
path  = "posts/old/x.md"
title = "x"                    # from file name; you probably want to set this
```

A second button, `[ Copy minimal config ]`, gives only the lines for flagged decisions.
CLI: `ink explain --pin > ink.toml` and `ink explain --pin --minimal`.

## 4. Everyday loop

### 4.1 Write and push

The person writes in their editor and pushes. Nothing in the loop requires the
dashboard.

### 4.2 Seeing the update

| Surface | What appears | When |
|---|---|---|
| Forge (GitHub check run, Forgejo/GitLab commit status) | `uses.ink / publish` check: `pending`, then `success: 23 pages, deploy d_8a1m` with link, or `failure: 1 error` with annotations on the exact file and line | Within 5 s of push |
| CLI | `ink status` (last deploy), `ink watch` (follows the next deploy and exits with its result) | On demand |
| Dashboard | Site screen top line | Live |
| Email | Only on failure of an automatic deploy, and only the first failure in a row; the fix email after it says `Fixed by a1b2c3d`. Off switch on the Account screen. | |
| Feed for the owner | `/<site>/deploys.atom`, private token URL | Optional |

GitHub check run summary (Markdown rendered by GitHub):

```
Published 23 pages in 3.2 s. Deploy d_8a1m.
Changed: posts/new.md (added), about.md (edited).
2 warnings: date.undated posts/a.md, image.no_alt posts/new.md:8
https://cstef.uses.ink/posts/new
```

Warnings become annotations with level `warning`; errors with level `failure`.

`ink push` from a CLI source and a git push behave the same from here on.

### 4.3 Previews of a branch or draft

- Every non-default branch push builds a preview at
  `<branch-slug>--<handle>.uses.ink` (single-level wildcard, so one certificate covers
  all). Drafts are included and labelled `draft` on the page.
- On GitHub, the check run links the preview. On a PR, the check is enough; **no bot
  comment** by default (comments are noise the person did not ask for). Knob:
  `previews.comment = true`.
- CLI: `ink push --preview` builds from the working tree without committing.
  Output ends with the preview URL and its expiry.
- Previews are public to anyone with the URL, `noindex`, and expire 7 days after the
  last push to the branch. For private sources, previews require sign-in as an
  identity linked to the handle. The banner on every preview page says which:

```
Preview of branch new-post @ c3d4e5f. Includes 2 drafts. Not indexed.
Anyone with this link can see it. Expires 2026-12-03.
```

Decision: **default-branch only goes live.** Knob: `publish.branch = "release"`.

### 4.4 Rollback

Every deploy is immutable and keeps its URL `d-<id>--<handle>.uses.ink` for 90 days.
Rollback points the live address at an older deploy.

Rolling back **pauses automatic publishing**, because otherwise the next push would
silently undo the rollback. The Site screen then shows a standing line:

```
Live is pinned to deploy d_7f3k (rolled back 2026-11-26 16:02 by cstef).
New pushes build previews but do not go live.  [ Resume: publish latest ]
```

The forge check for pushes during a pause says `success: built d_9b2n, not published
(live is pinned to d_7f3k)`.

```
$ ink rollback d_7f3k
Live is now d_7f3k (main @ a1b2c3d, 2026-11-25 10:14).
Automatic publishing is paused. Run `ink resume` to publish the latest commit.
```

## 5. Secondary flows

### 5.1 Claiming and changing a handle

- Claimed at first sign-in from the identity's login (2.2). Email-only accounts pick
  one on the publish screen.
- Change: Account screen, field plus `Change handle`. One confirm that states the
  consequence:

```
Change cstef to cs?
- cs.uses.ink serves your site immediately.
- cstef.uses.ink redirects (301) to cs.uses.ink for 12 months, then stops working.
- Nobody else can claim cstef during those 12 months.
- Feeds keep working through the redirect; most readers follow it, some do not.
[ Change handle ]
```

- Custom domains are unaffected and the screen says so if one exists.

### 5.2 Linking a second identity

Account screen, `Identities`. Each row: provider, login, linked date, what it is used
for (`sign-in`, `source access`, `authority for github.com/cstef/blog`).

- `Link another` lists GitHub, Codeberg, GitLab, Forgejo instance, ATproto DID, email.
- Linking an identity already linked to another handle fails with `identity.in_use`:
  `This GitHub account is linked to handle other. Unlink it there first, or sign in as
  other.`
- Unlinking the last sign-in identity is refused: `identity.last: This is the only way
  to sign in to cstef. Link another identity first.`
- Unlinking an identity that is the authority for a live source shows: `Unlinking stops
  updates to cstef.uses.ink from github.com/cstef/blog. The last deploy stays live.`

### 5.3 Custom domain

Settings, `Domains`, `Add domain`. One field. The page then shows the records and a
state that the server re-checks every 30 s while the page is open and every 10 min
otherwise.

```
blog.cstef.dev

Add this record at your DNS provider:
  type   CNAME
  name   blog
  value  cstef.uses.ink

State: waiting for DNS. Last check 16:10:04, resolver 1.1.1.1: NXDOMAIN.
[ Check now ]
```

Apex domains get `A` and `AAAA` records instead, plus a note: `If your provider supports
ALIAS or ANAME, use that pointing at cstef.uses.ink instead; it follows our address if
it changes.`

| State | Meaning | Copy | Action |
|---|---|---|---|
| `waiting_dns` | No record yet | `waiting for DNS. <resolver>: <answer>` | Check now |
| `dns_wrong` | Record points elsewhere | `blog.cstef.dev points to 185.199.108.153 (GitHub Pages?), expected cstef.uses.ink.` | shows old record to remove |
| `dns_conflict` | CAA forbids our CA | `A CAA record on cstef.dev allows only "digicert.com". Add: 0 issue "letsencrypt.org".` | |
| `issuing` | DNS ok, ACME in progress | `DNS is correct. Requesting a certificate from Let's Encrypt, usually under 1 min.` | |
| `issue_failed` | ACME error | `Let's Encrypt refused: <their error text>. Next attempt 16:40.` | Retry now |
| `active` | Serving | `active. Certificate valid until 2027-02-24, renews 30 days before.` | |
| `redirecting` | Primary domain set | `cstef.uses.ink redirects to blog.cstef.dev.` | toggle |
| `degraded` | DNS changed after active | `DNS for blog.cstef.dev no longer points here (since 2026-12-01). Serving stopped on this domain; cstef.uses.ink still works.` | |
| `renewal_failing` | Renewal failed, cert still valid | `Renewal failed 3 times. Certificate expires 2027-02-24.` email once | |

Decision: once a custom domain is `active`, it becomes the canonical URL (in feeds,
`<link rel=canonical>`) and `<handle>.uses.ink` redirects to it. Knob: `Primary
address` radio. This is stated next to the radio.

### 5.4 Switching or adding a source

A site has one or more sources, each **mounted** at a path. One source at `/` is the
default. Settings, `Sources`:

```
Sources
  /       github.com/cstef/blog   main   snapshot, history, watch
  /notes  folder via ink push    none   snapshot (no history: dates from frontmatter)
[ Add source ]
```

- Add: same chooser as the landing, then a mount path field. Overlapping paths fail with
  `source.mount_conflict: /notes is already served by github.com/cstef/notes.`
- Switch (replace the source at `/`): builds a preview from the new source and shows a
  diff before going live: `14 pages unchanged, 3 added, 6 removed (their URLs will 404),
  2 changed URL`. Removed URLs offer `[ Add redirects ]`, which copies a `redirects`
  block for `ink.toml`.
- Remove: `Removing /notes deletes its 9 pages from the live site at the next deploy.
  The folder on your machine is not touched.`

### 5.5 ATproto source

Sign in with a handle or DID, pick a collection (`com.whtwnd.blog.entry`,
`pub.leaflet.document`, `site.standard.*` `[unverified lexicon names]`). Capabilities:
`snapshot`, `watch` (firehose or Jetstream), `history` partial (record `createdAt` only).
The capability line: `Dates come from each record's createdAt. Edit history is not
available from this source.`

### 5.6 Private sources

- GitHub/GitLab/Forgejo: access comes through the same app or token; capability
  `private` is shown on the source row.
- Bare remote: a deploy key generated by us (Ed25519, public half shown, private half
  never displayed), or a read token the person pastes through a secret field.
- What is honest to say on the source row:
  `Private repository. Published pages are public. Only files listed under Published
  are served; the rest of the repository is read to build but never served.`
- Optional: whole site behind sign-in (identities linked to the handle) for notes
  shared with yourself. Default off. If on, feeds are disabled and the site screen says
  so.

### 5.7 Theme

Settings, `Theme`: a list of themes, each previewed with **the person's own home page
and newest post**, not lorem ipsum. Choosing one shows the line to add, consistent with
3: `theme = "plain"`. The dashboard can apply it only by telling the person what to
commit; no hidden setting. CLI: `ink theme list`, `ink theme preview <name>` (opens a
preview URL).

Theme scope for now: a fixed set, chosen by name, with documented CSS variables
overridable in `ink.toml`. Custom templates are an owner question (9).

### 5.8 Export

Account, `Export`. One button, one archive:

```
Export cstef
Includes:
- every published deploy's built HTML, feeds and assets (static, serve with any web server)
- ink.toml as inferred for the latest deploy (pinned, see What we understood)
- decisions.json for every deploy
- account.json: handle, identities (no tokens), domains, sources
Does not include: your content as a separate copy. It is already in your sources.
[ Build export ]  Ready in about 30 s; the link stays valid for 24 h.
```

CLI: `ink export -o cstef.tar.zst`.

### 5.9 Account deletion

Account, `Delete account`. One confirm, which requires typing the handle because the
action cannot be undone:

```
Delete cstef

This deletes now:
- the site cstef.uses.ink and 47 deploys (pages return 410 Gone)
- custom domain blog.cstef.dev (remove the DNS record yourself)
- snapshots and caches of 2 sources
- 3 linked identities and their tokens (revoked at GitHub and Codeberg)
- the GitHub App installation on cstef

This does not touch:
- your repositories and folders
- copies readers or feed readers already fetched

Backups holding this data are overwritten within 30 days.
The handle cstef stays reserved for 12 months, then anyone can claim it.

Type cstef to confirm: [        ]   [ Delete account ]
```

After: `Deleted cstef. 47 deploys, 2 snapshots and 3 identities removed. Tokens revoked
at GitHub (ok) and Codeberg (ok).` If a revocation fails it says which one and links to
the provider's page to revoke by hand.

### 5.10 What readers see

| Feature | Behaviour |
|---|---|
| HTML | Server-rendered, works with JS off, no cookies, no third-party requests by default |
| Feeds | Atom at `/feed.xml`, JSON Feed at `/feed.json`, both full-content, linked with `<link rel=alternate>`; per-tag feeds when tags exist |
| Reading view | Default theme is the reading view: single column, 60 to 75 characters, respects `prefers-color-scheme` and `prefers-reduced-motion` |
| Raw | `/<path>.md` returns the source markdown (knob `raw = false`) |
| Source link | Footer `Source: github.com/cstef/blog/posts/x.md @ a1b2c3d` (knob) |
| Updated | `Updated 2025-06-02` shown only when it differs from the date by more than a day |
| 404 | `No page at /posts/rustt. Closest: /posts/rust.` plus the index. No joke. |
| 410 | For removed pages after a source switch or deletion: `This page was removed on 2026-12-01.` |
| Sitemap, robots | Generated; `noindex` on previews |

## 6. States catalogue

| State | Trigger | Exact copy | Action |
|---|---|---|---|
| Empty: no sites | Signed in, nothing published | `No site yet. Pick a source to publish from.` | source chooser inline |
| Empty: no posts | Source has pages but no dated files | `No posts found: no folder has dated files. The home page and 3 pages are published.` | `posts = "..."` line |
| Empty: no deploys in history | Site created, first build failed | `No deploy is live. The first build failed: <code>.` | open diagnostic |
| Empty: no domains | | `No custom domain. The site is at cstef.uses.ink.` | Add domain |
| Loading: snapshot | Fetching | `Reading <source>, <n> files so far...` | none, with elapsed time |
| Loading: slow | Stage over 30 s | `Render has taken 34 s, longer than usual (p95 6 s). Still running.` | `[ Cancel deploy ]` |
| Queued | Another deploy running | `Queued behind d_8a1m, which started 12 s ago.` | |
| Superseded | Newer push arrived mid-build | `Cancelled: superseded by c3d4e5f.` | |
| Error: build | Render error | `<code> <file>:<line> <what>. <fix>. The live site is unchanged (d_7f3k).` | open file on forge |
| Error: config | Bad `ink.toml` | `config.invalid ink.toml:7 posts must be a folder path, got 3. The live site is unchanged.` | |
| Error: config key unknown | Typo | `config.unknown_key ink.toml:4 "post" is not a setting. Did you mean "posts"?` | Warning, deploy continues |
| Error: auth expired | Token revoked on forge | `uses.ink lost access to github.com/cstef/blog on 2026-11-30 (installation removed). The last deploy stays live; pushes are not published.` | `[ Reconnect ]` |
| Degraded: forge down | Webhook not arriving / API 5xx | `GitHub is not answering since 14:02. Pushes after that time will publish when it recovers. Last checked 14:20.` | |
| Degraded: watch fallback | Webhook failing, polling instead | `Webhook deliveries are failing (last: 404 at 13:50). Checking for commits every 5 min instead.` | link to fix webhook |
| Degraded: capability missing | No history | `This source has no history. Dates come from frontmatter or file names.` | |
| Degraded: quota | Over a limit | `This site has 5,214 pages. The limit is 5,000. The deploy was not published; the live site is d_7f3k.` | `[ Choose folder ]` / limits page |
| Degraded: service | uses.ink incident | `Builds are delayed by about 10 min (incident i_23, started 15:01). Live sites are served normally.` | status page |
| Paused | After rollback | see 4.4 | Resume |
| Warning: no alt | Image without alt | `image.no_alt posts/x.md:8 has no alt text. Screen readers will skip it.` | |
| Warning: orphan | Markdown not linked, not a post | `page.orphan notes/2023.md is not linked from any page, so it is not published.` | `include` line |
| Warning: build output | Hugo/Jekyll detected | see 2.2 | |
| Warning: heading skipped | h1 then h3 | `a11y.heading_skip posts/x.md:20 jumps from h1 to h3.` | |
| Retry visible | Transient fetch failure | `Fetch failed (timeout after 20 s). Retry 2 of 5 in 8 s.` | Retry now |
| Reader: page gone | 410 | see 5.10 | |
| Vault: nothing selected | Vault source, no `publish: true`, no include | `This vault has 1,284 notes and none are selected, so nothing is published. Pick notes to publish.` | note picker |
| Vault: selection held | Git vault, picked in dashboard, not committed | `Selection is held by uses.ink until ink.toml includes it.` | Copy line |
| Vault: new notes waiting | Plugin sees new notes | `3 new notes since last publish. New notes are not published until marked publish: true.` | Review |
| Warning: embed unpublished | `![[private]]` in a published note | `embed.unpublished Public/Weekly.md:5 embeds Journal/2026-11-20, which is not published. The embed shows "Embedded note not published."` | publish note or remove embed |
| Warning: ambiguous link | Two notes share a name | `link.ambiguous Public/A.md:3 [[Index]] matches 2 notes. Linked to Public/Index.md because it is published. Write [[Public/Index]] to be explicit.` | |
| Warning: unsupported syntax | Dataview block | `syntax.unsupported Public/Books.md:10 is a Dataview query. uses.ink does not run plugins; the query is shown as code.` | |
| Upload: folder too big | Browser upload over limit | `The selected notes and attachments are 1.3 GB. The upload limit is 500 MB. Deselect attachments or large folders.` | |
| Plugin: offline | Obsidian offline | `uses.ink: offline, 2 changes not published` (status bar) | Publish when back |
| Reader: site gone | Handle deleted | `cstef.uses.ink no longer exists. It was deleted by its owner.` | none |

Diagnostic page `uses.ink/d/<code>`: code, one-paragraph cause, the inputs that trigger
it, the fix, and the config key if one exists. Codes are dotted lowercase
(`source.unreachable`) and never renamed; retired codes keep their page.

## 7. CLI surface

Binary `ink`. Output rules:

- Human output is plain text, no colour when `NO_COLOR` is set or stdout is not a TTY,
  no spinners when not a TTY, stable column order, times in UTC ISO 8601 unless
  `--local`.
- `--json` on every command prints exactly one JSON document on stdout (or NDJSON events
  for streaming commands with `--json-stream`) with a `"schema": "uses.<cmd>/v1"` field,
  on failure too: a command that stops on an error prints `{"schema": "uses.error/v1",
  "exit", "error", "what", "causes"}` instead, `error` being a diagnostic code or a
  `noun.noun.reason` kind, and never a plain `error:` line. Paths that are not UTF-8 are
  written with U+FFFD in place of the bytes that are not. Diagnostics go to stderr as NDJSON
  events `{code, severity, file, line, what, why, fix}`; notices join them only with `-v`,
  as in text output.
- Progress goes to stderr, results to stdout.

### Commands

| Command | Does |
|---|---|
| `ink login` / `logout` / `whoami` | Device-flow sign-in; prints handle and linked identities |
| `ink init` | Links the current folder or repo to a site without publishing; writes `.uses/site` |
| `ink push [--preview] [--yes] [--message <text>]` | Uploads the folder (CLI source) or, in a repo with a forge source, triggers a deploy of the pushed commit and refuses unpushed changes with `push.not_pushed` |
| `ink status [<site>]` | Live deploy, latest deploy, paused or not, warnings count |
| `ink watch` | Waits for the next deploy for the current HEAD and exits with its result |
| `ink deploys [--limit n]` | History, newest first; reports `more` when truncated |
| `ink explain [--pin] [--minimal] [<path>]` | The "What we understood" data; with a path, the decisions for that file, and with `--pin` too, only that file's pins. A path that is no file of the folder exits 2 |
| `ink check` | Runs Snapshot and Interpret locally, no upload; for CI and pre-commit |
| `ink preview` | Local server rendering the working tree with the same renderer |
| `ink rollback <deploy>` / `ink resume` | 4.4 |
| `ink source list|add|remove` | 5.4 |
| `ink domain add|status|remove <domain>` | 5.3 |
| `ink handle set <new>` | 5.1 |
| `ink theme list|preview <name>` | 5.7 |
| `ink export -o <file>` | 5.8 |
| `ink account delete` | 5.9, requires `--confirm <handle>` when not a TTY |
| `uses diag <code>` | Prints the diagnostic page text |
| `ink select <glob>... [--record frontmatter\|toml]` / `uses unselect` | Vault selection; prints what it will write before writing |
| `ink explain --links` | Links, embeds and backlinks table, including links to unpublished notes |

The Obsidian plugin calls the same endpoints and prints the same codes; there is no
plugin-only feature and no CLI-only feature.

### Samples

```
$ ink status
site      cstef.uses.ink  (primary: blog.cstef.dev)
live      d_8a1m  main @ c3d4e5f  2026-11-26T15:12:40Z  23 pages
latest    d_8a1m  same as live
auto      on
warnings  2  (ink explain)
```

```
$ ink status --json
{"schema":"uses.status/v1","site":"cstef.uses.ink","primary":"blog.cstef.dev",
 "live":{"id":"d_8a1m","ref":"main","commit":"c3d4e5f","at":"2026-11-26T15:12:40Z","pages":23},
 "latest":{"id":"d_8a1m"},"auto":true,"paused":null,"warnings":2}
```

```
$ ink check
ink.toml        ok
pages            23 published, 18 excluded
decisions        31 (2 to check)
error  render.unclosed_fence  posts/x.md:40  code fence opened at line 12 is never closed
       fix: add ``` on its own line after line 39
1 error, 2 warnings
$ echo $?
1
```

```
$ ink explain posts/old/x.md
posts/old/x.md  post (in posts/)
  title    x                 from file name      check   set: title in frontmatter
  date     2023-02-10        from first commit 9e1f2aa
  updated  same
  author   cstef             from commit author
  draft    no
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Done. Warnings, if any, were printed. |
| 1 | Content error: the deploy or check failed on diagnostics. Nothing changed live, and `ink build` wrote nothing. |
| 2 | Usage error: a folder that is missing or cannot be listed, a path given wrong, an output folder `ink build` will not write into, or a confirmation was needed and not given. |
| 3 | Not signed in, or access to a source was lost. |
| 4 | Network or service unavailable after retries. Safe to retry. |
| 5 | Conflict: site paused after rollback, mount conflict, unpushed commit. |
| 6 | `--strict` was given and there were warnings. As with 1, nothing changed live and `ink build` wrote nothing. |

`ink build` reads, interprets and renders the whole site before it touches its output folder, so
exits 1 and 6 leave the previous build there as it was. Before it removes anything, it checks
every file the previous build's manifest lists: a file whose bytes changed, a link, or a file in
the way that no build wrote is refused with exit 2, and the folder is left as it was. An output
folder that is the site's folder or holds it is refused with exit 2.

## 8. Information architecture

The dashboard has **four screens**. Each answers one question.

| Screen | Question it answers | Contains |
|---|---|---|
| **Site** (`/<site>`) | Is it live, and what changed? | Live URL and primary address, live deploy, latest deploy, paused banner, capability lines, last 5 deploys, current warnings and errors. Order: state first, then the thing to fix, then history. |
| **What we understood** (`/<site>/understood`) | Why does it look like this? | Section 3. Files, decisions, pinning config. |
| **Deploys** (`/<site>/deploys`) | What happened when? | Full history, each deploy's log, diagnostics and decisions diff against the previous one, previews per branch, rollback. |
| **Settings** (`/<site>/settings`) | Where does it read from and serve to? | Sources, domains, theme (read-only plus line to commit), previews on or off. |

Plus **Account** (`/account`), outside any site: handle, identities, notifications,
export, delete.

Why this is the minimal set:

- Configuration of interpretation is not a screen, because it lives in `ink.toml`
  (decision in 3). A settings form for it would create a second source of truth.
- Build logs are not a screen; they belong to a deploy.
- Analytics are not offered, so there is no analytics screen. The Site screen says
  `No visitor analytics. Server logs are kept 7 days for abuse handling only.`
- Previews are deploys from other branches, so they live in Deploys with a branch
  filter.
- Domains could be on Account, but a domain belongs to a site, and the handle-level
  multi-site case is an open question (9).

With one site per handle (the default), `/` after sign-in is the Site screen directly;
there is no "your sites" list to click through.

## 9. Open questions for the owner

Each has a default I would ship and the knob that changes it.

| # | Question | Default | Knob |
|---|---|---|---|
| Q1 | Render a public repo before its owner claims it, as the old uses.ink did? | No. Only previews from the bare-URL path (24 h, unlisted). Nothing at `<handle>.uses.ink` without an authority. | Allow anonymous previews longer, or not at all |
| Q2 | Orphan markdown (not a post, not linked) published? | No, flagged as `page.orphan` (privacy) | `include` patterns, or flip the default site-wide |
| Q3 | Config file name and format | `ink.toml` at the source root, one file, no per-directory files | `.ink.toml`, `uses.ink.json` for continuity with the old version |
| Q4 | One site per handle, or many? | One site, many sources mounted at paths | Allow `<site>--<handle>.uses.ink` for extra sites |
| Q5 | Handle change and deletion: how long is the old name held? | 12 months redirect and reservation | Shorter, or permanent tombstone |
| Q6 | A broken page fails the whole deploy? | Yes, all or nothing | `publish.partial = true`: skip failing pages, publish the rest, list them |
| Q7 | Previews public-by-URL for public sources? | Yes, unlisted, 7 days | Sign-in required for all previews |
| Q8 | Rollback pauses auto-publish? | Yes, until `resume` | Rollback until next push |
| Q9 | Themes: fixed set, or custom templates from the repo? | Fixed set plus CSS variables | `theme.path = "theme/"` with a template language (which one decides lock-in) |
| Q10 | Dashboard writes to the repo (e.g. "commit this ink.toml for me")? | Never; the landing promises it | Opt-in PR creation with `contents: write`, separately granted |
| Q11 | Email: failure notices on by default? | On, first failure and its fix only | Off by default |
| Q12 | Raw `.md` and source link in the reader footer | Both on | Off per site |
| Q13 | Limits shown on the landing (pages, MB, deploys per hour) | Show exact numbers on a limits page linked from the landing | Hide until hit |
| Q14 | Backup retention after deletion | Overwritten within 30 days, stated | Whatever the hosting setup can honestly promise |
| Q15 | Self-host parity: do the GitHub App, commit status and previews work self-hosted? | Yes, with the operator registering their own GitHub App; the self-host docs list each forge integration's setup | Ship self-host with polling only at first, and say so |
| Q16 | Vault selection default: opt-in per note, or publish everything reachable from the home note? | Opt-in (nothing until chosen), new notes private | `new_notes = "public"`, or `select = "reachable-from-home"` |
| Q17 | Where the picker records the selection by default | Frontmatter `publish: true` (moves with renames, matches Obsidian Publish `[unverified]`) | `ink.toml` patterns, so notes are never edited |
| Q18 | Ship the Obsidian plugin at launch, or browser upload first? | Both at launch; the plugin is the everyday loop for J6 | Browser upload only, plugin later |
| Q19 | Garden shape: backlinks and tag pages on by default; graph view never (needs JS)? | Yes | Offer a JS graph as an opt-in enhancement that degrades to the link list |
| Q20 | Allow a dashboard-held vault selection before it is committed (2.6 Path C)? | Yes, labelled as held outside the source | Refuse; require the `ink.toml` commit first |
