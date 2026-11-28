# syntax=docker/dockerfile:1
#
# The self-host image: `ink serve`, one static binary, no shell needed to run it.
#
#   docker build -t uses-ink .
#   docker run --read-only --tmpfs /tmp -v uses-ink:/data -p 8080:8080 uses-ink
#
# The image builds no site. `ink serve` reads `.ink/out` of its working directory unless told
# another folder, and refuses to start when it is missing, so the image makes it, empty: an empty
# folder serves no site, and `--data` turns on the sites of the control plane (docs/selfhost.md).
#
# Release builds are the dist ones (dist-workspace.toml); this builds the same `dist`
# profile from source, on musl, so the binary has no libc to match. `ink` carries its components
# (decision 0054), so they are built first, in the same step, by `cargo xtask component`.
# docs/release.md says how the image is published.

# The toolchain tag equals `channel` in rust-toolchain.toml; `cargo xtask layout` checks it.
FROM rust:1.100.0-alpine3.24@sha256:8d0e82abfdf9a38071e07cabc5afba4ddbe077631a80e4e742a760c38beadc33 AS build

# A C compiler for the tree-sitter grammars; nothing else links C.
RUN apk add --no-cache build-base

# The image's own toolchain, so rustup does not fetch the targets rust-toolchain.toml names; the
# one target the components need is added below.
ENV RUSTUP_TOOLCHAIN=1.100.0 \
    CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=never \
    RUSTFLAGS="--remap-path-prefix=/src=/build --remap-path-prefix=/usr/local/cargo=/cargo"

RUN rustup target add --toolchain 1.100.0 wasm32-unknown-unknown

WORKDIR /src
COPY . .

# The registry and build state live in cache mounts: a rebuild after a source change compiles
# our crates only. The components are built into target/plugin first, which the build script
# of `ink` embeds, and the binary is copied out of the mount in the same step.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo --locked xtask component \
 && cargo build --locked --profile dist --package uses-ink-cli --bin ink \
 && install -D target/dist/ink /out/ink

FROM busybox:1.37.0-musl@sha256:f6a95d611e0a8780fabdb2aec43e31275b2720dfefa3e41b16835bfc42454478

# busybox is here for its `wget`, which the healthcheck uses; `ink` itself needs nothing.
COPY --from=build /out/ink /usr/local/bin/ink

# Everything the process writes is under /data, the only path that has to be writable: the
# root filesystem can be read-only, and /tmp a tmpfs.
RUN mkdir -p /data/.ink/out && chown -R 65532:65532 /data
USER 65532:65532
ENV HOME=/data XDG_CACHE_HOME=/data/cache
WORKDIR /data
VOLUME /data

# `ink serve` listens on 127.0.0.1 unless told otherwise, which a published port cannot reach, so
# CMD names every interface (`cargo xtask layout` checks it). The port is the one the healthcheck
# and the example above use.
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null "http://127.0.0.1:8080/healthz" || exit 1

# The command prints every default it chose as a notice (decision 0005), so
# `docker logs` is where a first run's settings are read.
ENTRYPOINT ["ink"]
CMD ["serve", "--addr", "0.0.0.0:8080"]
