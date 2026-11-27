# Authentication

Status: **Accepted** (decision 0068): the owner took the recommendation on every open choice in
section 23. This is the design the code follows; what is built is listed in the roadmap. `auth-preview.html` shows the flows as sequence diagrams and the
screens (CLI and dashboard) in light and dark; open it in a browser. Mock-ups are illustrative:
ids, codes and names are made up. A code marked (new) is not in `diag/catalog.toml` yet; every
other name is taken from the code on `main`.

The owner's brief: holding passwords is scary, so design a cryptographic scheme that is practical,
secure and modern. The owner approved the direction "no passwords anywhere". Section 1 says what
that means. Where this design changes the first proposal, the section says so and section 21
collects the changes.

Contents: 1 Scope, 2 Credentials at a glance, 3 Research that shaped it, 4 Passkeys, 5 Device
keys and the signed-request profile, 6 Enrolling a device, 7 Deploy keys and CI, 8 Browser
sessions, 9 Bootstrap and recovery, 10 The operator and the existing token, 11 Relying party,
origins and TLS, 12 Rights, scopes and orgs, 13 Audit and "last used", 14 Threat model, 15 Crypto
and crates, 16 Alternatives, 17 Post-quantum, 18 Data model, 19 API, the port and the CLI,
20 Errors, 21 What changed from the first proposal, 22 Testing and review, 23 Open choices.

## 1. Scope

**"No passwords" means no secret that a person chooses and types.** Every secret in the scheme is
either a private key that a machine made (passkey, device key, deploy key) or a random string a
machine made (session id, recovery code, link token). A person never invents one, so nobody
reuses, guesses or loses one to a phishing form.

Out of that rule, and not a person's credential: the **operator's SMTP password** that
`mail/smtp` holds (a service credential in the operator's config, sealed in memory), and the git
tokens an author hands the `secret` port for a private repository (0031). Neither signs anyone in.

What the design covers: people in a browser, the `ink` CLI on a laptop, CI and deploy jobs, the
self-host operator, and hosted accounts with orgs. What it does not cover: how an author proves a
repository or a domain is theirs (`ink:authority-*`, architecture section 12); that proves a claim
over a name, it does not sign anyone in (open choice 9 asks whether it ever should).

## 2. Credentials at a glance

| Who | Credential | Private half lives | Server stores | Sent how | Lasts |
|---|---|---|---|---|---|
| A person, in a browser | **Passkey** (WebAuthn, ES256) | the authenticator (phone, laptop, security key, password manager) | credential id, COSE public key, flags, label | a WebAuthn assertion, once per sign-in | until removed |
| That person, after sign-in | **Session** | the browser's cookie jar | BLAKE3 digest of a 256-bit random id | `__Host-` cookie, dashboard host only | 30 days absolute, 7 idle |
| The `ink` CLI on a laptop | **Device key** (Ed25519) | OS keystore, else a 0600 file | public key, label, rights | every API request is signed | 180 days, renewable |
| A CI or deploy job | **Deploy key** (Ed25519) | the CI secret store | public key, one site, rights, expiry | every API request is signed | 90 days by default, 365 at most |
| The self-host operator | **Operator key** (Ed25519) | the operator's keystore | a line in `<data>/operator-keys` | every API request is signed | until the line is removed |
| Recovery | **Recovery code**, **email link** | printed or in a password manager; the mailbox | digests only | typed once, opened once | one use |

Two rules hold for the whole table.

1. **The server holds no secret that signs in.** Public keys and digests of random 256-bit values
   are all it has. A database leak lets nobody sign in (section 14).
2. **Cookies and signatures never meet.** The dashboard host reads cookies and accepts no
   signature. The API host reads signatures and ignores cookies. The API therefore has no ambient
   credential, so it has no CSRF and needs no CORS (section 8).

## 3. Research that shaped it

Read for this design, with what each changed.

| Source | What it settled |
|---|---|
| **WebAuthn Level 3** (W3C, Candidate Recommendation), registration and assertion verification (7.1, 7.2), user verification, backup eligibility flags (6.1.3), signature counter (6.1.1), Related Origin Requests | UV required (4), flags stored and checked (4), counter used only when nonzero (4), rp id rules (11), ROR as the domain-move escape hatch (11) |
| **RFC 8628** OAuth device grant, 3.1 (user code), 5.1 (brute force), 5.2 (code length), 5.4 (remote phishing) | Short code, rate limit and expiry (6); the phishing section is why the CLI shows a key fingerprint and the page does not prefill the code (6) |
| **RFC 9421** HTTP Message Signatures | The shape (covered components, created, nonce, key id) and the footguns that make a fixed profile better than the general format (5) |
| **RFC 9530** Digest Fields | The idea of a body digest in a header. We depart from its algorithm registry on purpose: BLAKE3 (5) |
| **RFC 9449** DPoP | Why a proof-of-possession JWT per request is the same idea with more parts (5, 16) |
| **RFC 8032** Ed25519, with Chalkias et al., "Taming the Many EdDSAs" (2020) | Strict verification: canonical S, small-order keys rejected (5, 15) |
| **RFC 9807** OPAQUE | Out: it is still a password (16) |
| **CTAP 2.x** notes on discoverable credentials and storage | Why `residentKey: required` is the honest setting for a login with no username (4) |
| **OWASP ASVS** (V2 authentication, V3 session, V6 cryptography) and the OWASP **Session Management**, **CSRF Prevention**, **Forgot Password** and **Authentication** cheat sheets | Cookie attributes, absolute and idle timeouts, step-up, uniform replies, single-use links (8, 9) |
| `webauthn-rs` 0.6.2 source, `ed25519-dalek` 3.0.0 and `ed25519-compact` 2.6.1, `lettre` 0.11.24, `keyring` 4.2.0, the RustSec database, `deny.toml` | Crate choices, licences, advisories (15) |
| Cloudflare docs (`request.cf.tlsClientAuth`), PyPI trusted publishers, the web.dev Related Origin Requests page | mTLS (16), CI without secrets (7), ROR support (11) |

Facts found while reading that changed the first proposal:

- **`webauthn-rs`'s high-level `Passkey` API does not do what the brief asks.** It registers with
  `residentKey: discouraged`, forces user verification to required, accepts ES256 only, and its
  discoverable (usernameless) login exists only behind the `conditional-ui` feature and forces
  conditional mediation. Its own docs recommend against that flow. A login with no username needs
  `residentKey: required`, so section 4 builds on `webauthn-rs-core`, behind our own port.
- **`uses.ink` is going on the Public Suffix List** (`docs/psl.md`). `app.uses.ink` therefore
  cannot use `uses.ink` as its relying party id; the id is the exact host (section 11).
- **The licences are not in `deny.toml`'s allow list.** `webauthn-rs` and its tree are MPL-2.0,
  `ed25519-dalek` and `curve25519-dalek` are BSD-3-Clause, `ring` and `aws-lc-rs` carry ISC.
  Each is a crate-scoped exception, which 0044's comment says is a decision (section 15, open
  choice 2).
- **`crypto-glue` 0.2.3 (webauthn-rs's crypto layer) was published 2026-11-26** and depends on
  `rsa`. The RustSec entry RUSTSEC-2023-0071 (Marvin timing attack) is patched in `rsa` 0.10.0
  (published 2026-11-08), so a lock file must resolve `rsa >= 0.10`; `cargo deny` checks it.
  `webauthn-rs` also moved off OpenSSL to this layer in the 0.6 series, so whatever review the
  OpenSSL-based versions had does not cover what we would ship.
- **`lettre` had six advisories on 2026-11-17** (SMTP command injection, forged STARTTLS replies,
  mail sent to the wrong address for a quoted local part, two panics, unbounded buffering), all
  patched in 0.11.24. Pin `>= 0.11.24`. The `signin` agent's `mail/smtp` already uses `lettre`.

## 4. Passkeys

People in a browser sign in with a passkey: a WebAuthn credential (W3C WebAuthn Level 3).

### Settings, and where they differ from the brief

| Setting | Value | Why |
|---|---|---|
| `residentKey` | **required** (discoverable) | The login has no username field. A non-discoverable credential cannot be found without one, and enumeration is avoided because nothing is asked. A security key that cannot store one fails at registration, with a message, not months later at login. CTAP2 keys hold at least 25 such credentials; we use one per account. |
| `userVerification` | **required** at registration and at every login (the brief said preferred) | There is no second factor. Presence alone would make a stolen unlocked key, or a phone in a pocket, a full account. The server also checks the UV flag in the authenticator data; it does not trust the request option. A U2F-only key without a PIN cannot register. |
| `attestation` | `none` | We do not filter authenticators by make. Attestation adds privacy cost and a trust list to maintain, for no property we need. |
| `pubKeyCredParams` | ES256 (-7) only | What the library verifies, and what every platform authenticator offers. Ed25519 (-8) is left out until the library supports it and the passkey providers do. RS256 is left out: it is TPM-era legacy. |
| `authenticatorAttachment` | unset | Platform and roaming keys are both allowed. |
| `excludeCredentials` | the account's existing credential ids | One authenticator, one passkey per account. |
| Ceremony state | server side only, single use, 5 minutes | Section 8. A challenge kept in a cookie can be replayed. |

### What the server stores per passkey

`credential id`, `COSE public key`, `sign count` (nullable), `backup eligible` (BE) and `backup
state` (BS) flags, `transports`, a `label` the person gives, `created`, `last used` (a date, not a
time: section 13). User handle is the account id, which is 16 random bytes and carries no personal
data.

### Checks at login (WebAuthn 7.2)

All of 7.2 through the library, plus these, each a typed refusal:

- The UV flag is set. The origin is one of the configured origins and the rp id hash matches.
- **BE never changes** after registration. A credential that was not backup eligible and now is,
  or the reverse, is refused: it is a different thing than the one enrolled.
- **The counter is a clone signal only when it is used.** Synced passkeys (BE set) report 0
  forever, so a zero counter is normal and is never an error. When the stored and the presented
  counters are both nonzero and the new one is not greater, the credential is **flagged** (an
  audit event, a banner on the account page) and, for a credential that is not backup eligible,
  **refused**. Hardware keys do not run backwards unless cloned. For synced passkeys the policy
  does nothing, because the platform may share a counter value across devices.
- BS may change (a passkey became synced); it is stored and shown.

### The crate, as an adapter behind a port

The core of the design never names `webauthn-rs`. It names a port:

```rust
pub trait Passkeys {
    type Error: Error + 'static;
    fn start_registration(&self, who: &Account, existing: &[CredentialId]) -> Result<(Creation, Registering), Self::Error>;
    fn finish_registration(&self, state: Registering, reply: &Created) -> Result<Enrolled, Refused<Self::Error>>;
    fn start_login(&self) -> Result<(Request, Authenticating), Self::Error>;
    fn who(&self, reply: &Asserted) -> Result<(AccountId, CredentialId), Refused<Self::Error>>;
    fn finish_login(&self, state: Authenticating, reply: &Asserted, stored: &Passkey) -> Result<Verified, Refused<Self::Error>>;
}
```

The adapter is `webauthn-rs-core` (its `RegistrationState` and `AuthenticationState` derive serde,
so the state goes into the `ceremony` table without the `danger-allow-state-serialisation`
feature the wrapper crate needs). **Unverified: that `webauthn-rs-core` builds for
`wasm32-unknown-unknown`**, which M4's Worker needs. The tree is pure Rust (RustCrypto,
`x509-parser`, `der-parser`, `nom`, `serde_cbor_2`) and `crypto-glue` lists `wasm-bindgen`, which
suggests it was meant to; a spike proves it before M3 depends on it (section 22). If it fails, the
fallback is our own verifier for the one ceremony shape we use (attestation `none`, ES256): about
a thousand lines, which is open choice 3.

