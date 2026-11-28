---
number: "0101"
title: "Custom domains are proved by asking a real nameserver through a small typed DNS client, chosen by a flag"
status: built
date: 2026-11-28
area: edge
supersedes: []
---

## 0101 Custom domains are proved by asking a real nameserver through a small typed DNS client, chosen by a flag

Built (2026-11-28), the reach of decision 0093: `ink domain add|list|prove|remove`, the Settings forms, and a
resolver that asks DNS. Until now the `Resolver` port had a table for tests and `Offline`, so no server could
verify a proof.

- **No DNS dependency.** `hickory` is not in the lock file and the system resolver (`getaddrinfo`) cannot read
  TXT records. A TXT lookup is one question and one answer, so `authority-dns` has its own client: `wire.rs` writes
  the question and reads the answer, `system.rs` sends it over UDP and again over TCP when the answer is cut. It
  adds `tokio` (already in the tree) and `clock`, and no licence to `deny.toml`.
- **Hostile input is a typed error.** Every read is checked against the message length, a compressed name may only
  point backwards and at most 16 times, labels over 63 bytes and the reserved length bits are refused
  (`Malformed`, with the offset). A test cuts a valid answer at every byte.
- **Only the nameserver's answer counts.** The query id comes from `clock::System` (random, 16 bits), the socket is
  `connect`ed to the nameserver so a datagram from anyone else is never read, and an answer with another id, another
  name or another type is waited past, not believed. That is a defence in depth, not a proof of the answer: the
  proof is the token, which the control plane compares (0093), and a forged record needs the token.
- **A name with no records is an answer.** `NXDOMAIN` is an empty list, which the control plane turns into
  `domain.proof.missing`. A server failure, a refusal, a silent server and a bad answer are `domain.lookup.failed`
  (503): the name is not refused, only unchecked, and the cause chain names the nameserver.
- **Flags, all with `INK_` variables.** `--resolver system|offline` (`INK_RESOLVER`, default `system`),
  `--resolver-server ADDR[:PORT]` (repeatable, `INK_RESOLVER_SERVERS`, comma separated), `--resolver-file PATH`
  (`INK_RESOLVER_FILE`, default `/etc/resolv.conf`, read at each lookup so a change shows) and
  `--resolver-timeout SECONDS` (`INK_RESOLVER_TIMEOUT`, 1 to 60, default 5). The limit is the whole lookup: each
  nameserver gets an equal share, so a dead one cannot take the time of the rest. A timeout outside the range stops
  the server at start, naming the flag.
- **One concrete type.** `Chosen` is `Offline` or `System<clock::System>`, so the API and the dashboard hold the
  same resolver without a type parameter each.
- **The dashboard calls the same code.** The Settings screen posts `claim`, `prove` and `unclaim` with one field,
  `name`. They call the control plane and `Dns::prove` exactly as the API's handlers do, so the two cannot differ
  about what proves a name. A lookup that failed is a page with `domain.lookup.failed` and status 503, and the cause.
- **The CLI names the site by its folder's link**, like `ink status`, so `ink domain add www.example.org` in a
  linked folder needs no flags.

**Not done.** The hostname lookup that the hosted edge does through Cloudflare for SaaS, DNSSEC validation (the
nameserver is trusted as the system's resolver is), EDNS0 (an answer over 512 bytes arrives by the TCP retry), and
asking for a CNAME to the site's address. **Reversal:** `--resolver offline` restores the old behaviour with no
code change.
