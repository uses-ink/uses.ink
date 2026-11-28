---
number: "0162"
title: "The forge transport is one https client that calls listed hosts only, and a site claims its repository by `ink repo` or a form"
status: built
date: 2026-11-28
area: edge
supersedes: []
---

## 0162 The forge transport is one https client that calls listed hosts only, and a site claims its repository by `ink repo` or a form

Built (2026-11-28). The real `Transport` and the verbs of 0113.

**The client.** `uses_ink_https::Fetch` is a request-and-bounded-answer client over the one rustls configuration of
`Https` (the bundled roots, `--git-ca`, certificates always verified): https only, HTTP/1.1, no proxy, **no redirect** (a `3xx`
is the answer and nothing is followed), a timeout for the whole call, and a reply read to one byte past its limit so a
caller can tell it was cut. Header values are marked sensitive. A name can be sent to an address the operator gave
(`--forge-route HOST=ADDR`) for a mirror or a test; the certificate is still checked for the name.

**The adapter.** `uses_ink_authority_repo::Wire` implements the forge `Transport` over `Fetch`, and refuses before anything
is sent a URL that is not https or whose host is not on the list: GitHub's API host, each Forgejo instance (the bundled list,
or `--forge-instances FILE`) and each GitLab host (`--forge-gitlab`, `gitlab.com` by default). A repository on any other
host is `repo.host.unlisted`: the server never sends a call to a name a stranger chose. `--forge-timeout` (10 s) and
`--forge-reply` (1 MiB) are the limits the client stamps on every call. Everything is a flag with an `INK_FORGE_` variable.
The crate has a `system` feature, so the API on a Worker builds it without sockets (0130).

**The verbs.** `ink repo claim|prove|rotate|list|release` call `POST /v1/sites/{site}/repos/{claim,prove,rotate,release}` and
`GET .../repos`. The repository is the site's own git source unless a body names one. `claim` and `rotate` print the
challenge and the secret once; `prove` reads the forge; `--json` prints `uses.repo-claim/v1`, `uses.repo/v1`, `uses.repos/v1`
and `uses.repo-released/v1`. A server without `--forge-hooks` answers `repo.forges.off`. The Settings screen has the same four
forms (POST, Origin-checked, like the domain forms); a claim or a rotation answers with the page itself rather than a
redirect, so the secret is on that one response and in no row, cookie or log.

**Tests.** The HTTPS tests run against a fake forge on the loopback whose certificate authority is made in the test, with a
DNS-name certificate; the real-binary tests start `ink serve --forge-hooks` against it, claim and prove a repository, send
signed webhooks and check the builds, a replay, a rotation and a release.

*Left:* the GitHub App registration and the OAuth applications are owner work **(owner)**, and the user OAuth step.
