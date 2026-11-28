# Your notes are already a blog!

Point uses.ink at a repo or a vault and you get a blog at your own address. Your files stay right where they are.

![A person hands a file from a pile of markdown files to a browser window that shows a blog at you.uses.ink.](scene.svg)

uses.ink is pre-release. The hosted service is not open yet. Today you can build a site with `ink`, preview it while you write, and run the whole thing yourself with `ink serve`. The [roadmap](https://github.com/uses-ink/uses.ink/blob/main/docs/roadmap.md) says what is built.

```
$ ink preview
```

No config file, no build step. `ink preview` reads the folder you are in and serves the blog on your own machine.

## Then it tells you what it figured out

uses.ink reads your files the way you left them and writes every guess down next to its reason. Nothing hides behind a setting. `ink explain` prints the list.

| File | What uses.ink decided |
|---|---|
| `README.md` | The home page: the only file at the top. |
| `shelves.md` | A post, dated from git, titled from its first heading. |
| `drafts/` | Drafts are sleeping. Not published. |
| `.obsidian/` | App settings. Skipped. |

Disagree with a guess? Pin it in your repo, in a file called `ink.toml`:

```toml
exclude = ["wip/**"]
```

`ink pin` writes the pin for you, from the decision you want to change.

## It reads what you already use

**A git repo.** Any git remote over https. GitHub, Codeberg and GitLab can also prove the repo is yours. Push like always and the blog follows.

**A folder.** Run `ink push` in any folder of markdown. No git needed.

**An Obsidian vault.** Wikilinks and embeds work. Only the notes you pick get out.

## And your readers get a lovely little page

- It works without JavaScript.
- Every list of posts has a feed.
- Your own domain, if you have one.

> **Notes on building shelves**
> 2 March 2026, 4 minute read
>
> The wood is pine and every cut is a little wrong. Measuring twice did not help; measuring with the same tape twice might have.

That is an example post, as a reader sees it.

## And it stays yours

**Leave whenever.** Your files never moved, so there is nothing to export. Unplug uses.ink and your notes are exactly where you left them.

**Or run it yourself.** One binary, on your own box:

```
$ ink serve --data ./state
```

The [self-host guide](https://github.com/uses-ink/uses.ink/blob/main/docs/selfhost.md) has the install, the first run, backups and upgrades.

---

Open source. Your notes not included. [GitHub](https://github.com/uses-ink), [roadmap](https://github.com/uses-ink/uses.ink/blob/main/docs/roadmap.md), [self-host guide](https://github.com/uses-ink/uses.ink/blob/main/docs/selfhost.md).