**Randomness exception.** The library draws the challenge from its own `rand` (ChaCha seeded by
the operating system), not from `clock::Draw`. That is an OS CSPRNG either way; the exception is
documented, and a test checks the challenge is 32 bytes and never repeats. Everything else of ours
draws through `Draw`.

### The JavaScript fact

WebAuthn is a browser API, so the sign-in, registration and step-up pages need JavaScript: about
120 lines (base64url, `navigator.credentials.create` and `.get`, `fetch`), in one file served
from the dashboard host and covered by its CSP (`script-src 'self'`, no inline). Reader pages stay
free of JavaScript (architecture section 13); this is the dashboard, which already loads the pen
as wasm. A browser without JavaScript gets a page that says so and names `ink login` and the
recovery codes as the ways in (a recovery code still needs a passkey registration at its end, so
no-JavaScript access is not a goal).

## 5. Device keys and the signed-request profile

The CLI holds an **Ed25519 device key**. Every API request carries a signature made with it. No
token crosses the network, so nothing a proxy, a log or a CI trace sees can be replayed past its
window (section 14).

### Where the private key lives

1. **The OS keystore** through `keyring` 4.2.0 (MIT OR Apache-2.0): Secret Service on Linux,
   Keychain on macOS, Credential Manager on Windows. The stored item is the 32-byte seed.
2. **A file** when no keystore answers (a headless server, a container): `$XDG_CONFIG_HOME/ink/keys/<server>.key`,
   mode 0600, in a directory of mode 0700. `ink` refuses a key file readable by others, as `ssh`
   does, and says the `chmod` to type. The fallback is a notice every run (`-v`), never silent.

Honest limit: Ed25519 has no hardware-backed, non-exportable form on common platforms (the Secure
Enclave and TPMs do P-256). Any process of the same user can ask the keystore or read the file. A
stolen laptop with an unlocked session loses the key. Mitigations are expiry, revocation, and the
rule that a device key cannot mint credentials (section 12). A later key type `p256-v1` can be
bound to a TPM or Secure Enclave without changing the profile (section 5, agility).

Key text format, for files and CI secrets: `ink-ed25519-1:<43 base64url characters>` (the seed).
The prefix makes it greppable by secret scanners.

### The decision: a fixed profile, modelled on 9421, not 9421

RFC 9421 is built for generality: any header can be covered, the signer chooses which, several
signatures can ride one message, and the algorithm is a parameter. Each of those is a place for a
verifier to be tricked. We need none of them: we write both ends and the covered set never varies.

Known footguns of 9421 that the fixed profile removes:

1. **The signer picks the covered components in `Signature-Input`.** A verifier that forgets to
   require `@method` or the body digest accepts a signature that covers neither. In the profile the
   covered set is not on the wire at all.
2. **Structured Fields parsing** (RFC 9651) is two header grammars and a dictionary to get right.
   The profile is one header with five fixed-length fields.
3. **Header canonicalisation** (folding, multiple instances, case, whitespace) is where proxies and
   verifiers disagree. The profile covers no header except the one it lives in.
4. **`@target-uri` and `@authority` depend on what the proxy rewrote.** The profile signs the
   request target as the verifier received it, and refuses any target that is not already in
   canonical form (below).
5. **`alg` is a request parameter**, an invitation to downgrade. The profile has none.
6. **Several signatures and labels.** One signature, always.

We keep from 9421: signing a defined base string, `created`, `nonce`, `keyid`, a body digest.

### The wire form

One header, no other header is read:

```
Authorization: Ink-Sig-1 k=<kid>, c=<created>, n=<nonce>, d=<digest>, s=<signature>
```

| Field | Form | Meaning |
|---|---|---|
| `k` | 22 base64url characters (16 bytes) | the key id |
| `c` | decimal, no sign, no leading zero, at most 10 digits | `created`, unix seconds |
| `n` | 22 base64url characters (16 bytes, one `Draw`) | a fresh nonce |
| `d` | 43 base64url characters (32 bytes) | BLAKE3 of the request body (of the empty string when there is none) |
| `s` | 86 base64url characters (64 bytes) | the Ed25519 signature |

The parser accepts exactly this: the scheme `Ink-Sig-1`, the five names in this order, one space
after each comma, unpadded base64url that decodes and re-encodes to the same text, nothing before
or after. Anything else is `auth.signature.malformed` (new). Strict means there is one wire form
per message, which makes the parser small enough to fuzz to exhaustion (section 22).

`Ink-Sig-1` is the version of the whole profile (signing input, field set, parser). A change is
`Ink-Sig-2`, not a flag.

### What is signed

The signing input is these lines, each ending in `\n`, as bytes:

```
uses.ink ink-sig 1
aud <authority>
method <METHOD>
target <request-target>
digest <d, base64url>
created <c, decimal>
nonce <n, base64url>
key <k, base64url>
```

Ed25519 (the pure scheme of RFC 8032, not Ed25519ph) signs those bytes. Why this is unambiguous:

- Every value's alphabet excludes `\n`: `authority` is the configured API host and port (letters,
  digits, `.`, `-`, `:`), `METHOD` is upper-case ASCII from the router's closed set, `target` is
  from the canonical alphabet below, the rest are base64url or decimal. A test asserts that for
  every field type, so no field can contain a line break and shift the next line.
- Every line has a fixed label and the order is fixed, so there is one parse of the string.
- The first line is the **context**: it names the product, the protocol and its version, so a
  signature made here verifies nowhere else and nothing made elsewhere verifies here.
- `aud` is the API authority **as the server is configured**, not as the `Host` header said, so a
  signed request to one instance fails on another (a CI key used against a staging and a
  production server).
- `key` is signed, so a signature cannot be moved to another key id.

### The request target

The signed `target` is the raw request target: the path and, if present, `?` and the query, as sent.
It must already be in **canonical form**, and the verifier does not canonicalise: it **checks**,
and refuses a target that is not. Canonical form:

- The path is what `Target::parse` already demands (0051, 0052): `/` separated segments, no `.`,
  `..`, empty segment or `%2f`.
- Bytes outside the unreserved set (`A-Z a-z 0-9 - . _ ~`) and the segment delimiter `/` are written
  as `%XX` with **upper-case** hex. Unreserved bytes are never escaped. So `%7e` is refused, `~` is
  right.
- The query is `name=value` pairs joined by `&`, sorted by name (then value) as bytes, same
  escaping, no empty pair, no repeated `?`, no fragment. A name without `=` is refused.

The reason for refusing rather than repairing: a repair function has to be identical on the client
and on every box in between. A request that is already in the canonical alphabet passes through
normalising proxies and URL parsers byte for byte (Cloudflare normalises percent-encoding and
dot segments on the way to a Worker; a canonical path has none of either), and anything else fails
closed with `auth.target.not-canonical` (new) before a signature is looked at. Today's `Request`
carries a `path` and no query; it gains a `query` field with the received bytes.

### The body

The signature covers the body through its BLAKE3 digest `d`. The order of checks resolves the
tension with 0052's "authentication runs before routing":

1. **At the head**: parse the header, check the skew, look up the key, verify the signature (which
   covers `d`), check the key is live, claim the nonce, check the rights. This is when a caller
   with no valid credential is refused and learns no route.
2. **After the body is read**: compute the BLAKE3 of the bytes and compare with `d` in constant
   time. A mismatch is `auth.digest.mismatch` (new), and no effect has happened, because the API
   already validates a whole intake before it stores anything (0052).

BLAKE3 is not in RFC 9530's registry of digest algorithms (`sha-256`, `sha-512`). We depart on
purpose: BLAKE3 is the product's one content hash (blobs, deploy ids), so a client that streams an
upload for its manifest already computes it, and `d` is a field of our profile, not a `Content-Digest`
header a third party has to understand. A client that must sign a 256 MiB archive reads it twice, or
spools it, the same as any digest-in-header scheme.

### Skew, nonces and restarts

- **Skew window** `skew` (operator setting, default 60 s, at most 300 s): `c` must be within
  `[now - skew, now + skew]` by the `clock::Clock` port. A request outside it is
  `auth.signature.skewed` (new), with the server's time in the reply and in `Date`. The server's
  time is not secret; the CLI needs it to tell the person their clock is wrong, in numbers, and does
  not adjust its own (no magic).
- **Nonce store**: the pair `(k, n)` is kept until `c + skew`, and a second use is
  `auth.signature.replayed` (new). A nonce is claimed **only after the signature verifies and the
  key is live**, so an attacker who cannot sign cannot fill the store. The store is bounded per key.
- **Self-host**: the store is memory. On start the server records `boot = now` and answers a signed
  request whose `c` is earlier than `boot + skew` with `auth.server.restarting` (new, retry after
  the remainder). A request captured before the restart has `c` at most `skew` ahead of the
  capture time, so it is refused for the whole window after restart without persisting a nonce. The
  cost is up to `skew` seconds of "try again" after a restart; the CLI waits and retries once.
