# Open questions

Questions only the owner can answer, merged from `research/flow.md` section 9 (Q) and
`research/architecture.md` section 6 (O), grouped by topic for one interview round each. Each has
the default that ships if nothing changes. Answered questions move to `decisions.md` and leave a
line here.

## Answered

- O2, Q9 (third-party plugins, themes): themes are wasm components in v1 (0015) with the
  contract in 0014.
- Q3 (config file): `ink.toml` at the source root (0018).
- Licence: 0026.
- D1 (dashboard stack): `unspliced` (0032).

## 1. Themes (next)

| # | Question | Default |
|---|---|---|
| T1 | What `ink:theme-quiet` looks like: type, measure, colour, what it signs | to be designed with the owner |
| T2 | What `ink:theme-notebook` keeps from the mock | answered by 0180: no second theme, the hand-drawn look is a look of `theme-quiet`, with the marks drawn once at build time |
| T3 | Tokens every first-party theme accepts (accent, font choice, width) | accent and a serif or sans body |
| Q12 | Raw `.md` and source link in the reader footer | both on, off per site |
| Q19 | Backlinks and tag pages on by default; no graph view (needs JS) | yes |

## 2. Dashboard stack

| # | Question | Default |
|---|---|---|
| D1 | answered: 0032 | |

## 3. Publishing scope and privacy

| # | Question | Default |
|---|---|---|
| Q1, O9 | Render a public repo before its owner claims it? | previews only, 24 h, unlisted |
| Q2 | Orphan markdown (not a post, not linked) published? | no, `page.orphan` |
| Q6 | A broken page fails the whole deploy? | yes, all or nothing; `publish.partial` knob |
| O1 | Reader pages ever run author JavaScript? | never in v1 |

## 4. Vault selection

| # | Question | Default |
|---|---|---|
| Q16 | Opt-in per note, or everything reachable from home? | opt-in, new notes private |
| Q17, O8 | Where selection is recorded | frontmatter `publish: true`; folder selection as `ink.toml` `include` |
| Q20 | Allow a dashboard-held selection before it is committed? | yes, labelled as held outside the source |
| Q18 | Obsidian plugin and browser upload at launch? | both |

## 5. Handles and sites

| # | Question | Default |
|---|---|---|
| Q4 | One site per handle, or many? | one site, many sources mounted at paths |
| Q5, O3 | How long an old handle is held; brand reservation list | 12 months; short list; never reclaim a verified handle |

## 6. Deploys and previews

| # | Question | Default |
|---|---|---|
| Q7 | Previews public by URL for public sources? | yes, unlisted, 7 days |
| Q8 | Rollback pauses automatic publishing? | yes, until `resume` |
| O6 | Deploys kept for rollback | 20 |
| Q10 | Dashboard ever writes to the source (a PR with `ink.toml`)? | never |

## 7. Limits, money, retention

| # | Question | Default |
|---|---|---|
| O7 | Are the budgets a free tier or the only tier? Paid tier at all? | one tier at launch |
| Q13 | Limits shown on the landing? | exact numbers on a limits page |
| Q14 | Backup retention after deletion | overwritten within 30 days, stated |
| Q11 | Failure emails on by default? | first failure and its fix only |

## 8. Forges and self-host

| # | Question | Default |
|---|---|---|
| O4 | Self-hosted Forgejo registration: any instance admin, or approved by hand? | any admin, verified by an admin-only API call |
| O5 | Apex domains without CNAME flattening | accept the limit, offer `www` plus a redirect |
| Q15 | Self-host parity for GitHub App, statuses, previews | yes, operator registers their own App |

## Not the owner's, to settle in code

- Which Rust generator gives clean host-side WIT types: `wasmtime::component::bindgen!` (0035).
- Container cold start time: measured in the M0 workers-rs spike.
- Names of the pen and text-measuring crates: checked free on crates.io when cut.
