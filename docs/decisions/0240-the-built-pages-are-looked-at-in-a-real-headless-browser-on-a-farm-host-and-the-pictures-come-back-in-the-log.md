---
number: "0240"
title: "The built pages are looked at in a real headless browser on a farm host, and the pictures come back in the log"
status: built
date: 2026-11-28
area: farm
supersedes: []
---

## 0240 The built pages are looked at in a real headless browser on a farm host, and the pictures come back in the log

Built (2026-11-28). Until now nobody could see the output: the stylesheet of the doodle look (0180 to 0182)
was written against the mock's CSS and the built HTML, and `docs/site.md` said no farm host had a browser.
`cargo xtask shoot` closes that.

- **What it does.** `cargo xtask shoot <site|dashboard|mock> --width 1440,390` builds `ink`, builds the
  landing page and the docs with `ink build`, serves each with `ink serve` on loopback, drives a headless
  Chromium over the pages and writes a full-page PNG per page and width under `target/shots/`. `mock` serves a
  folder of the design mock (`--from`) from the driver, as a reference drawn by the same engine. `probe` says
  whether a browser starts on the host, and `lock` prints the npm lockfile to commit. It is not a stage of the
  gate and not a job of `.config/farm.toml`, since every job there runs in `farm check`; it is run by name,
  `farm cargo xtask shoot site`.
- **The browser.** Playwright's `chromium-headless-shell`, downloaded by `playwright-core` into
  `~/.cache/uses-ink/browsers/` of the host. It starts on the x86_64 hosts with no system package and no sudo
  (verified on `petal`; Playwright says Fedora is not an official platform and uses its Ubuntu build, which
  works). The driver falls back to the static build of `@sparticuz/chromium`, which carries its own
  libraries, when that does not start, and fails with both causes if neither does. The packages are pinned exactly
  in `tools/shoot/package.json` and its lockfile and installed with `npm ci` under `target/shoot/node/`.
- **The way back.** The farm's log is the only channel, so each picture is a palette PNG (the fewest colours,
  from 256 down to 16, that keep it under 500 KB) printed as base64 between `=====SHOT` and `=====END` lines.
  A page taller than 9000 pixels is cut there, which the log says. The driver also prints, per page and width,
  the faces the page declared and whether each loaded, the height, any horizontal overflow with the elements
  past the edge, and every console message, failed request and status of 400 or more.
- **Not in the repository.** The mock is git-ignored and the farm does not sync `.impeccable`, so a run of
  `mock` needs the folder inside the workspace (untracked or ignored there), named by `--from`.
- **Dashboard.** `dashboard` starts `ink serve --data` with an empty data folder and takes the sign-in page.
  The signed-in screens need a seeded session, which the dashboard rebuild owns; the target takes them once it
  has one.
- **What it found** is in the commits that follow it: a wide table or a long line of code made the docs
  scroll sideways on a phone, the landing's pinned note and command card did the same, an inline code span in a
  list item broke out of its line, and the docs header listed nine long titles over two lines.
