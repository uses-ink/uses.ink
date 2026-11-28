---
number: "0086"
title: "The client address behind a proxy is read from X-Forwarded-For by trusted hop count, off by default"
status: built
date: 2026-11-28
area: security
supersedes: []
---

## 0086 The client address behind a proxy is read from X-Forwarded-For by trusted hop count, off by default

Built (2026-11-28). It closes the forwarded-header item of 0066. Every limit by source (0066, 0083, 0085)
counts the address of the connection, which behind a reverse proxy is the proxy's.

- **Typed setting, off by default.** `edge_axum::Forwarded` is `Off` or `Hops(NonZeroU8)`. `Bridge::forwarded`
  sets it on the API mount's bridge only; `Bridge::new` keeps its signature. Pages are not affected,
  as they use no source. On a server reached directly the header is whatever the client sends, so
  nothing reads it unless the operator says how many proxies are in front.
- **The client is the entry `N` from the end** of the `X-Forwarded-For` lines (all lines, split on commas),
  where `N` is the trusted hop count. The last entry is what the proxy nearest the server wrote, the
  connection itself being that proxy. Entries left of the `N`th from the end were written by the client or
  by someone else's proxy and are never read, so a client that sends its own `X-Forwarded-For` cannot
  choose its address. IPv4-mapped IPv6 is read as IPv4.
- **Fail loud.** With the setting on, a request with no header, fewer entries than `N`, or an entry that
  is no address gets a 400 `auth.forwarded.refused` (an operator fault, whose fix names the proxy
  setting) and does not reach the API. It is not served as if it came from the proxy, and it does not fall
  back to the connection, because either would let a misplaced proxy quietly turn every limit into one
  shared bucket or into a spoofable one. The reply holds no part of the header.
- **CLI:** `ink serve --data --forwarded-hops <N>` (or `INK_FORWARDED_HOPS`); absent means off, and the
  start-up listing says which. `cli/tests/selfhost.rs` runs it over the real binary: a request with no
  header is a 400, two challenges are allowed per forwarded client, the third is a 429, and another client
  has its own count.
- **Not built:** the `Forwarded` (RFC 7239) header and vendor headers, and a list of proxy addresses to trust
  instead of a count.