- **Hosted**: isolates do not share memory, so the store is the per-account Durable Object's
  storage (or a D1 unique key on `(k, n)` with a delete after `c + skew`). A Durable Object that is
  evicted applies the same boot rule.

Residual risk, stated plainly: a replay inside the skew window, **of the same request, to the same
server, with the same body** can only happen if the nonce store lost it. The two designs above close
that for restarts. They do not close it against a nonce store an attacker can write to.

### Algorithm agility policy

**None.** A key's type is fixed when it is enrolled and stored beside it. The only type today is
`ed25519-v1`. The verifier looks up the key by `k`, reads its stored type, and runs that type's
verifier with that type's signing input; the request cannot name or change either. A future type
(section 17) is a new stored type with a new profile version, enrolled the way any key is, with a
passkey approval. The operator can list the types a server accepts (`key-types`), so retiring one
is a config line. There is no negotiation, so there is no downgrade.

The key id is derived, not chosen: `k = first 16 bytes of blake3::derive_key("uses.ink 2026-11-27 key id v1", type byte || public key)`.
A key id is public and not secret.

### Ed25519 strictness

Verification uses the strict form: it rejects a non-canonical `S` (malleability), a small-order or
non-canonical public key, and it is cofactorless. `ed25519-dalek` has `verify_strict`; section 15
covers the alternative. The CLI builds its signing key from the 32-byte seed only
(`SigningKey::from_bytes`), never from a separately supplied public key: RUSTSEC-2022-0093 is the
double public key oracle, where `ed25519-dalek` 1.x signed with a public key that did not belong to
the private one and leaked the private key. 2.x and 3.x close it by deriving the public key; we
also never take one separately.

### Replies that do not leak

The order is chosen so a caller learns only what its proof entitles it to:

| Stage | Reply | Why it leaks nothing |
|---|---|---|
| Header not in the profile | `auth.signature.malformed` | Says only that the caller's bytes are wrong |
| Target not canonical | `auth.target.not-canonical` | Same |
| `c` out of window | `auth.signature.skewed` with the server time | True for any request, no key was looked at |
| Unknown key id, **wrong signature**, wrong audience | the one `auth.signature.refused` | A caller cannot tell "no such key" from "wrong signature" |
| Signature valid, key revoked or expired | `auth.key.revoked`, `auth.key.expired` | Only the holder of the private key reaches this branch, and the holder should know |
| Signature valid, nonce seen | `auth.signature.replayed` | Same |
| Signature valid, right missing | `auth.right.missing` | The caller is authenticated; a site of another account is still a 404 (0052) |

A uniform timing is not attempted for unknown key versus bad signature: the key id is public
(derived from a public key), so there is nothing secret to probe.

## 6. Enrolling a device

`ink login` enrols the CLI's key with the **device flow of RFC 8628**, adapted: the secret is the
key pair the CLI makes, so no `device_code` or access token exists.

1. The CLI generates a key pair (or reuses the one it holds for this server), then calls
   `POST /v1/enrol` with the public key, the key type, a label (hostname, editable) and the purpose
   (`device`). Unsigned: the caller has no credential yet.
2. The server stores a pending **enrolment** (10 minutes) and answers with an id, a **user code**
   (8 characters from a 20-letter alphabet without vowels or lookalikes, about 34 bits, shown as
   `HRKM-2WPD`), the verification URL (`https://app.uses.ink/device`, no code in it) and a polling
   interval.
3. The CLI prints the URL, the code and the **key fingerprint** (the first 10 characters of `k` in
   Crockford base32, `7QKM-2H4P-X9`). It then polls `GET /v1/enrol/{id}`, **signed with the pending
   key**: only the holder of the private key learns the outcome.
4. The person opens the URL on any device where they are signed in, types the code, and sees:
   the label (shown as text from the requester, not as fact), the fingerprint, the key type, the
   rights it would have, the time it asked and the first request's coarse origin (country and
   network name, if the edge supplies it; never stored beyond the enrolment). They compare the
   fingerprint with their terminal, choose **Approve** or **Deny**, and approve with a **passkey
   assertion** (UV), which is a step-up (section 8).
5. The poll answers `approved`. The key is live. The CLI prints the result.

Why this differs from stock RFC 8628, and the remote phishing attack it answers (RFC 8628 5.4: an
attacker starts a flow and talks the victim into approving the code):

- The attacker's key is the one that would be enrolled. The victim has no terminal showing the same
  fingerprint, and the page says so in words: "Approve only if you ran `ink login` just now and this
  code and key match your terminal."
- **No `verification_uri_complete`.** The code is typed, not carried in a link, so a link in a
  message cannot pre-fill it.
- The approval needs a passkey. A stolen session cannot approve a key.
- The code is short-lived (10 minutes) and **five wrong guesses at a user code, or twenty pending
  enrolments, close the enrolment endpoint** for the minute (RFC 8628 5.1 and 5.2). A 34-bit code
  under those limits is not guessable in ten minutes. The code is not secret-grade and is stored as
  text; a leaked pending code alone enrols nothing, because approval needs a passkey and the key is
  not the guesser's.
- A denied or expired enrolment gives the same `auth.enrolment.expired` (new) for an unknown code,
  so codes are not probed.

Self-host adds a second route in, with no browser: `ink login --data <folder>` on the server's host
(section 10).

## 7. Deploy keys and CI

A **deploy key** is an Ed25519 key with a scope: **one site**, and the rights `deploy` (and
optionally `read`). CI signs requests with the private half. The server stores the public half,
the site, the rights, an expiry, the creator and a label. No bearer secret exists on the server.

Make one with `ink key add --site garden --right deploy`. The CLI generates the pair locally, starts
an enrolment exactly as in section 6 (the page shows the **site, the rights and the expiry**, which
is what the person is approving), and on approval prints the private key **once**, on stdout, only
when stdout is not a terminal or `--out <file>` is given (it refuses to print a secret into a
scrollback: `auth.key.print-refused`, new). The private key never reaches the server.

- **How CI reads it**: `INK_KEY` (the `ink-ed25519-1:` text) or `INK_KEY_FILE` (a path), plus
  `INK_SERVER`. Both follow the `INK_*` rule of `cli.md` section 13. A file is better than an
  environment variable, which any child process inherits; the docs say so and show the CI step
  that writes the secret to a file.
- **Lifetime**: required. Default 90 days, maximum 365 (operator setting, 0044: the operator's value
  is the ceiling and a request above it is a diagnostic naming both numbers). A key with no expiry
  does not exist.
- **Rotation**: `ink key add --site garden --right deploy --replaces 3DSF-7K2M`. The new key is
  approved with a passkey; the old one stays valid until the new one is first used, or seven days,
  whichever is sooner, then retires. Nothing is renewed silently.
- **Warning before it breaks**: a signed reply carries a typed notice when the key has 14 days left
  or fewer (`auth.key.expiring`, new), so the CI log says so; `ink key list` and the dashboard show
  the date.
- **What a stolen deploy key can do**: push to one site. It cannot read other sites, manage
  credentials, or mint keys. That bounds the damage of a leaked CI secret, which is the realistic
  failure, so the default scope is small.
- **Keep the key out of the build.** The key belongs in the push step only, not in the environment of
  `npm run build` or any step that runs repository code. `ink` builds in the CI job from the author's
  own repository, which is the author's trust, but a build step that does not need the key should
  not hold it. The docs show a two-step example.

**Builds and plugins never see credentials** (0046, 0055). The hosted builder runs `ink build` in a
container that has no binding to the control database and holds no deploy key; the source plugin
is handed a grant by name and the host holds the bytes. A deploy key lives in the author's CI, and
the hosted builder fetches by the sealed git token of the `secret` port, not by anything in this
document. The self-host `build/local` runs plugins as wasm components with no file or environment
capability, so a plugin cannot read `<data>`.

**Better than a long-lived secret: trusted publishing.** A CI job that proves its own identity needs
no secret at all. GitHub Actions can mint a short-lived OIDC identity token for a workflow (the
`id-token` permission); PyPI accepts it as a "trusted publisher" bound to a repository, a workflow
and optionally an environment (PyPI's documentation confirms this; I did not confirm crates.io's
equivalent from its documentation). The server would trust a claim set (`repository`, `ref`,
`environment`) for a site and verify the token against the issuer's keys. It costs a JWT verifier
(the footguns of DPoP and JWT, section 16), an outbound fetch of the issuer's keys (egress on the
edge), and one integration per forge. It is not v1. The deploy key stays the generic path that
works on any CI and any forge, and `Credential` gets a `Workload` variant later (section 19).
Open choice 6.

## 8. Browser sessions

After a passkey login the server makes a session.

- **The id**: 32 bytes from two `Draw` calls, shown as 43 base64url characters. Only
  `blake3::derive_key("uses.ink 2026-11-27 session id v1", id)` is stored. The cookie is the only
  place the id exists.
- **The cookie**: `__Host-ink_session=<id>; Path=/; Secure; HttpOnly; SameSite=Lax`, no `Domain`,
  with `Max-Age` equal to the absolute expiry. The `__Host-` prefix makes browsers refuse it unless
  it is `Secure`, has `Path=/` and no `Domain`, so a sibling subdomain cannot set or shadow it
  (with `uses.ink` on the Public Suffix List, author sites at `<handle>.uses.ink` are separate sites
  anyway). It is set by the dashboard host and read only there.
- **Expiry**: an **absolute** limit (default 30 days) and an **idle** limit (default 7 days), both
  operator settings with ceilings (90 days and 30 days). Either ends the session. Per OWASP ASVS V3
  the absolute limit is not extended by use.
- **Idle accounting**: `last_seen` is updated at most once per 10 minutes, so a session list shows
  "last used" without a write on every page view.
- **Step-up**: a session holds `stepped_up_at`. Anything that **adds or widens** a credential
  (register a passkey, approve a device or deploy key, change recovery, start deletion of an
  account) needs a passkey assertion within the last 5 minutes. A step-up **replaces the session id**
  (new cookie), as does sign-in (no fixation).
- **Revocation and the list**: the account page lists sessions (label such as "Firefox on Linux",
  created, last used as a date, "this one") and ends any of them. Ending a session deletes its row.
  Removing a passkey ends the sessions that were made with it. Section 9 lists the other events that
  end every session.
