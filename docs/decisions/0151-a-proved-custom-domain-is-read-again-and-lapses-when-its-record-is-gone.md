---
number: "0151"
title: "A proved custom domain is read again and lapses when its record is gone"
status: accepted
date: 2026-11-28
area: control
supersedes: []
---

## 0151 A proved custom domain is read again and lapses when its record is gone

Accepted (2026-11-28). 0093 proves a name once and serves it until the site or the name is removed. A review found
the gap: a domain that changes hands, expires or is dropped by its owner keeps serving the site that proved it, and
no other site can take it.

- **The proof is read again.** `domain_proof` (migration 310) keeps, for each proved name, the token it was proved
  with, when it was last seen and since when it has not been. The `domain` table has no such columns and the database
  has no `ALTER`, so they sit beside it. `prove_domain` writes the row in the batch that makes the name serve.
- **Control lists, the authority reads.** `Control::due_domains(at, limit)` lists the names last seen a
  `Policy::reproof` ago (`--domain-recheck-days`, `INK_DOMAIN_RECHECK_DAYS`, 30 by default), the oldest first.
  `Dns::recheck` reads the TXT record of each and hands what it found to `Control::recheck_domain`, which answers
  `Verdict::Held`, `Restored` or `Lapsed { since }`. `ink serve` runs the pass once an hour as a supervised loop
  (`domains`) and prints a line only when a name lapsed, came back, was freed or could not be read.
- **A lapsed name stops resolving at once.** `Control::domained` answers none for it, so the edge serves nothing at
  the host. It stays the site's: another site's claim or proof of it is `domain.name.taken` during the grace. It is
  listed with the site's pending names, with the token it was proved with, so the owner publishes the same record
  and proves it again, or the next hourly pass restores it by itself.
- **After the grace the name is free.** `Policy::lapse` (`--domain-lapse-days`, `INK_DOMAIN_LAPSE_DAYS`, 14 by
  default) after the first failed read, `Control::free_lapsed` removes the proof and the name. A claim or a proof of
  the name by any site does the same check first, so a name never waits for the loop.
- **A lookup that failed is not a result.** A resolver error is counted and recorded nowhere: the name stays as it was
  and is due again at the next pass. Only a successful read without the token lapses a name. That follows from 0101:
  the lookup says nothing about the name when it got no answer.
- **Events.** `domain.lapsed` and `domain.restored` join `domain.proven` and `domain.removed` in the site's log.
- **Names proved before this migration have no token.** They cannot be checked, so the first pass after the proof
  was last seen a `reproof` ago lapses them, and the owner removes the name and proves it again. That is a break
  for a deployment that already serves domains; none was in service when this was decided. The alternative, to
  serve them for ever unchecked, is the gap this decision closes.

**Left.** A name is read by the authority's resolver alone, over the DNS the operator chose (0101), so the limits of
that lookup apply to the re-check as well. The loop is a single pass per hour over at most 100 names, so an
installation with more than 72,000 proved names needs a larger limit.
