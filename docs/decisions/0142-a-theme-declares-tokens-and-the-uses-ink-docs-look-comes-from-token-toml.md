---
number: "0142"
title: "A theme declares tokens, and the uses.ink docs look comes from `token.toml`"
status: built
date: 2026-11-28
area: theme
supersedes: []
---

## 0142 A theme declares tokens, and the uses.ink docs look comes from `token.toml`

Built (2026-11-28), the open question T3.

- **Tokens are words.** `[theme]` already kept `name = value` pairs and ignored them. `theme-quiet` now
  declares two: `look` (`quiet`, the default, or `uses-ink`) and `navigation` (`header` or `docs`, 0141).
  A token the theme does not declare is `theme.token.unknown` at the name, with the tokens it has; a word
  it does not take is `theme.token.invalid` at the value, with the words it takes. Either stops the
  build, in the CLI and in the hosted builder alike, with no deploy.
- **The boundary.** The theme world's `assets` takes the tokens and the renderer's constructor is now
  `(base, tokens, site)`, with a `token {name, value}` record in the `site` interface. The host passes
  the pairs as written and the guest refuses them with the same diagnostics. `Runtime::theme` takes a
  `Hosting` (base and tokens). This is a change to the world, which is why it is written here; the
  package version is as 0141 says.
- **Identity.** The theme's identity hashes its code, its files and each token's name and word, and so
  does the component's, so a changed token invalidates the cache; the sites stage still compares the
  native and the component build byte for byte.
- **The look.** `crates/core/token/token.toml` gained `[look.uses-ink]` with a light and a dark set of
  six roles (ink, paper, muted, line, tint, accent), two faces and five sizes. `cargo xtask token` makes
  `token::Look`, whose `css()` is the stylesheet: `:root` custom properties, with the dark set under
  `prefers-color-scheme: dark`. With `look = "uses-ink"` the theme links it as `/look.css` after
  `style.css`; `style.css` reads the same properties, so the look replaces the defaults.
- **Palette.** PRODUCT.md says the identity, palette and typography are being redone, so this set is
  restrained and meant to be replaced: warm paper and near-black ink, one accent (a red), the `reader`
  face for text and `mono` for code, and no other colour. Every text role is at least 5.6:1 on paper
  and on tint in both schemes; `crates/core/token/tests/contrast.rs` computes WCAG contrast from the
  generated colours and fails below 4.5:1. Colour never carries state alone: the current page is also
  bold and carries `aria-current`.
- **The docs site** sets `look = "uses-ink"` and `navigation = "docs"`.

Left: tokens for sizes and spacing a site can set by value, and a second theme that declares tokens.