- **What is not stored**: IP address and the full user agent. The label is a short class parsed at
  creation.

### CSRF

1. `SameSite=Lax` blocks the cookie on cross-site POSTs. `Lax` (not `Strict`) so that opening the
   dashboard from an email link is still signed in; the next point is what makes that safe.
2. **No state change on GET, ever.** The router refuses a mutating call on a read method, and a
   link opened from email only shows a page (section 9).
3. **Origin check** on every request whose method is not `GET`, `HEAD` or `OPTIONS`: `Origin` must
   equal a configured origin exactly (scheme, host, port). If `Origin` is absent, `Sec-Fetch-Site`
   must be `same-origin`; if both are absent the request is refused. This is the Fetch Metadata
   defence of the OWASP CSRF cheat sheet. Failure is `auth.origin.refused` (new).
4. No per-form token: Lax plus Origin plus Fetch Metadata covers every browser that can run the
   passkey pages. A token would be a second mechanism to get wrong. (Open choice 8.)
5. **Ceremonies** (register, login, step-up) are bound to a short-lived `__Host-ink_ceremony` cookie
   that holds the random id of the server-side state, so a response from another browser or a
   cross-site post of someone else's assertion cannot complete (login CSRF).
6. The API host never reads a cookie, so none of this reaches the API.

## 9. Bootstrap and recovery

### First run (self-host)

`ink serve --data <folder>` on an empty database prints, once, to stderr:

```
No account exists yet. Open this link on this machine to make the first one:
  http://localhost:8080/welcome/<256-bit token, 43 characters>
It works once, for one hour, and only while there is no account.
```

- The token is 256 random bits; its context-separated BLAKE3 digest is the stored `link` row
  (purpose `bootstrap`). The page shows a button, not an action: opening the link changes nothing
  (an email scanner or a link preview cannot spend it, and a browser prefetch cannot either).
- Pressing the button starts a passkey registration for the new owner account and then adds a recovery
  code set (section 9, recovery codes). The account is made at the end of registration, so a half-run
  leaves nothing behind.
- The link is single use, and it is dead once an account exists. Restarting the server before use
  prints a **new** link and kills the old one. `ink serve --data <folder> --admin-link` prints a new
  one at any time (the folder's owner is the operator, section 10): this is also the self-host
  answer to "I lost every passkey and every code", and it needs no email.
- The token goes through `Link`'s `Debug` of `Link(<redacted>)` (the `signin` agent's type) and the
  server's access log records the route (`/welcome/..`), not the token. The response sets
  `Referrer-Policy: no-referrer` and `Cache-Control: no-store`.

**The LAN case.** `http://192.168.1.20:8080` is not a secure context and an IP address cannot be a
relying party id (WebAuthn needs a domain, 11), so passkeys do not work there at all. The first-run
output says so when the listen address is not loopback and no `[auth] origin` with TLS is set, and
names two ways: open the link at `http://localhost:8080` on the host itself, or forward it
(`ssh -L 8080:localhost:8080 host`) and open `http://localhost:8080` locally. A passkey registered
at `localhost` works at `localhost` only; moving to a real domain later is a relying-party change
(section 11).

### Hosted sign-up

A new hosted account starts from an **email link** (verifying the address) or an authority proof
(GitHub). Either one only gets the person to the **passkey registration page**; the account is not
usable for sign-in until a passkey exists. The email address is kept to send recovery and notices
to, and is the only personal datum the auth tables hold (section 14, database leak).

### Email links

Purposes: verify an address, start a recovery, cancel a recovery. All the same shape:

- A random 256-bit token in the URL path, its digest (context `link token v1`) stored with the
  purpose, the account and an expiry of **15 minutes** (the figure of architecture section 12).
- **A GET shows a confirmation page; a POST from that page spends the link.** Mail scanners and link
  previews GET every link; this is the well-known way a magic link is burned before the person sees
  it. The GET changes nothing.
- Single use: the row is deleted in the same batch that does the effect (0049: atomic batches).
- A used, expired or unknown link is the same `auth.link.refused` (new).
- **There is no magic-link login.** An emailed link never gives a session. Email is a recovery and
  verification channel, because making it a login would make the mailbox the account and undo what
  a passkey buys. The first proposal listed the magic link for bootstrap and recovery only, and so
  does this.

### Recovery codes

Optional, shown once at registration and on request (making a new set voids the old one).

- 10 codes, each 128 bits from `Draw` (the brief's size), shown as 26 Crockford base32 characters in
  groups of four plus a check character, so a typo is caught before it counts as an attempt.
- Stored as `derive_key("uses.ink 2026-11-27 recovery code v1", code)` digests, with a used flag.
  128 random bits need no salt and no slow hash (section 15).
- **A code is spent on a new passkey registration, immediately** (no delay). The person made the codes
  while signed in, kept them offline, and holds them now: that is the pre-authorisation a delay would
  re-ask for. Every use is notified (below).

### Email recovery, with a delay

For a person who has neither a passkey nor a code, in hosted, or in a self-host with SMTP configured:

1. The person gives an address on the recovery page. **The reply is always the same** ("If that
   address has an account, we sent a link"), the work is queued, so time does not tell either.
   Rate limits per address and per client.
2. The link (15 minutes, confirmation page, POST) **starts a recovery**; it does not complete one.
3. A recovery row records `completes_at = now + delay`. The default **delay is 48 hours** (operator
   setting, floor 1 hour, ceiling 30 days).
4. **Notices**: an email to the address, a banner on every signed-in dashboard page, a line in
   `ink whoami` (a typed notice `auth.recovery.pending`, new), and an audit event. Each contains
   the time it completes and **a cancel link**. A cancel link can only cancel, never grant, so it is
   safe to email. Cancel also works from any signed-in session, device key with the `keys` right, or
   passkey.
5. At `completes_at` the person opens a second link (sent then) and registers a passkey.
6. **On completion**: every session ends and every device key is revoked (a lost laptop key is easy to
   re-enrol with `ink login`). Deploy keys survive, so CI keeps working, and the notice lists them.
   Old passkeys stay until the person removes them; the page lists each one with a "remove" box, none
   ticked by default (a recovery by an attacker should not remove the real owner's passkeys, and a
   recovery by the real owner can remove the lost ones in one click).

**The operator can turn email recovery off per account once a passkey exists** (`recovery.email =
off` in the account policy, set from the dashboard with a passkey step-up) **and for the whole
instance** (`[auth] recovery-email = false`). With it off, the paths back are a passkey, a recovery
code, and (self-host) the operator's `--admin-link`. Hosted gives the same advice: set it off once
you have two passkeys or a code set. There is no "ask support" path in hosted, because one is a
social-engineering path; the page says so.

### Email takeover

The delay and the notices are what an attacker who has the mailbox is up against. During the delay
the real owner, who still has a passkey or a session somewhere, sees the banner and cancels. An
owner who has nothing left is exactly who cannot. The delay is a policy dial, and the account switch
above is the strict setting. Residual risk, stated in section 14.

## 10. The operator and the existing token

`edge/api` today has `Owner`: one bearer token, kept as a BLAKE3 digest, compared in constant time,
made by `ink serve --data` into `<data>/owner-token` (0600), shown once (0052). In the new model:

- **`Owner` is deleted, not deprecated.** Nothing is released, so the only holder of such a token is
  the owner's own machines. The server's first start on a folder that has an `owner-token` file
  prints one notice: `note[auth.token.retired]` (new), "the owner token in `<data>/owner-token` no
  longer works; sign in with `ink login --data <folder>`", and **leaves the file alone** (the owner
  deletes it; ink does not remove files it did not make this run).
- **The operator credential is an authorized-keys file**, `<data>/operator-keys`: one line per key,
  `ed25519-v1 <base64url public key> <label>`. Whoever can write the data folder is the operator,
  the same trust the `owner-token` file had. The server reads it per request (a `stat` per request
  is the cost; it is rechecked by mtime) and a key in it signs as the owner account with all rights.
  Nothing secret is in the file, so a backup of `<data>` leaks no credential.
- **`ink login --data <folder>`**, run as the folder's user on the host: generates a key (section
  5), appends its public half to `operator-keys`, and signs in with it. No browser, no email, no
  server round trip for the enrolment; the running server picks the new line up on its next request.
  It is the headless and CI-on-the-same-box path, and the way an operator that never opens a browser
  uses `ink`.
- **Operator keys are not editable through the API.** `ink key list` shows them as `operator (file)`,
  and `ink key revoke` on one says to delete its line. The file is the single place.
- **Hosted has no operator key in tenant accounts.** The uses.ink staff's access to the platform is
  Cloudflare's, outside this scheme.

## 11. Relying party, origins and TLS

