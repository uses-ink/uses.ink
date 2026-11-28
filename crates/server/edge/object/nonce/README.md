# uses-ink-edge-object-nonce

The nonce store's state machine (decision 0135), with no Worker in it. A `Ledger` remembers `(key, nonce)` pairs
until the end of their window, at most `most` per key, and refuses with `Verdict::Quiet` any request created
before `boot + window` of the ledger's own start. It reads no clock: each `Ask` says what time it is. The Durable
Object is a shell around it (`uses-ink-edge-worker`).
