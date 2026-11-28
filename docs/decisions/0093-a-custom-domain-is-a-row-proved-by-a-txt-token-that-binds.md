---
number: "0093"
title: "A custom domain is a row proved by a TXT token that binds the name to a site; claims serve nothing"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0093 A custom domain is a row proved by a TXT token that binds the name to a site; claims serve nothing

Built (2026-11-28), the native part of custom domains with the owner's defaults of `docs/hosted.md` HQ7 (package
C7, migration 306): our TXT proof first, then, in L2, the Cloudflare for SaaS hostname. Decision 0019 is the
direction; this is what the control plane does before any account exists.

- **Two tables, so the unique key does the work.** `domain` holds proved names: the name is its primary key, so
  of two sites that prove one name at once the database lets one. `domain_claim` holds a site's ask, keyed by
  name and site, with the token its record must carry. Any number of sites may claim a name; none serves it. A
  stranger who claims `victim.example` first therefore blocks nobody: the owner of the name publishes a record
  for their own claim and proves it, and the stranger's claims of that name are dropped in the same batch.
- **The token is the binding.** It is 128 random bits drawn by the caller and stored in the claim row; the TXT
  record at `_uses-ink.<name>` says `uses-ink-domain=<token>`. A record that carries another site's token proves
  nothing for this one (`domain.proof.missing`, with how many records were read). The token is published in DNS,
  so it is not a secret and is compared in constant time only so no code path compares a proof with `==`. Claiming
  again makes a new token; the old record no longer proves it.
- **The proof binds the name to a site, and the site's handle is its name today.** The row holds the site's id,
  not its handle, so a rename keeps the domain (it is the same site) and a held old handle serves nothing. This
  is the choice the plan left open. The other reading, a domain that dies with the handle, would make a rename a
  way to lose a name. The domain is released in the same batch that erases a removed site's rows, so the handle's
  hold (0087) and the domain's release happen together: after a removal the name is free for any site to claim.
- **The control plane decides, the authority reads.** `uses-ink-authority-dns` has the `Resolver` port (TXT
  records at a host, async, not `Send`), a `Fake`, and `Offline`, which says the server has no resolver. `Dns`
  reads the records and hands `Found { records, tokens }` to `Control::prove_domain`, which compares them with the
  claim. A lookup that got no answer is `domain.lookup.failed` and proves nothing: it is not a refusal of the name.
  Self-host has `Offline` until an adapter for a real resolver exists; the claim and list calls work without it
  and the proof call says why it cannot run.
- **Names are checked where they are read.** `DomainName` is lower-case ASCII labels (a name with other
  characters is written in its `xn--` form), two labels or more, at most 253 characters, the top label not all
  digits. A name equal to or under the operator's origin (`uses.ink`, `domains.uses.ink`, `main--tea.uses.ink`) is
  refused (`domain.name.ours`), so no site can claim the operator's own space.
- **A site has at most 10 domains, proved or not** (`Policy::domains`, `--limit-domains`, `INK_LIMIT_DOMAINS`),
  so a claim, which costs a row, is bounded. It is not on the public limits page: that page is the budgets of
  0088, and adding one there changes every golden of the page. It moves there with the Cloudflare per-hostname
  allowance in L2, where the number matters to the owner's bill.
- **The edge asks every request and caches nothing of it.** A host that is no handle's and no preview's is parsed
  as a `DomainName` and looked up in `domain`; the site it names is served by its handle's live pointer (cached
  as before), so removing a domain, removing the site or suspending it applies at once. A suspended site answers
  its notice at its custom domain too.
- **Apex and `www`.** The rows do not know the difference: a name is a name. Whether a customer's DNS can point an
  apex at `domains.uses.ink` is a fact about their DNS provider (CNAME flattening) and is checked when the
  Cloudflare hostname is made (L2). The words the dashboard says (apex only where flattening works, else `www` and
  a redirect, said plainly) belong there too.
- **API.** `GET /v1/sites/{site}/domains`, `PUT` and `DELETE /v1/sites/{site}/domains/{name}`, and `POST
  .../domains/{name}/proof`, with the edit right. `Api` has a defaulted fifth parameter, the resolver, so the
  places that build one are unchanged; `Api::with_resolver` gives it one. The Settings screen lists the names and,
  for a pending one, the record to publish.

**Not done.** The adapter that asks a real resolver, the Cloudflare hostname and its certificate (L2), the CLI verb
`ink domain`, and forms on the Settings screen to claim and prove. **Reversal:** `domain` and `domain_claim`
can be dropped with their routes; the host lookup falls back to handles and previews alone.
