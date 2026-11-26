# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Rust everywhere: one workspace, one binary `ink`. Hosted on Cloudflare (Worker edge, D1, R2, Containers for builds); self-hosted as `ink serve`. Plugins are WIT-typed; builder-side plugins and themes are wasm components. Spec: `docs/architecture.md`; reasons: `docs/decisions.md`.

## Users

Two primary groups, both people who already write somewhere and do not want to build or run a site:

- **Developers** who keep markdown in a git repository (a notes repo, a docs folder, a repo that is only a README) and live in a terminal and a forge.
- **Note-takers** who keep a folder of notes, typically an Obsidian vault, with wikilinks, embeds and attachments. Not all of them use a terminal or git.

Readers of the published blogs are a third audience: anyone with a browser, including without JavaScript.

## Product Purpose

uses.ink turns content a person already keeps into a blog at `<handle>.uses.ink`, with no configuration required. Writing stays where it already happens; publishing is pointing uses.ink at it once. Success: a live blog from existing content in about a minute, and every later change to the content shows up on the blog without the person thinking about uses.ink.

## Positioning

**Zero config, no lock-in.** uses.ink publishes from the person's own source and never holds anything they cannot get back from that source. Leaving costs nothing: the content was never moved. Neighbours either require setup and a build (GitHub Pages, Quartz), or keep the content in their own store (bearblog, Hashnode, Ghost, Obsidian Publish).

Historic tagline: "You got a repo? You got a blog!" Whether it survives the rebrand is open.

## Operating Context

- Content comes from **sources**, each a plugin: any git remote, a folder pushed by a `ink` CLI, ATproto repo records, forge integrations. Git is one source, not the protocol. Sources declare capabilities (snapshot, history, watch, private); a missing capability is shown, never guessed.
- **Authorities** prove a person controls a source (GitHub, Codeberg/Forgejo, GitLab, DNS, ATproto DID, email). A subdomain belongs to a uses.ink handle that links one or more identities.
- Everything uses.ink infers about the content (home, posts, titles, dates, drafts, nav, wikilinks) is a recorded, visible decision the person can override with an optional config file.
- Custom domains are supported. Publishing to ATproto / standard.site comes later.

## Capabilities and Constraints

- Open source and self-hostable; the hosted service at uses.ink is the main path.
- Nothing hardcoded: every source, authority and theme goes through the same plugin contract as first-party ones.
- Nothing is silent: defaults, fallbacks, retries and inferences surface as typed events; errors are diagnostics with a stable code, cause and fix.
- Undecided: monetisation, tagline, the first-party theme designs, the dashboard stack. Tracked in `docs/open.md`.

## Brand Commitments

- Name and domain: **uses.ink**. GitHub org `uses-ink`.
- Full rebrand: identity, logo, palette and typography are being redone. The previous uses.ink look (`assets/light.png`, `assets/dark.png` in the old repo) is evidence only, not a reference to keep.
- Voice and look: **happy, warm and a little cute**, drawn by hand (MS Paint pencil, xkcd stick figures, doodles). Plain words, honest facts, never smug or nonchalant. No marketing puffery. Owner decision 2026-11-26.

## Evidence on Hand

- The previous uses.ink (branches `legacy` and `astro` of `uses-ink/uses.ink`, 7 stars, old demo at `cestef.uses.ink`, now offline).
- No users, testimonials, metrics or customers for the rewrite. Do not invent any.

## Product Principles

1. **The source is the truth.** uses.ink derives; it never becomes the place content lives.
2. **Zero config, full override.** Every inference works without a file and can be pinned by one.
3. **Show what happened.** Any decision, fallback or failure is visible with its reason and its fix.
4. **One contract.** First-party features use the same plugin contract as anything else.
5. **Readers first on the blog.** A published page is fast, readable and works without JavaScript.

## Accessibility & Inclusion

WCAG 2.2 AA for the dashboard and every first-party theme. State is never conveyed by colour alone. Reduced motion respected. Published blogs readable without JavaScript.
