# uses-ink-watch-worker

The watch's `Heads` port (`uses-ink-watch`) on a Worker (decision 0136). `Smart<F: Fetch>` asks a forge for
`<repository>/info/refs?service=git-upload-pack`, reads the reference advertisement (git protocol version 0: packets
of `<4 hex digits of length><payload>`, the capabilities after a NUL on the first line, `symref=HEAD:<branch>` naming
the default branch, `<tag>^{}` lines giving the commit an annotated tag peels to) and picks from it what
`git ls-remote` would. No object is fetched and no git library is linked.

- `Fetch` is the HTTP call: `Fetching` (`wasm32` only) is the platform's `fetch`, `Replay` serves recorded bodies.
- `Advertisement::parse` is the whole parse, pure. `Misread` says what was wrong with a body: a bad length, a cut
  packet, no service packet (not a smart server), a malformed reference line, or protocol version 2.
- A login goes out as `Authorization: Basic` and is redacted in every `Debug`. Only `https` is read; `file` is
  `Unlisted::Scheme`, and the operator's `Network` setting is checked first, before any request.
- The answer is limited (`Smart::MOST`, 8 MiB; `most_of` sets another).
- `Case` is the suite over the three forges' recorded advertisements (`fixtures/`, with how they were recorded and
  trimmed) and a few made bodies. `tests/local.rs` reads one repository both ways, through `watch/git`'s `file`
  transport and through `git upload-pack --advertise-refs`, and the two must agree on heads, branches and the default.
