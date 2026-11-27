## 0046 A source plugin sees no bytes; the host fetches, stores and measures

Accepted, as built (2026-11-27). The `ink:source` world imports `ink:host/git` and exports one
call, `read(want) -> result<snapshot, refused>`. A plugin holds the rule (how much history to
fetch for what the site needs, and a check that the snapshot is of the commit fetched); the host
holds the network, the credentials, the clone cache and every file's bytes.

- **Two calls.** `git.fetch(request)` clones into the host's cache under the builder's network
  setting, the grants it holds and the clone limits of 0044, and answers with the commit, whether
  the clone is shallow and what it cost. `git.read(cloned)` reads that commit's tree into a
  `Snapshot` with `Known` dates and the abilities observed. A refusal is a typed value (network,
  auth, budget, revision, transport) and the rest is `failed`, whose causes the host says once as
  a diagnostic.
- **Bytes stay on the host.** The files are put in a store the host made for the call and moved
  into the caller's store only when the plugin has returned a snapshot, so a plugin that fails
  or refuses leaves the caller's store as it was.
- **One budget.** The time the host spends in `git.fetch` counts against the plugin's call, and a
  source has a budget of its own, since a fetch takes longer than a page's render.
- **One rule, two bodies.** `uses-ink-source-git` is the rule over a `Git` trait of `model`, run
  natively over `host_native`'s own fetching and as a component over the imported one; a test
  holds the two to the same snapshot.

**Why:** a plugin that never holds bytes or a credential cannot leak or corrupt them, and the
budgets stay the host's. **Reversal:** a source that must transform bytes (an archive unpacker)
would need a capability that hands them over under a limit.