The **relying party id** (rp id) and **origins** are operator configuration (`[auth]` in the
operator's settings, 0044 bucket 2):

```toml
[auth]
rp-id   = "app.uses.ink"
origins = ["https://app.uses.ink"]
```

WebAuthn rules (W3C WebAuthn L3, 5.1.3, 5.1.4): the rp id is a domain; the caller's origin's host
must equal it or be a subdomain of it, and a browser refuses an id that is a public suffix. The
page must be in a secure context: HTTPS, or `http://localhost`.

- **Hosted.** `uses.ink` is a public suffix (psl.md), so the id cannot be `uses.ink` for a page at
  `app.uses.ink`, and cannot be shared with `<handle>.uses.ink`. The id is the dashboard's exact
  host, `app.uses.ink`. A side effect worth having: an author site at `<handle>.uses.ink` can
  never ask the browser for a credential of `app.uses.ink`, because that host is neither equal to
  nor a suffix of its own. (This follows from the rules; the browser matrix check in section 22
  confirms it on Chrome, Safari and Firefox before it is relied on.)
- **Self-host with a domain.** The id is a domain the operator controls. Advice in the docs: use the
  **parent** you will keep (`example.com`) with the dashboard at `ink.example.com`, so moving the
  dashboard host later does not invalidate passkeys. (Unless `example.com` is a public suffix.) The
  server **prints its chosen rp id and origins on every start** (nothing is silent), and a default
  derived from the dashboard host says it was derived.
- **Self-host on localhost.** `http://localhost:<port>` with rp id `localhost` works in Chrome and
  Firefox, and the `__Host-` cookie with `Secure` is accepted on `localhost` there; **Safari does not
  accept a `Secure` cookie from `http://localhost`**. For Safari the docs say to use a name with TLS
  (a local `*.localhost` does not help; use a real or a `mkcert` name). This is a fact to verify in
  section 22's matrix, not to assume.
- **LAN IP over HTTP**: no WebAuthn (section 9).

### The id is bound to the credentials

The first passkey stores the rp id it was made under (`auth_config` row). On every start the server
compares the configured id with it. A difference is a refusal to start sign-in, not a silent
break: `auth.relying-party.changed` (new), naming both ids and how many passkeys it would orphan,
and the two ways forward. This is where "a domain change breaks passkeys" is turned from a surprise
into an error.

### A domain change

A passkey is tied to the rp id forever, and an authenticator cannot rewrite it. So a change of id
means passkeys do not work on the new one, and the honest path is **re-enrolment**:

1. Self-host: `ink serve --data <folder> --admin-link` on the new configuration prints a link that
   registers new passkeys for the owner account (the folder's owner is the operator). Old passkeys
   are listed as orphaned and removed after.
2. Hosted: recovery codes or email recovery, as in section 9, on the new host.
3. **Related Origin Requests** (WebAuthn L3, "related origins") are the bridge: the **old** domain
   serves `https://<old>/.well-known/webauthn` listing the new origins, and the new site keeps using
   the old rp id. Chrome and Safari support it; as of January 2026 Firefox was still considering it
   (web.dev), and the spec limits the number of distinct labels it requires a browser to accept.
   So ROR keeps passkeys working for most people while the old domain stays up, and cannot be the only
   plan. It is documented as an option, not built in v1.

## 12. Rights, scopes and orgs

### The types

```rust
/// What a credential may do. A closed list; adding one is a code change and a decision.
#[non_exhaustive]
pub enum Right { Read, Deploy, Edit, Destroy, Keys, Members }

/// Where a right applies.
#[non_exhaustive]
pub enum Scope { Account(Id<AccountKind>), Site(Id<SiteKind>) }

pub struct Grant { pub scope: Scope, pub rights: Rights }   // Rights is a set of Right
```

| Right | Lets the holder |
|---|---|
| `Read` | list and read sites, deploys, builds, logs, export |
| `Deploy` | intake a built folder, request a build, roll back, resume |
| `Edit` | add a site, change its settings |
| `Destroy` | remove a site (the receipt of M4), delete the account |
| `Keys` | list keys and sessions, **revoke** a device or deploy key |
| `Members` | (orgs) invite, remove, change a member's role |

`Right` has a `NAMES` table like `credential::Use` does, and the column that stores it is built
from it (0049).

The calls of `edge/api` today need these rights:

| Call | Right |
|---|---|
| `Sites`, `Status`, `Deploys`, `Builds`, `Log`, `Export` | `Read` |
| `Add` | `Edit` |
| `Remove` | `Destroy` |
| `Rollback`, `Resume`, `Build`, `Intake` | `Deploy` |

### How they compose

- **Union within a principal, no deny.** The rights a principal has on a site are the union of its
  grants whose scope covers it: `Account(a)` covers every site of `a`; `Site(s)` covers `s`. No grant
  removes another, so an effective right is always explainable by naming the grant that gives it.
- **A key is an attenuation.** A device key acts for a person: its effective rights are **the key's
  grants intersected with that person's current grants**, so demoting a person narrows their keys the
  same moment. A deploy key belongs to the account or org (a service identity): its effective rights
  are its own grants, within what the owning account may do. Neither can exceed what created it:
  creation checks the new grants are within the approver's.
- **Least privilege by default.**
  - A device key: `Read`, `Deploy`, `Edit`, `Keys` on the approver's account scope. Not `Destroy`,
    not `Members`. Deleting a site is done in the dashboard, with a passkey.
  - A deploy key: `Deploy` on one site.
  - An org member's key never exceeds the member's role.
- **Narrowing is cheap, widening needs a human.** `Keys` lets a key list and **revoke**. Nothing lets a
  key **mint** a credential: every enrolment of a device or deploy key, every new passkey, every
  change of rights is approved by a passkey assertion at a step-up. A stolen laptop key can deploy; it
  cannot make itself immortal by minting keys, and revoking is allowed because a stolen key that revokes
  harms availability and nothing else. This is a change from the brief, which did not say; section 21
  and open choice 5.

### Orgs and roles (hosted, M4)

An account has a kind: person or org. An org has members; a member has a **role**, which is a named
set of rights in code, not a row (no custom roles in v1):

| Role | Rights on the org's sites |
|---|---|
| Owner | all, including `Members` and `Destroy` |
| Admin | all but `Destroy` of the org |
| Writer | `Read`, `Deploy`, `Edit` |
| Reader | `Read` |

A member's grant is `Account(org)` with the role's rights, or `Site(s)` for a member who works on
one site. A deploy key of an org outlives the member who made it (it belongs to the org) and shows
"creator removed" in the list; the org's owner decides whether to revoke it. An org's last owner
cannot be removed. Roles and the org tables arrive with M4; self-host M3 has one account and the
`Owner` role only, so nothing in sections 4 to 11 waits on orgs.

## 13. Audit and "last used"

Two kinds of record, kept apart so the second does not turn into a tracking log.

- **Lifecycle events** are typed values, `control::Event` variants (the enum that exists in
  `control/src/event.rs`), stored with the control plane's other events:
  `PasskeyAdded`, `PasskeyRemoved`, `PasskeyFlagged { why }`, `SessionStarted`, `SessionEnded { why }`,
  `StepUp`, `KeyApproved { key, kind, rights, scope }`, `KeyRevoked { key, why }`, `KeyExpired { key }`,
  `KeyRotated { old, new }`, `RecoveryStarted { completes_at }`, `RecoveryCancelled`,
  `RecoveryCompleted`, `RecoveryCodeUsed`, `RelyingPartyChanged`, `ReplayDetected { key }` (once per
  hour per key at most, and only after a valid signature). Each names the credential and the account
  and never the secret. Default retention 180 days, operator setting.
- **Last used** is **a date on the credential's own row**, `last_used_on`, written when the date
  changes (at most once a day per credential). No per-request row, no IP, no user agent, no path. The
  key list says "last used 2026-11-27" and that is all the server remembers of ordinary use.
- **Refusals** are counted, not logged: counters by code and hour (`auth.signature.refused`: 41) for
  the operator's metrics, with no per-attempt detail. Attempts against a known key that fail its
  signature are not recorded under that key, because anyone can send them.

## 14. Threat model

| Threat | What the attacker gets and how the design answers | Residual |
|---|---|---|
| **Database leak** | Passkey public keys, device and deploy public keys, digests of session ids, link tokens and recovery codes, email addresses, labels. Each digest is of 128 to 256 random bits, so it cannot be searched for a preimage and there is nothing to replay. | Email addresses and key labels are personal data. Encrypting the email column under the `secret` port's key is an option (open choice 10). |
| **Server compromise (live)** | The server is the authority: an attacker with its write access can add a passkey or a key, mint sessions, read private repos' sealed tokens in memory. No design that has the server decide who is signed in survives this. | Detection only: lifecycle events go to the operator's log sink; `ReplayDetected` and `KeyApproved` are the alarms. Hosted separates the builder container from the database (0046). |
| **Phishing** | A fake dashboard cannot obtain an assertion for `app.uses.ink` (the authenticator checks the rp id). A phished device-code approval is answered by the fingerprint comparison, the typed code and the passkey (section 6). A phished recovery link only starts a delayed recovery with notices. | A person who ignores the fingerprint mismatch and the page's warning, and approves anyway. |
| **Token theft: session** | `HttpOnly` stops script; `__Host-` and `Secure` stop downgrade and shadowing; absolute and idle expiry bound it; the session list ends it. | Malware on the person's own machine reads the browser's memory. Out of scope. |
| **Token theft: device key** | Read from the OS keystore or the 0600 file by a process of the same user. Expiry (180 days), revocation, and no right to mint credentials bound what it does. | Same as an SSH key on a laptop. Hardware-bound `p256-v1` is the later answer. |
| **Token theft: deploy key** | Leaked from CI. Scope is one site and `Deploy`; expiry 90 days; revocation; `auth.key.expiring` makes rotation routine. | Until revoked it can push garbage to that one site (the previous deploy is a rollback away). |
| **Replay** | A captured signed request is useless to anyone but the same server in the same window: `aud`, `created`, the nonce store, the boot rule. | A replay to the same server inside the window if the nonce store were writable by the attacker. |
| **Email takeover** | Recovery by email takes 48 hours, notifies every device and the address, and can be cancelled from anywhere signed in. Per account it can be switched off. There is no magic-link login. | An owner with no passkey, no code and no session, whose mailbox is taken, loses the account. That is the price of email recovery; turning it off moves the price to "lose all three and lose the account". |
| **CSRF** | The API host has no ambient credential. The dashboard uses Lax, Origin, Fetch Metadata and no state change on GET. | A browser that sends neither `Origin` nor `Sec-Fetch-Site` on a POST is refused, so it cannot use the dashboard. Acceptable. |
| **Enumeration** | Login asks for no identifier (discoverable credentials). Recovery and enrolment replies are uniform and queued. An unknown key id and a wrong signature give the same reply. Handles are public by design. | The address of an account can be learned by someone who already has the mailbox. |
| **Lost device** | Another passkey, a recovery code, or email recovery with delay; revoke the lost device's keys from the dashboard. | Having one passkey, no codes and no second device. The setup flow asks for a second passkey or codes before it finishes. |
| **A malicious plugin or build job reaching credentials** | Builds and plugins never see credentials (0046, 0055): the hosted builder has no database binding and no deploy key; plugins are wasm with no file or environment capability; the source plugin holds a grant's name. | A build step in the author's own CI that runs repository code while the deploy key is in its environment. The docs show the push step with the key alone. |
| **Supply chain** | New dependencies are few and pure Rust (section 15); each licence exception is a decision; `cargo deny` gates advisories; `cargo vet` or a pinned reviewed set is an option. `dist` releases are the channel for the CLI. | A compromised crate in the signing path. Test vectors and a conformance suite catch a verifier that changes behaviour. |
| **Clock skew** | `auth.signature.skewed` with the server time and the number of seconds; the CLI says how far off the clock is and does not fix it. | A client whose clock is wrong cannot use the API until it is fixed. By design. |
| **Nonce store loss on restart** | Self-host: the boot rule refuses any `created` before `boot + skew`. Hosted: the store is the Durable Object's storage, or a D1 unique key. | Up to one skew window of "try again" after a restart. |
| **Passkey provider breach** | A synced passkey is only as safe as its provider's account. The BE and BS flags are stored and shown so the person knows which of their passkeys sync. | Not ours to fix. An account with only a synced passkey inherits its risk. |
| **Quantum** | Section 17. | Forgery needs a quantum computer at the moment of a request. |

## 15. Crypto and crates

### Primitives

| Use | Primitive | Why this, and not another |
|---|---|---|
| Signatures of CLI and CI requests | **Ed25519** (RFC 8032), strict verification | Deterministic (no nonce reuse failure), 32-byte keys, fast, small, one algorithm. |
| Passkey assertions | **ES256** (ECDSA P-256), by WebAuthn | What authenticators do. Not our choice, and verified by the library. |
| Digests of random tokens (session id, link token, recovery code) | **BLAKE3**, `derive_key` with one context per purpose, 256-bit output | The inputs are 128 to 256 bits of OS randomness, so the digest only has to be one-way. **No salt**: a salt defends against precomputed tables of guessable inputs, and these are not guessable. **No slow hash** (Argon2, scrypt): slowness buys resistance to guessing a low-entropy secret, which these are not, and costs CPU on every request. A password would need both; there are none. Per-purpose `derive_key` contexts mean a session digest cannot be presented as a link token digest. BLAKE3 is already in the tree (`control/rows/src/digest.rs`, blob hashes). |
| Body digest | **BLAKE3** | One hash across the product (section 5). |
| Comparing digests | `blake3::Hash`'s `==` (constant time, documented), `subtle::ConstantTimeEq` where bytes are not a `Hash` | `subtle` is already an exception in `deny.toml` for the sealer. A database lookup by digest is not a secret comparison; its timing reveals a digest prefix, from which no preimage follows. |
| Randomness | `clock::Draw` (16 bytes, OS CSPRNG in `System`, counting in `Counter`) | A token is two draws. A key seed is two draws. One exception: the WebAuthn challenge, from the library's OS-seeded `rand` (section 4). |

### Crates

Checked 2026-11-27 on crates.io, in the RustSec database snapshot in `~/code/strangers/advisory-db`
(2026-11-26), and in clones under `~/code/strangers`.

| Crate | Version | Licence | Last release | Advisories | Verdict |
|---|---|---|---|---|---|
| `webauthn-rs-core` (and `-proto`, `webauthn-attestation-ca`, `base64urlsafedata`, `crypto-glue`) | 0.6.2; `crypto-glue` 0.2.3 | **MPL-2.0** | 2026-10-31 | none in RustSec for `webauthn-rs` | **In**, behind the `Passkeys` port, after the wasm spike. Kanidm's project, about 10M downloads, 60-day coordinated disclosure policy. I found no published audit of the current crypto layer. |
| `ed25519-dalek` | 3.0.0 | **BSD-3-Clause** | 2026-07-06 | RUSTSEC-2022-0093 (fixed in 2.0); `curve25519-dalek` RUSTSEC-2024-0344 (fixed in 4.1.3, now 5.0.0) | **In, recommended.** Pure Rust, `verify_strict`, builds for wasm. Needs two licence exceptions (`ed25519-dalek`, `curve25519-dalek`). The Quarkslab audit I know of was of `curve25519-dalek` in 2019; I could not confirm one of the 3.x and 5.x series. |
| `ed25519-compact` | 2.6.1 | **MIT** | 2026-11-07 | none | **The no-exception alternative.** One maintainer, no required dependencies, rejects non-canonical `S` and small-order keys. Smaller review surface and smaller community. Open choice 2. |
| `ring` | 0.17.14 | Apache-2.0 AND ISC | 2025-03-11 | none for it | Out: C and assembly, awkward on `wasm32-unknown-unknown`, ISC exception. |
| `aws-lc-rs` | 1.19.0 | ISC AND (Apache-2.0 OR ISC) | 2026-10-28 | none for it | Out for the same reasons, plus a C build. |
| `lettre` | 0.11.24 | MIT | 2026-11-17 | six on 2026-11-17, all patched in 0.11.24 | **In, self-host only**, pinned `>= 0.11.24`, through the `signin` agent's `mail/smtp`. Hosted email is another adapter of that `mail` port. Not for `wasm32`. |
| `keyring` | 4.2.0 | MIT OR Apache-2.0 | 2026-08-29 | none | **In** (CLI only). Platform stores are separate crates; a missing store is the file fallback. |
| `blake3` | 1.8.8 | CC0-1.0 OR Apache-2.0 (workspace has it) | 2026-11-26 | none | In, already used. |
| `coset` | 0.4.2 | Apache-2.0 | 2026-03-02 | none | Not needed: the library parses COSE. |
| `opaque-ke` | 4.0.1 | MIT OR Apache-2.0 | 2026-11-13 | none | Out (section 16). |

`deny.toml` changes if choice 2 and 3 go as recommended, each an exception with its reason, as the
file's comment requires: `webauthn-rs-core`, `webauthn-rs-proto`, `webauthn-attestation-ca`,
`base64urlsafedata`, `crypto-glue` (MPL-2.0: file-level copyleft, compatible with AGPL-3.0, and we
modify none of it), `ed25519-dalek`, `curve25519-dalek` (BSD-3-Clause, as `subtle` already is). The
exact list is what `cargo deny check licenses` prints in the spike; a transitive crate may add
another. Also checked in the spike: the lock file resolves `rsa >= 0.10`, and `serde_cbor_2` (a
fork of the unmaintained `serde_cbor`) has no advisory.

## 16. Alternatives considered

| Alternative | Verdict | Reason |
|---|---|---|
| **DPoP** (RFC 9449) | Out | A DPoP proof is a JWT signed per request over method, URL, time and a `jti`, bound to an access token. That is our signed request with more parts: JWT parsing (the `alg: none` and header-supplied `jwk` class of bugs), an authorization server that issues tokens, and token binding we do not need because the key itself is the credential. Per-request signatures without a token is DPoP minus the token. |
| **mTLS** | Out | Needs the TLS terminator to check client certificates. Hosted: a Worker sees a client certificate only through Cloudflare's mTLS feature (`request.cf.tlsClientAuth`, zone-level, a plan-dependent product), and `workers-rs` has no binding for it that I could find. Self-host: the common deployment is `ink serve` behind Caddy, nginx or Traefik, which terminate TLS and make client-certificate forwarding an operator task per proxy. A CA, certificate lifetimes and revocation lists are a second system. A signed request survives every proxy that leaves the target and body alone. |
| **OAuth device grant with bearer access tokens** | Out | Bearer tokens are what we are removing: copied from a log, replayable. A device grant stays in the *enrolment*, as section 6. |
| **OPAQUE** (RFC 9807) | Out | An augmented PAKE keeps the password from the server, and is the best way to have a password. It is still a secret a person chooses, typed on a form a phisher can imitate, and the owner's brief is no passwords. `opaque-ke` 4.0.1 is maintained; it is the thing to reach for if a device without a passkey ever has to be supported. |
| **SRP** | Out | Older, no RFC-standard modern parameter set in use, same objection as OPAQUE. |
| **PASETO, Biscuit, macaroons** | Out for v1 | Their value is delegation: a holder narrows a token and passes it on. Builds and plugins never see credentials (0046, 0055), so there is no one to delegate to. A share link or a build-scoped token could use Biscuit later. |
| **Own WebAuthn verifier** | Fallback | About a thousand lines for attestation `none` and ES256, no MPL dependency, no `crypto-glue`. Signature and CBOR parsing is easy to get subtly wrong, hence the library first (open choice 3). |
| **Magic link as login** | Out | Makes the mailbox the account (section 9). |

## 17. Post-quantum, plainly

- **Confidentiality** is the risk that exists now: traffic recorded today and decrypted later.
  Nothing in this scheme is secret in transit beyond a session cookie, and TLS key exchange is the
  layer that covers it. Hybrid X25519 with ML-KEM is already the default in current browsers and on
  Cloudflare; the self-host edge's TLS (rustls) is a deployment setting, which the docs note.
- **Signatures** are only forged by a quantum computer that exists **at the moment of the request**:
  a recorded signed request is bound to one server, one moment and one nonce, and the stored public
  keys give nothing to harvest. So nothing signed today is endangered later.
- **WebAuthn** passkeys are ES256 today; post-quantum COSE algorithms are still drafts. The platforms
  move first, and the relying party follows by allowing the new algorithm.
- **The migration** is the versioned key type: a new type (a hybrid such as ML-DSA with Ed25519, or
  whatever standardises) is a new stored type and a new `Ink-Sig-2`, enrolled with a passkey; the
  operator's `key-types` list retires `ed25519-v1` when it should. Nothing in the data model assumes
  Ed25519's sizes (public key and signature columns are blobs with a type).

## 18. Data model

Tables are enum types and migrations are Rust values, as 0049 says. No SQL text. A sketch:

```rust
#[derive(IdenStatic)] #[iden = "passkey"]
pub enum Passkey { Table, CredentialId, AccountId, PublicKey, SignCount, BackupEligible, BackupState,
                   Transports, Label, CreatedAt, LastUsedOn }

#[derive(IdenStatic)] #[iden = "key"]
pub enum Key { Table, Id, AccountId, Kind, KeyType, PublicKey, Label, Rights, ScopeKind, SiteId,
               CreatedAt, CreatedBy, ExpiresAt, LastUsedOn, RevokedAt, RevokedWhy, Replaces }

#[derive(IdenStatic)] #[iden = "session"]
pub enum Session { Table, Digest, AccountId, Label, CreatedAt, LastSeenAt, ExpiresAt, SteppedUpAt, PasskeyId }

#[derive(IdenStatic)] #[iden = "ceremony"]
pub enum Ceremony { Table, Digest, Purpose, AccountId, State, ExpiresAt }

#[derive(IdenStatic)] #[iden = "enrolment"]
pub enum Enrolment { Table, Id, UserCode, KeyType, PublicKey, Label, Purpose, SiteId, Rights, ExpiresIn, State,
                     CreatedAt, ExpiresAt, ApprovedBy }

#[derive(IdenStatic)] #[iden = "link"]
pub enum Link { Table, Digest, Purpose, AccountId, ExpiresAt }

#[derive(IdenStatic)] #[iden = "recovery_code"]
pub enum RecoveryCode { Table, Digest, AccountId, UsedAt }

#[derive(IdenStatic)] #[iden = "recovery"]
pub enum Recovery { Table, Id, AccountId, StartedAt, CompletesAt, State }

#[derive(IdenStatic)] #[iden = "auth_config"]
pub enum AuthConfig { Table, RpId }                     // the id the first passkey was made under
```

`account` gains `email` (nullable, and `email_verified_at`) and `recovery_email` (allowed or off).
`Kind`, `KeyType`, `ScopeKind`, `Purpose`, `State` and `Rights` each have a `NAMES` table the column's
`CHECK` is built from. `ceremony.State` is the library's serialised state, sealed by the `secret`
port when it is not already short-lived (it holds a challenge and expires in 5 minutes, so sealing is
a choice, not a need).

```rust
#[derive(Clone, Copy, Debug)]
pub struct Auth;                                         // the migration after Domain

impl Migration for Auth {
    const NUMBER: u32 = 2;
    const NAME: &'static str = "auth";
    fn statements() -> Vec<Statement> {
        [Self::passkeys(), Self::keys(), Self::sessions(), Self::ceremonies(), Self::enrolments(),
         Self::links(), Self::recovery()].concat()       // each a Table::create() builder expression
    }
}
```

Pending rows (ceremony, enrolment, link) expire by their time; a sweep deletes them with the batch
that handles the next request, not a background task. A session lookup is one indexed read by
digest; a signed request is one indexed read by key id (a key cache with a short TTL is an
optimisation that waits for a measurement).

## 19. API, the port and the CLI

### Hosts

| Host | Reads | Routes |
|---|---|---|
| API (`api.uses.ink`, `--api-host`) | `Authorization: Ink-Sig-1 ...` only; ignores cookies; no CORS headers | `/v1/...` below |
| Dashboard (`app.uses.ink`) | the session cookie; accepts no signature | HTML pages and the passkey ceremony endpoints (`/auth/...`), served by `web` |

### Routes under `/v1`

Added to `edge/api`'s `Call` enum next to the site routes. All are signed unless noted.

| Route | Right | Does |
|---|---|---|
| `POST /v1/enrol` (unsigned) | none | Begin an enrolment (section 6). Body: public key, type, label, purpose. Rate limited. |
| `GET /v1/enrol/{id}` | signed by the **pending key** | Poll: `pending`, `approved`, `denied`, `expired` |
| `GET /v1/whoami` | any | Account, the key (id, kind, label, rights, scope, expiry), pending notices |
| `GET /v1/keys` | `Keys` | The account's keys: id, kind, label, rights, scope, created, expires, last used (a date), state |
| `DELETE /v1/keys/{key}` | `Keys`, or the key itself | Revoke; `ink logout` is this on one's own key |

There is no route that creates a credential other than `POST /v1/enrol`, and that one needs a
passkey's approval on the dashboard. Sessions and passkeys are managed on the dashboard only.

### The `Credentials` port, extended

Today: `Credentials::verify(&Bearer) -> Option<Principal>`, with `Request` carrying only the
`Authorization` bytes. A signature needs the method, target, body digest, audience and a clock, so
the port takes a typed credential and a request context. This must be reconciled with the `signin`
agent's work in progress (a `Credential` enum with `Link` first, an open `Credentials` adapter),
which had not landed when this was written; the sketch below is the shape this design needs, not a
claim about that code.

```rust
#[non_exhaustive]
pub enum Credential {
    Link(Link),               // a one-time link token (bootstrap, recovery, cancel)       (signin agent)
    Session(Session),         // the dashboard cookie, already parsed                       (dashboard host)
    Signed(Signed),           // Ink-Sig-1: kid, created, nonce, digest, signature          (API host)
    Passkey(Assertion),       // a WebAuthn assertion, for login and step-up                (dashboard host)
    // later: Workload(Token) for OIDC trusted publishing (open choice 6)
}

pub trait Credentials {
    type Error: Error + 'static;
    async fn verify(&self, credential: &Credential, at: &Presented<'_>) -> Result<Verdict, Self::Error>;
}

pub struct Presented<'a> { pub method: Method, pub target: &'a Target, pub audience: &'a Authority, pub now: Timestamp }
pub enum Verdict { Granted(Principal), Refused(Refused) }  // Refused is typed; its reply is chosen in section 5's table
pub struct Principal { pub account: Id<AccountKind>, pub via: Via, pub rights: Effective }
```

How each variant is verified:

| Variant | Checks, in order |
|---|---|
| `Signed` | skew, key by id, strict Ed25519 over the signing input, key live (not revoked, not expired, type accepted), nonce claim, rights by the call. Then, after the body is read, the digest. |
| `Session` | digest of the id, row by digest, `expires_at` and idle limit, Origin check on non-GET, rights of the account (a session has the person's full rights; Destroy and credential changes need step-up). |
| `Passkey` | ceremony row by digest of the cookie id (deleted in the same batch), library verification, UV, BE unchanged, counter policy, account by credential id, `last_used_on`. |
| `Link` | digest, purpose, expiry, POST only; the effect and the row's deletion in one batch. |

`Request` gains `query: String`, and the `Signed` header's bytes ride in `authorization` as they do
now, so the HTTP adapter changes by one field. `Bearer`, `BearerError` and `Owner` leave with the
token (section 10). `Fault` gains the `Denied` variants of section 20 in place of `Missing`,
`Malformed`, `Unknown`.

### CLI

Commands of `cli.md`'s Publish group, already named there: `login`, `logout`, `whoami`, and a new
noun group **`key`** (`list`, `add`, `revoke`; four verbs would pass the rule of three, and `key` is not
the folder). Exit codes: sign-in and key failures are **3**; the server restarting is **4**; a key that
needs the dashboard and is not given it is **2** (a usage fix).

```
ink login [--server URL] [--label TEXT] [--data DIR]
ink logout
ink whoami
ink key list
ink key add    --site SITE --right deploy [--right read] [--expires 90d] [--replaces KEY] [--out FILE]
ink key revoke KEY
```

Output uses the ten blocks of 0061: `login` is stage lines and a record; `whoami` a record; `key list` a
table; `key add` stage lines, then a result (the key, only when it may be printed); errors are the
error block with a code, causes, fix and state. Mock-ups of every one are in `auth-preview.html`; two
are here.

```
$ ink whoami
ada  on api.uses.ink
  key        7QKM-2H4P-X9  "laptop"  device, ed25519-v1
  rights     read, deploy, edit, keys on every site of ada
  expires    2027-05-26
  notice     auth.recovery.pending: a recovery of this account completes 2026-11-29 14:03Z;
             cancel it in the dashboard if it was not you

$ ink key list
  key            kind    where             rights        expires     last used
● 7QKM-2H4P-X9  device  laptop            read deploy   2027-05-26  2026-11-27
● 3DSF-7K2M-QA  deploy  garden (CI)       deploy        2027-02-25  2026-11-26
· operator-1    device  operator (file)   all           never       2026-11-27
✗ 9WHB-4C1N-ZE  deploy  garden (old CI)   deploy        revoked     2026-10-02
```

A mock-up's glyphs follow `cli.md` section 5: `●` live, `✗` failed or revoked, `·` neutral.

## 20. Errors, for `diag/catalog.toml`

Each has a code (`noun.noun.reason`), a cause (what is true), a fix (what to do), a `fault` and the
**fields it carries**. None names a key, an address or a secret, and none distinguishes an unknown
credential from a wrong one unless the caller proved the key. The wording is plain, in the voice of
the catalog. Codes marked (new) do not exist yet.

| Code | Severity, fault | Fields | Cause | Fix |
|---|---|---|---|---|
| `auth.signature.malformed` | error, author | `header` | The `Authorization` header is not an `Ink-Sig-1` value. | Update `ink`; a script that signs its own requests follows the profile in `docs/auth.md`. |
| `auth.target.not-canonical` | error, author | `target` | The request target is not in canonical form. | Escape with upper-case hex, sort the query, drop empty pairs. `ink` does this itself. |
| `auth.signature.skewed` | error, source | `skew`, `window` (counts, seconds), `server_time` | The request's time is `skew` seconds away from the server's, over the allowed `window`. | Set this machine's clock (`timedatectl set-ntp true`). |
| `auth.signature.refused` | error, source | none | The server did not accept the signature. | Run `ink login` to sign in again on this machine. |
| `auth.key.revoked` | error, source | `since` | This key was revoked. | Run `ink login`; for CI, make a new key with `ink key add`. |
| `auth.key.expired` | error, source | `since` | This key has expired. | Same. |
| `auth.key.expiring` | note | `days` | This key has `days` days left. | `ink key add --replaces` before then. |
| `auth.signature.replayed` | error, ours | none | A request with the same nonce was already seen. | Send a new request; a client that reuses a nonce has a bug. |
| `auth.digest.mismatch` | error, author | none | The body is not the one that was signed. | Resend; a proxy that changes bodies must not sit between. |
| `auth.server.restarting` | error, operator | `seconds` | The server just started and takes signed requests again in `seconds`. | Wait; `ink` retries once. |
| `auth.right.missing` | error, author | `right`, `scope` | This key may not do this. | Use a key with the right, or approve one in the dashboard. |
| `auth.session.expired` | error, source | `limit` (`idle` or `absolute`) | The session ended. | Sign in again. |
| `auth.origin.refused` | error, author | none | The request did not come from this dashboard. | Reload the page and try again. |
| `auth.passkey.refused` | error, source | none | The passkey was not accepted. | Try again, or use another passkey or a recovery code. |
| `auth.enrolment.expired` | error, source | none | That code is unknown or has expired. | Run `ink login` again. |
| `auth.enrolment.denied` | error, source | none | The request was not approved. | Run `ink login` again if it was a mistake. |
| `auth.link.refused` | error, source | none | That link is unknown, used or expired. | Ask for a new one. |
| `auth.recovery.pending` | note | `completes_at` | A recovery of this account completes at that time. | Cancel it in the dashboard if it was not you. |
| `auth.recovery.disabled` | error, source | none | Email recovery is off for this account. | Use a passkey or a recovery code. |
| `auth.relying-party.changed` | error, operator | `was`, `now`, `passkeys` | The domain is not the one passkeys were made for; `passkeys` would stop working. | Restore the setting, or run `ink serve --admin-link` and register new passkeys on the new domain. |
| `auth.key.print-refused` | error, author | none | The private key would be printed to a terminal. | Send it to a file with `--out`, or redirect stdout. |
| `auth.token.retired` | note | `file` | The owner token in `file` no longer works. | `ink login --data <folder>`; delete the file. |

The caller never sees the cause for `auth.signature.refused`; the operator's counters do. A reply's
`Problem` is as 0052 made it: `problem`, `message`, `causes` (empty for the server's own failures).
`auth.signature.skewed` adds `server_time`.

## 21. What changed from the first proposal

| First proposal | This design | Why |
|---|---|---|
| Passkeys, user verification **preferred** | **Required**, checked in the authenticator data | No second factor exists; presence alone makes a held key the whole account |
| Passkeys as discoverable credentials, `webauthn-rs` | Discoverable, `residentKey: required`, **on `webauthn-rs-core` behind a port** | The high-level API discourages and disables the flow; section 3 |
| Sign counter or backup flags | Both stored; counter used only when nonzero and for non-synced keys | Synced passkeys report 0 |
| "A small typed profile modelled on RFC 9421" | A **fixed one-header profile** with strict parser, not wire-compatible with 9421 | Removes the footguns of 9421 we do not need; section 5 |
| Covers method, path, query, body digest, timestamp, nonce | Same, plus **audience**, key id, and a context line; the target must be canonical or is refused, not repaired | Cross-instance replay, key substitution, proxy normalisation |
| Nonces kept for the window | Plus a **boot barrier** (self-host) and a Durable Object or unique key (hosted) | Memory is lost on restart |
| CLI approves in the dashboard with a passkey | Same, **plus a key fingerprint shown on both screens and no prefilled code**; polling is signed by the pending key | RFC 8628 5.4 remote phishing |
| Device keys could be general | A device key **cannot mint any credential**; it can revoke | A stolen key must not make itself permanent |
| CI deploy keys | Same, expiry required (90 days default), rotation with overlap, an expiry notice; **OIDC trusted publishing** as the later secretless path | Secrets in a CI store still leak |
| Magic link for bootstrap and recovery | Same, but **never a login**; a GET never spends a link; links are confirmed by POST; the first-run link is also the operator's `--admin-link` | Mail scanners; the mailbox must not be the account |
| Recovery codes optional | Same; a code enrols immediately, email recovery waits 48 hours with notices and cancel | A held code is pre-authorised; a mailbox is not |
| Owner token becomes an operator device key | `Owner` deleted; an **authorized-keys file** `<data>/operator-keys`, with `ink login --data` | Same trust as the token file, nothing secret in `<data>`, no browser needed |
| rp id and origin config | Plus the **id is stored with the first passkey and checked on start** | Turns a silent breakage into a typed error |
| Libraries `ed25519-dalek` or `ring` | `ed25519-dalek` recommended, `ed25519-compact` the no-exception alternative; `ring`, `aws-lc-rs` out | `wasm32-unknown-unknown` and the licence list |

## 22. Testing and review plan

**Test vectors** are committed files with typed readers, the same way `fixture` and `host/image`
goldens are:

- Ed25519: RFC 8032 section 7.1 vectors; the Wycheproof Ed25519 set (malformed, non-canonical `S`,
  small-order keys); the vectors of Chalkias et al. for the cofactor and strictness cases.
- The profile: a file of (key, request, signing input, signature) with every field boundary, and
  **cross-checked by a second implementation** (a 40-line Python script run once, vectors kept) so one
  bug in `ink-sig` cannot hide in both signer and verifier.
- Digests: BLAKE3 vectors and one per `derive_key` context, so a context string is never edited by
  accident (changing one invalidates every stored digest).
- WebAuthn: recorded registrations and assertions (a software authenticator, an Apple, a Google, a
  YubiKey; at least one BE-set credential with counter 0 and one hardware credential with counter
  above 0).

**A conformance suite per credential kind**, a crate of cases that are types with a `check` over any
adapter, like `db/conformance` (architecture section 9): `signed`, `session`, `passkey`, `link`,
`recovery`. A new adapter (D1, a Durable Object nonce store) runs the same cases. Required cases
include: replay (same nonce twice, in the window and after it), expiry (key, session absolute, session
idle, link, enrolment), **tamper** (flip each field of the header, each line of the signing input, a
byte of the body), **enumeration** (the reply and the status for an unknown key id and a wrong
signature are equal; same for unknown and used links, unknown and expired user codes; timing within a
bound for unknown and known emails on recovery), skew (both sides of the window, with `clock::Manual`),
boot barrier (a restart with `Manual`), revocation takes effect on the next request, a demoted person's
keys narrow, step-up expires, a session id never appears in a log line or a `Debug` (`Session(..)`).

**A fuzz target for the signature profile parser**: the header bytes to `Signed`, with the invariant
that anything accepted re-encodes to the same bytes (one wire form), and that every field fits its
alphabet (no `\n`). A second target for the canonical-target check against `Target::parse`. Both run
under `cargo fuzz` and for a fixed number of iterations in CI.

**Spikes before M3 depends on them**, each ending in a line in `decisions.md`:

1. `webauthn-rs-core` builds and passes its tests on `wasm32-unknown-unknown`, with the exact licence
   list `cargo deny` prints.
2. A browser matrix: register and sign in with a discoverable, UV-required credential on Chrome,
   Safari and Firefox with a platform authenticator and a security key; the `localhost` and
   `__Host-` cookie facts of section 11; the PSL claim of section 11.
3. The key fingerprint and user-code screens read aloud and in a screen reader (`cli.md` section 14).

**External review checklist**, for someone who did not write it:

- Does any request path reach a handler without a `Verdict::Granted`? (The router is one enum; the
  review reads `Call` and its right.)
- Does a digest of one purpose verify as another? (`derive_key` contexts are unique and tested.)
- Is any secret in a `Debug`, a log line, an error cause or a URL logged? (`Link`, `Session`, `Key`
  print redacted; the access log records routes, not tokens.)
- Is every comparison of a secret constant time, and every comparison of a digest a `Hash` `==`?
- Can a caller learn from a reply whether a key id, an address, a code or a link exists?
- Does a revoked key work for one more request anywhere (a cache)?
- Is the nonce claimed before the signature is verified, or after a failure?
- What does each credential do after a clock jump of a minute either way?
- Is Ed25519 verification strict, and are public keys ever taken apart from a seed?
- Do the dashboard's responses set `Referrer-Policy`, `Cache-Control: no-store` where a token appears?
- What happens in the second after `ink serve` restarts?

## 23. Open choices

The owner decides; each has a recommendation.

1. **User verification required, not preferred.** *Recommend required.* With no password and no second
   factor, presence alone makes a stolen or unlocked authenticator the whole account. Cost: a
   security key without a PIN cannot register.
2. **The Ed25519 crate:** `ed25519-dalek` 3.0.0 (two BSD-3-Clause exceptions, the larger community and
   the code most reviewed) or `ed25519-compact` 2.6.1 (MIT, no exception, one maintainer, tiny).
   *Recommend `ed25519-dalek`*, with the conformance suite making a swap a test run. Accept the three
   MPL-2.0 and two BSD-3 exceptions as one decision about the whole auth dependency set.
3. **The WebAuthn implementation:** `webauthn-rs-core` behind a port (recommended), the `webauthn-rs`
   wrapper (needs a feature the authors discourage for usernameless login), or our own verifier.
   *Recommend core behind the port, gated on the wasm spike*; if the spike fails, our own verifier of
   attestation `none` and ES256 is the fallback, not the wrapper.
4. **Discoverable passkeys with `residentKey: required`** (no username on the login page) or
   identifier-first. *Recommend discoverable.* Identifier-first reintroduces enumeration (it needs decoy
   credential ids) and a field to type.
5. **A device key cannot mint credentials** (every new key and passkey needs a passkey approval), or a
   device key with the `Keys` right may issue narrower deploy keys itself. *Recommend the strict
   version.* The convenient version lets a stolen laptop key make keys that outlive its revocation.
6. **CI without secrets:** ship deploy keys only in v1 and add OIDC trusted publishing (GitHub first)
   for hosted later, or build both for M4. *Recommend deploy keys now, OIDC after M4's first launch.*
7. **Nonce store on restart:** memory plus the boot barrier (up to `skew` seconds of "try again"),
   or persist nonces in the database. *Recommend the barrier*: no write on every request.
8. **CSRF tokens:** Lax plus Origin plus Fetch Metadata only (recommended), or add a per-session token
   to every form. *Recommend none.* Revisit if a browser that sends neither header must be supported.
9. **An authority proof (GitHub) as a login method** in hosted. *Recommend no in v1*: a second path in
   widens takeover, and handles are claimed through authorities, not signed in with them.
10. **Email addresses at rest:** plain in the account row, or sealed under the `secret` port's key.
    *Recommend sealed in hosted*, plain in self-host, where the operator owns the disk.
11. **Recovery delay:** 48 hours default (floor 1 hour, ceiling 30 days). *Recommend 48 hours hosted and
    24 hours self-host*, and email recovery off by default in self-host without SMTP (it is unusable
    there anyway).
12. **Session lifetimes:** 30 days absolute, 7 days idle. *Recommend these.* A dashboard used weekly
    stays signed in, and a lost browser times out.
13. **Skew window:** 60 seconds, at most 300. *Recommend 60.* It is also the length of the post-restart
    quiet period.
14. **Device key lifetime:** 180 days, renewable by `ink login`. *Recommend a limit* (a key that never
    expires is the one left on a retired laptop), shown in `ink whoami`.
15. **The operator credential as `<data>/operator-keys`** (recommended, with `ink login --data`), or an
    operator key generated and kept by the server. *Recommend the file*: the server never holds a private
    key.
16. **Delete `Owner` now, without a deprecation period.** *Recommend deleting*: nothing is released.
17. **Hardware-bound device keys** (`p256-v1`, TPM or Secure Enclave): reserve the type now, build it
    later. *Recommend reserving it*, and building when someone asks.
