# Stand-in for the ink builder image: a static musl binary on a tiny base.
FROM docker.io/library/rust:1-alpine AS build
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM docker.io/library/alpine:3
COPY --from=build /src/target/release/emit /emit
EXPOSE 8080
ENTRYPOINT ["/emit", "serve"]
