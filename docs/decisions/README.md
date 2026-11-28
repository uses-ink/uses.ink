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
| 0082 | [Sessions are listed and ended by id on the Door, and the dashboard ends one with a form post](0082-sessions-are-listed-and-ended-by-id-on-the-door.md) | built | 2026-11-28 | security |
| 0083 | [Every cookie route that changes state checks its origin, and starting a passkey sign-in is limited per source](0083-every-cookie-route-that-changes-state-checks-its.md) | built | 2026-11-28 | security |
| 0084 | [Step-up is a passkey sign-in within a window; adding or removing a passkey and approving a key need it](0084-step-up-is-a-passkey-sign-in-within-a-window.md) | built | 2026-11-28 | security |
| 0085 | [Recovery codes are minted once, kept as digests, and each is spent once to register a new passkey](0085-recovery-codes-are-minted-once-and-spent-once-on-a-new-passkey.md) | built | 2026-11-28 | security |
| 0086 | [The client address behind a proxy is read from X-Forwarded-For by trusted hop count, off by default](0086-the-client-address-behind-a-proxy-is-read-from-x-forwarded.md) | built | 2026-11-28 | security |
| 0087 | [A handle a site leaves is held for twelve months, a verified owner's handle for good, and the brand list is a data file](0087-a-handle-a-site-leaves-is-held-for-twelve-months-a.md) | built | 2026-11-28 | control |
| 0088 | [Budgets are one typed Limits value, counted from the rows and the blob list, and a deploy over storage is refused whole](0088-budgets-are-one-typed-limits-value-counted-from-the-rows.md) | built | 2026-11-28 | control |
| 0089 | [A reported site is suspended first, as a row of its own, and anyone may report with no account](0089-a-reported-site-is-suspended-first-as-a-row-of-its-own.md) | built | 2026-11-28 | control |
| 0090 | [A removal runs in bounded steps and keeps its totals in a row, so a resume gives a whole receipt](0090-a-removal-runs-in-bounded-steps-and-keeps-its-totals-in-a-row.md) | built | 2026-11-28 | control |
| 0091 | [A branch preview is a labelled manifest with an end and no deploy, served at label--handle](0091-a-branch-preview-is-a-labelled-manifest-with-an-end-and-no-deploy.md) | built | 2026-11-28 | control |
| 0092 | [The dashboard is served on the API host by ink serve, with a derived origin and a per-request site lookup](0092-the-dashboard-is-served-on-the-api-host-by-ink-serve.md) | built | 2026-11-28 | control |
| 0093 | [A custom domain is a row proved by a TXT token that binds the name to a site; claims serve nothing](0093-a-custom-domain-is-a-row-proved-by-a-txt-token-that-binds.md) | built | 2026-11-28 | control |
| 0094 | [The email authority is the magic link spent as a proof of an address, under its own digest key](0094-the-email-authority-is-the-magic-link-spent-as-a-proof-of-an-address.md) | built | 2026-11-28 | control |
| 0095 | [An org is an account with members whose roles are code; the last owner is held by one statement](0095-an-org-is-an-account-with-members-whose-roles-are-code.md) | built | 2026-11-28 | control |
| 0096 | [The hosted spikes answer yes with three changes: registration needs a clock patch, D1 carries integers to 2^53 only, and R2 needs a known length](0096-the-hosted-spikes-answer-yes-with-two-changes-registration-needs-a-clock-and-r2-needs-a-known-length.md) | built | 2026-11-28 | hosting |
| 0097 | [The db conformance suite is a list of async cases, and the D1 adapter passes all of them in workerd](0097-the-db-conformance-suite-is-a-case-list-and-d1-passes-it-in-workerd.md) | built | 2026-11-28 | hosting |
| 0098 | [D1 migrations are the typed migrations rendered to files and applied by wrangler, and the schema is the same as SQLite's](0098-d1-migrations-are-the-typed-migrations-rendered-to-files-and-applied-by-wrangler.md) | built | 2026-11-28 | hosting |
| 0100 | [`ink abuse` is the operator's verbs over the API's operator routes, signed by the operator key](0100-ink-abuse-is-the-operators-verbs-signed-by-the-operator-key.md) | built | 2026-11-28 | cli |
| 0101 | [Custom domains are proved by asking a real nameserver through a small typed DNS client, chosen by a flag](0101-custom-domains-are-asked-of-a-real-nameserver-by-a-small-typed-client.md) | built | 2026-11-28 | edge |
| 0102 | [The watch builds the branches of a public git site as previews, through a pin table beside the job](0102-the-watch-builds-the-branches-of-a-public-git-site-as-previews-through-a-pin-table.md) | built | 2026-11-28 | control |
| 0103 | [A call as an organization rewrites the principal once, and the account, the site check and the key reach change together](0103-a-call-as-an-organization-rewrites-the-principal-once-and-the-account.md) | built | 2026-11-28 | control |
| 0104 | [An organization has a typed one-line name and a roster that decides what a member may give a key](0104-an-organization-has-a-typed-one-line-name-and-a-roster-that.md) | built | 2026-11-28 | control |
| 0105 | [The Account screen changes organizations with four plain forms and asks for the step-up before it changes members](0105-the-account-screen-changes-organizations-with-four-plain-forms-and-asks-for.md) | built | 2026-11-28 | control |
| 0110 | [The GitHub authority proves a repo by the App's installation and makes installation tokens one call at a time](0110-the-github-authority-proves-a-repo-by-the-apps-installation-and.md) | built | 2026-11-28 | control |
| 0111 | [The Forgejo authority proves a repo by a committed challenge, lists the instances it calls, and ships Codeberg](0111-the-forgejo-authority-proves-a-repo-by-a-committed-challenge-and.md) | built | 2026-11-28 | control |
| 0112 | [The GitLab authority checks the secret token header over digests and proves a repo by a committed challenge](0112-the-gitlab-authority-checks-the-secret-token-header-in-constant.md) | built | 2026-11-28 | control |
| 0113 | [Forge webhooks reach the watch through one route that is not wired yet, and the proofs bind in the control plane](0113-forge-webhooks-reach-the-watch-through-one-route-that-is-not.md) | accepted | 2026-11-28 | control |
| 0120 | [The docs site is `docs/` built by `ink` with theme-quiet, and the sites stage builds it twice and refuses any diagnostic](0120-the-docs-site-is-docs-built-by-ink-with-theme-quiet-checked-twice.md) | built | 2026-11-28 | release |
| 0130 | [The API crates build for wasm32 because sockets and files are features of the crates that use them](0130-the-api-crates-build-for-wasm32-because-sockets-files-and-the-timer-are-features.md) | built | 2026-11-28 | hosting |
| 0131 | [The R2 adapter hashes while it takes bytes, puts whole up to one part and cuts a multipart upload past it](0131-the-r2-adapter-hashes-while-it-takes-bytes-and-cuts-parts-past-eight-mebibytes.md) | built | 2026-11-28 | hosting |
| 0132 | [The hosted queue is the rows on D1, and a Queues message only wakes a consumer that then reads the rows](0132-the-hosted-queue-is-the-rows-on-d1-and-a-queues-message-only-wakes-a-consumer.md) | built | 2026-11-28 | hosting |
| 0133 | [The sealing is one small crate both secret adapters use, and the Worker adapter reads its key from a secret binding with a previous key for rotation](0133-the-sealing-is-one-small-crate-and-the-worker-adapter-reads-its-key-from-a-secret-binding.md) | built | 2026-11-28 | hosting |
| 0134 | [The Cloudflare mail adapter calls its provider through a typed Transport, and the mail port gets a conformance suite](0134-the-mail-adapter-calls-its-provider-through-a-typed-transport-and-the-port-gets-a-suite.md) | built | 2026-11-28 | hosting |
| 0135 | [The nonce store is a trait with a memory store and a Durable Object store behind one suite, and the object's ledger refuses requests older than itself](0135-the-nonce-store-is-a-trait-with-a-memory-store-and-a-durable-object-store-behind-one-suite.md) | built | 2026-11-28 | hosting |
| 0140 | [A `[folder]` table in `ink.toml` rules the kind, weight and order of the pages in a folder](0140-a-folder-table-in-ink-toml-rules-the-kind-and-order-of-the-pages-in-it.md) | built | 2026-11-28 | config |
| 0141 | [The theme gets the section tree, the neighbours and the outline, and `theme-quiet` draws them with `navigation = "docs"`](0141-the-theme-gets-the-section-tree-the-neighbours-and-the-outline-and-draws.md) | built | 2026-11-28 | theme |
| 0142 | [A theme declares tokens, and the uses.ink docs look comes from `token.toml`](0142-a-theme-declares-tokens-and-the-uses-ink-docs-look-comes-from-token-toml.md) | built | 2026-11-28 | theme |
