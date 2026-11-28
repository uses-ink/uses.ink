---
number: "0230"
title: "A tag pattern GitHub refuses fails the whole workflow file, so the release rule reads them"
status: built
date: 2026-11-28
area: release
supersedes: []
---

## 0230 A tag pattern GitHub refuses fails the whole workflow file, so the release rule reads them

Built (2026-11-28). Every push to `main` showed a failed `release.yml` run with no jobs ("This run likely failed
because of a workflow file issue"). The run page says why: `push event contained invalid tags patterns:
v[0-9]+.[0-9]+.[0-9]+-[0-9A-Za-z.]+`. GitHub allows only letters, digits and ranges inside `[ ]` of a filter
pattern, and one refused pattern fails the file for every event, not only for tags. The pattern was a hand edit that
dropped the `*` of dist's own glob.

- **The pattern.** The prerelease suffix is `-[0-9A-Za-z]+` (`v1.2.3-rc1`) or the same followed by `.[0-9]+`
  (`v1.2.3-rc.1`). Nothing else in a tag name is allowed, so the name is still shell-safe.
- **The check is typed and narrow.** `cargo xtask layout` (release rule) reads the patterns under `tags:` and fails
  with `TagPattern` on a `[ ]` that holds anything but letters, digits and `-`, or is not closed. It also reads the
  list past a comment line, which the `*` rule did not: with a comment between `tags:` and the first item, that rule
  saw no patterns at all.
- **Not actionlint.** The farm runs only cargo on a host, so no verb can fetch and run a release binary, and the
  workspace has no YAML parser. A parser for the whole GitHub schema is a dependency and a maintenance cost for one
  failure; the check names the one rule that failed. A different refusal (a bad expression, a wrong key) shows only
  on GitHub, as the same empty run, and `docs/release.md` says where to read the reason: the run page's annotation.

**Why:** the owner learns of a refused workflow file only from GitHub, once per push, with no reason in the API.
**Reversal:** none; a YAML parser in xtask would replace the pattern check if workflows grow.
