---
number: "0091"
title: "A branch preview is a labelled manifest with an end and no deploy, served at label--handle"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0091 A branch preview is a labelled manifest with an end and no deploy, served at label--handle

Built (2026-11-28), the native part of previews of branches with the owner's defaults of `docs/hosted.md` HQ12:
public by URL for public sources, unlisted, seven days, never for private sources (package C6).

- **A row, not a deploy.** Migration 305 makes `preview` (site, label, the manifest's hash, files, bytes, made,
  ends). A preview has no row in `deploy`, so no deploy list, no live pointer and no rollback can reach it, and
  the retention sweep (0088) does not count it among the newest N. The manifest is stored as a blob in the site's
  tenant like a deploy's, and its files count against the account's storage; a preview that would take the account
  over its limit is refused whole, as a deploy is.
- **The address is `<label>--<handle>.<origin>`.** A handle has no `--`, so the host of a preview is never a
  handle: `Origin::handle` says none for it and `Origin::preview` splits it. The label (`Slug`) is lower-case
  letters, digits and single hyphens, at most 29 characters so label, `--` and the longest handle fit a host label
  of 63. A label that is another site's handle (`www`, `github`) is fine: it only ever names a preview of the
  site whose handle follows `--`, and the edge looks up that handle's own previews. `Slug::branch` makes a label
  from a branch name (`feature/Login_v2` is `feature-login-v2`); two branches that make one label replace each
  other, the later push winning.
- **Public sources only.** A site with a git source and no git token has previews. An upload or folder site has no
  branches (`preview.site.unsupported`, 422), and a site with a git token is private (`preview.site.private`,
  403): its branches are never served at an address anyone can open.
- **Unlisted.** Listing is by the owner only: `GET /v1/sites/{site}/previews` and the Deploys screen's branch table.
  No sitemap, no site list and no deploy list names a preview. The address is the only secret, and it is a guessable
  label, so a preview is public to anyone who learns it; the Deploys screen says so. No `noindex` header is sent: a
  manifest's headers are the build's, and the edge adds none per host.
- **Pushing.** `PUT /v1/sites/{site}/previews/{label}` takes the same tar archive as `PUT .../deploy`, with the same
  checks, and needs the deploy right. A push to a label replaces the preview, starts its seven days again and frees
  the files only the replaced one held.
- **Ending.** `Policy::preview` (`--preview-days`, `INK_PREVIEW_DAYS`, default 7) is the life. The edge asks the
  control plane for a preview at each request and never caches it, so an end applies at once. `Control::expire`
  deletes the blobs only ended previews named, bytes before rows, then their rows; the worker loop runs it
  whenever the queue is empty, at most once a rest. A blob a deploy or a live preview of the site still names stays.
  Retention and `discard` protect every preview's manifest and files.
- **Removing a site** deletes its preview rows with its other rows.

**Not done, and why.** The watch does not discover branches and queue preview builds. A build request carries no
branch or revision (the `job` table has no column for one and the `Build` port builds the site's own source), so a
branch build needs a change to both, which is not native control-plane work. Until then a preview is made by
pushing a built folder to the route above, which `ink` can do from CI for each branch. A cap on previews per site
is not set: their bytes count against the storage limit, which is what bounds them.

**Reversal:** `preview` can be dropped with its routes; nothing else refers to it.
