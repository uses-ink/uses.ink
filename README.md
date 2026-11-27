# uses.ink

uses.ink turns a folder of markdown, in a git repository or an Obsidian vault, into a blog with no
configuration. The `ink` binary reads the folder, builds a static site and serves it. Pre-release:
`docs/roadmap.md` says what is built.

## Build and run

The toolchain is pinned by `rust-toolchain.toml`. Build the plugin components, then the binary:

```
cargo --locked xtask component
cargo build --locked --profile dist --package uses-ink-cli --bin ink
cd my-notes                        # a folder of markdown
/path/to/uses.ink/target/dist/ink build   # writes .ink/out
/path/to/uses.ink/target/dist/ink serve   # serves .ink/out on 127.0.0.1:8080
```

To self-host in a container, `docker build -t uses-ink .`; the run line is in the header of the
`Dockerfile`.

## Contribute

- Version control is jj, not git.
- `cargo xtask check` runs the whole gate (fmt, clippy, tests, doc tests, wasm, site fixtures) and
  names any tool it needs. Heavy builds and checks go through `farm`, the build farm client: see
  [docs/workflow.md](docs/workflow.md).
- Naming and code rules are in [docs/conventions.md](docs/conventions.md). A decision goes in
  [docs/decisions.md](docs/decisions.md).

## Where things are

| Path | Holds |
|---|---|
| `model/`, `engine/`, `config/`, `diag/` | the data model and the pure engine |
| `plugin/`, `wit/` | first-party plugins and the plugin contract |
| `host/`, `edge/`, `control/`, `cli/` | the builder's capabilities, serving, the control plane, the `ink` binary |
| `docs/` | the specs; start with `docs/architecture.md` |
| `tools/farm/`, `xtask/` | the build farm client and the repo tasks |
| `spikes/` | experiments outside the workspace |

## Read next

- What it is: [PRODUCT.md](PRODUCT.md)
- How it works: [docs/architecture.md](docs/architecture.md)
- Rules: [docs/conventions.md](docs/conventions.md) and [docs/code.md](docs/code.md)
- Why: [docs/decisions.md](docs/decisions.md)
- Plans: [docs/roadmap.md](docs/roadmap.md), and [docs/layout.md](docs/layout.md) for the repository layout

## Licence

AGPL-3.0-or-later. The plugin SDK, the WIT packages and the product-independent crates are
MIT OR Apache-2.0. See `docs/conventions.md`.
