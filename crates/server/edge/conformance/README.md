# uses-ink-edge-conformance

The cases every adapter of the edge's two ports passes: `Sites` (a host to its deploy) and `Blobs`
(the bytes behind a hash, in the scope the deploy was found in). An adapter implements `Harness`, which
says how to seed a site, and runs `suite!` natively or `Case::run` in a Worker. Decision 0200.

- A case an adapter cannot offer (a folder has no suspensions and no custom domains) is `Outcome::NotOffered`
  with the reason, and a runner prints it as such. It is never counted as a pass.
- The cases that go through `Edge` check the serve path as a client sees it: status, length, a range, a
  `404`, a `403` for a suspended site.
