# Decisions

Each decision is one file, `NNNN-slug.md`, with front matter. A number never changes. A superseded
decision stays, with a pointer to its replacement.

Numbers 0058 and 0060 are reserved: they were review-fix logs, now in [`../history/review-fixes.md`](../history/review-fixes.md).

| Number | Title | Status | Date | Area |
|---|---|---|---|---|
| 0001 | [Own engine, written from scratch](0001-own-engine-written-from-scratch.md) | accepted | 2026-11-26 | product |
| 0002 | [Nothing hardcoded: four plugin slots](0002-nothing-hardcoded-four-plugin-slots.md) | accepted | 2026-11-26 | plugins |
| 0003 | [Rust everywhere, one workspace, one binary](0003-rust-everywhere-one-workspace-one-binary.md) | accepted | 2026-11-26 | repo |
| 0004 | [Hosted runtime: Worker edge, Container builds](0004-hosted-runtime-worker-edge-container-builds.md) | accepted | 2026-11-26 | hosting |
| 0005 | [Self-host is `ink serve`, one process](0005-self-host-is-ink-serve-one-process.md) | accepted | 2026-11-26 | hosting |
| 0006 | [The engine is pure](0006-the-engine-is-pure.md) | accepted | 2026-11-26 | engine |
| 0007 | [WIT is the plugin boundary; builder-side plugins are components in v1](0007-wit-is-the-plugin-boundary-builder-side-plugins.md) | accepted | 2026-11-26 | plugins |
| 0008 | [`git.fetch` is a host capability](0008-git-fetch-is-a-host-capability.md) | accepted | 2026-11-26 | plugins |
| 0009 | [Content addressing with BLAKE3, scoped per tenant](0009-content-addressing-with-blake3-scoped-per-tenant.md) | accepted | 2026-11-26 | data |
| 0010 | [Static builds; a deploy is a hashed manifest](0010-static-builds-a-deploy-is-a-hashed-manifest.md) | accepted | 2026-11-26 | engine |
| 0011 | [Facts carry provenance and completeness](0011-facts-carry-provenance-and-completeness.md) | accepted | 2026-11-26 | engine |
| 0012 | [Abilities are declared, then observed](0012-abilities-are-declared-then-observed.md) | accepted | 2026-11-26 | plugins |
| 0013 | [Authorities are separate from sources](0013-authorities-are-separate-from-sources.md) | accepted | 2026-11-26 | control |
| 0014 | [Theme contract: the engine owns structure, themes render pages](0014-theme-contract-the-engine-owns-structure-themes.md) | accepted | 2026-11-26 | theme |
| 0015 | [Themes are wasm components in v1](0015-themes-are-wasm-components-in-v1.md) | accepted | 2026-11-26 | theme |
| 0016 | [A quiet default theme; the hand-drawn theme is opt-in](0016-a-quiet-default-theme-the-hand-drawn-theme-is-opt.md) | accepted | 2026-11-26 | theme |
| 0017 | [Private by default for vaults and private sources](0017-private-by-default-for-vaults-and-private-sources.md) | accepted | 2026-11-26 | control |
| 0018 | [One config file, `ink.toml`; the decision code is the pin path](0018-one-config-file-ink-toml-the-decision-code-is-the.md) | accepted | 2026-11-26 | config |
| 0019 | [Custom domains through Cloudflare for SaaS, bound by our TXT proof](0019-custom-domains-through-cloudflare-for-saas-bound.md) | accepted | 2026-11-26 | hosting |
| 0020 | [Isolation: PSL, `app.uses.ink`, no JS on reader pages](0020-isolation-psl-app-uses-ink-no-js-on-reader-pages.md) | accepted | 2026-11-26 | security |
| 0021 | [Budgets are typed refusals, published](0021-budgets-are-typed-refusals-published.md) | accepted | 2026-11-26 | control |
| 0022 | [Deletion removes bytes before rows](0022-deletion-removes-bytes-before-rows.md) | accepted | 2026-11-26 | control |
| 0023 | [The build log is one typed event stream](0023-the-build-log-is-one-typed-event-stream.md) | accepted | 2026-11-26 | engine |
| 0024 | [Names](0024-names.md) | accepted | 2026-11-26 | product |
| 0025 | [One pen](0025-one-pen.md) | accepted | 2026-11-26 | brand |
| 0026 | [Licence](0026-licence.md) | accepted | 2026-11-26 | product |
| 0027 | [Markdown through comrak; raw HTML through our own allowlist](0027-markdown-through-comrak-raw-html-through-our-own.md) | accepted | 2026-11-26 | format |
| 0028 | [Dates with jiff](0028-dates-with-jiff.md) | accepted | 2026-11-26 | engine |
| 0029 | [Repo tasks are `cargo xtask`](0029-repo-tasks-are-cargo-xtask.md) | accepted | 2026-11-26 | repo |
| 0030 | [The `Db` port is shaped like D1](0030-the-db-port-is-shaped-like-d1.md) | accepted | 2026-11-26 | data |
| 0031 | [Credentials sealed with XChaCha20-Poly1305](0031-credentials-sealed-with-xchacha20-poly1305.md) | accepted | 2026-11-26 | security |
| 0032 | [HTML, SVG and CSS through `unspliced`; no maud, no Datastar](0032-html-svg-and-css-through-unspliced-no-maud-no.md) | accepted | 2026-11-26 | web |
| 0033 | [The pen crate is `freehand`; brand drawings are data](0033-the-pen-crate-is-freehand-brand-drawings-are-data.md) | accepted | 2026-11-26 | brand |
| 0034 | [The pen in the browser is plain wasm and one small loader](0034-the-pen-in-the-browser-is-plain-wasm-and-one.md) | accepted | 2026-11-26 | brand |
| 0035 | [Plugins run on wasmtime as written; guests target `wasm32-unknown-unknown`](0035-plugins-run-on-wasmtime-as-written-guests-target.md) | accepted | 2026-11-26 | plugins |
| 0036 | [Git history through gix: full clone for history, one walk for dates](0036-git-history-through-gix-full-clone-for-history.md) | accepted | 2026-11-26 | plugins |
| 0037 | [Container egress from Rust through `Reflect`; no JS shim](0037-container-egress-from-rust-through-reflect-no-js.md) | accepted | 2026-11-26 | hosting |
| 0038 | [Path data in tenths, as a `T` chain; lengths by `sqrt`](0038-path-data-in-tenths-as-a-t-chain-lengths-by-sqrt.md) | accepted | 2026-11-26 | brand |
| 0039 | [Highlighting is a host capability, through arborium](0039-highlighting-is-a-host-capability-through-arborium.md) | accepted | 2026-11-26 | engine |
| 0040 | [Images are prepared by the host, deterministically, and served as hashed copies](0040-images-are-prepared-by-the-host-deterministically.md) | accepted | 2026-11-26 | engine |
| 0041 | [Components as built: values across, one shared mirror](0041-components-as-built-values-across-one-shared.md) | accepted | 2026-11-26 | plugins |
| 0042 | [Decisions carry one evidence; changes carry a cause; inference rules are versioned](0042-decisions-carry-one-evidence-changes-carry-a.md) | accepted | 2026-11-26 | engine |
| 0043 | [Obsidian is a dialect of the markdown format; the engine resolves and embeds](0043-obsidian-is-a-dialect-of-the-markdown-format-the.md) | accepted | 2026-11-26 | format |
| 0044 | [Settings are split three ways: the site's, the operator's, and the fixed](0044-settings-are-split-three-ways-the-site-s-the.md) | accepted | 2026-11-26 | config |
| 0045 | [Releases are `dist` on version tags; the image is built from source; a rule keeps them in step](0045-releases-are-dist-on-version-tags-the-image-is.md) | proposed | 2026-11-26 | release |
| 0046 | [A source plugin sees no bytes; the host fetches, stores and measures](0046-a-source-plugin-sees-no-bytes-the-host-fetches.md) | accepted | 2026-11-27 | plugins |
| 0047 | [Builds and tests run on a farm of slots, placed by the `farm` binary](0047-builds-and-tests-run-on-a-farm-of-slots-placed-by.md) | accepted | 2026-11-26 | farm |
| 0048 | [A page is a render unit; the theme says what it reads](0048-a-page-is-a-render-unit-the-theme-says-what-it.md) | accepted | 2026-11-26 | engine |
| 0049 | [Statements and schema are typed values; one renderer writes the text](0049-statements-and-schema-are-typed-values-one.md) | accepted | 2026-11-26 | data |
| 0051 | [The edge answers from two ports and refuses what it cannot name; the adapter owns the times](0051-the-edge-answers-from-two-ports-and-refuses-what.md) | built | 2026-11-27 | edge |
| 0052 | [The API is routed by host and reaches the control plane over a channel; intake takes a built folder; the watch keeps its state in memory](0052-the-api-is-routed-by-host-and-reaches-the-control.md) | built | 2026-11-27 | edge |
| 0053 | [The dashboard is server-rendered with `unspliced`; no Datastar, no script; the log is a streamed page](0053-the-dashboard-is-server-rendered-with-unspliced.md) | accepted | 2026-11-26 | web |
| 0054 | [`ink` carries its first-party components; xtask lists them, the build script embeds them](0054-ink-carries-its-first-party-components-xtask.md) | built | 2026-11-27 | release |
| 0055 | [A source plugin fetches only its want; the host checks what it returns](0055-a-source-plugin-fetches-only-its-want-the-host.md) | accepted | 2026-11-26 | plugins |
| 0058 | Review fixes of the control plane and the edge: what a failure now does | reserved |  |  |
| 0060 | Review fixes: keys, panics, escapes, atomic writes, held nones | reserved |  |  |
| 0061 | [The `ink` CLI has one output grammar, one renderer and a grouped command tree](0061-the-ink-cli-has-one-output-grammar-one-renderer.md) | accepted | 2026-11-26 | cli |
| 0065 | [The process logs through one crate: typed fields, a span per layer of work, a channel of its own](0065-the-process-logs-through-one-crate-typed-fields-a.md) | accepted | 2026-11-26 | logging |
| 0066 | [Sign-in for self-host: typed credentials, digests at rest, a mail port, links that look before they spend](0066-sign-in-for-self-host-typed-credentials-digests.md) | built | 2026-11-27 | control |
| 0067 | [`ink serve --data` runs the loop: a sites lookup over the control plane, the watch and the worker as supervised threads, git through the source plugin](0067-ink-serve-data-runs-the-loop-a-sites-lookup-over.md) | accepted | 2026-11-26 | edge |
| 0068 | [Authentication has no passwords: passkeys for people, signed requests for machines](0068-authentication-has-no-passwords-passkeys-for.md) | accepted | 2026-11-27 | security |
| 0072 | [Git over https: one TLS stack, rustls over ring, and one licence exception](0072-git-over-https-one-tls-stack-rustls-over-ring-and.md) | accepted | 2026-11-27 | security |
| 0074 | [The repository is reorganised under `crates/` by layer; monoliths are split into directories first](0074-the-repository-is-reorganised-under-crates-by.md) | accepted | 2026-11-27 | repo |
| 0075 | [The farm reuses a pass: results are cached by what the job read, and only passes](0075-the-farm-reuses-a-pass-results-are-cached-by-what.md) | accepted | 2026-11-26 | farm |
| 0076 | [The farm places a job where it will finish first, by what that job took before](0076-the-farm-places-a-job-where-it-will-finish-first.md) | accepted | 2026-11-26 | farm |
| 0077 | [`ink push` sends a tree and only the blobs the site lacks; the folder verbs know their site by `.uses/site`](0077-ink-push-sends-a-tree-and-only-the-blobs-the-site.md) | accepted | 2026-11-27 | cli |
| 0078 | [The farm tests only the crates a change affects, when asked](0078-the-farm-tests-only-the-crates-a-change-affects.md) | accepted | 2026-11-26 | farm |
| 0079 | [Hosted is adapters of the existing ports: queue rows on D1, blobs through the egress gateway, nonces in a Durable Object](0079-hosted-is-adapters-of-the-existing-ports-queue.md) | proposed | 2026-11-28 | hosting |
| 0080 | [The farm keeps heavy jobs off a handicapped host and tests only on a quiet one](0080-the-farm-keeps-heavy-jobs-off-a-handicapped-host.md) | accepted | 2026-11-28 | farm |
| 0081 | [The farm removes the copies it made, seeds from the tree used last, and stops a gate that has failed](0081-the-farm-cleans-up-after-itself-and-keeps-new-work-warm.md) | accepted | 2026-11-28 | farm |
| 0081 | [Sessions are listed and ended by id on the Door, and the dashboard ends one with a form post](0081-sessions-are-listed-and-ended-by-id-on-the-door.md) | built | 2026-11-28 | security |
| 0082 | [Every cookie route that changes state checks its origin, and starting a passkey sign-in is limited per source](0082-every-cookie-route-that-changes-state-checks-its.md) | built | 2026-11-28 | security |
| 0083 | [Step-up is a passkey sign-in within a window; adding or removing a passkey and approving a key need it](0083-step-up-is-a-passkey-sign-in-within-a-window.md) | built | 2026-11-28 | security |
| 0084 | [Recovery codes are minted once, kept as digests, and each is spent once to register a new passkey](0084-recovery-codes-are-minted-once-and-spent-once-on-a-new-passkey.md) | built | 2026-11-28 | security |
| 0085 | [The client address behind a proxy is read from X-Forwarded-For by trusted hop count, off by default](0085-the-client-address-behind-a-proxy-is-read-from-x-forwarded.md) | built | 2026-11-28 | security |
