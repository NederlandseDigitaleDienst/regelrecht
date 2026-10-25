# syntax=docker/dockerfile:1
# API-only image: the harvester-admin binary is a standalone harvest API. Its
# dashboard UI now lives in the editor (frontend/src/harvester), which reaches
# this API through the editor-api /api/harvest-admin/* proxy — so there is no
# frontend build stage and no static assets here.

# Stage 1: Chef base
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:a41f7740f8b45d45795624eec13a8b42263cc700f19f7e4e86e04d3dda08a479 AS chef
RUN apk add --no-cache musl-dev curl xz
# cargo-chef als officiële prebuilt binary van de releasepagina van het project,
# in plaats van vanaf broncode te compileren. De sha256's zijn de gepubliceerde
# .sha256-bestanden van diezelfde release; zonder die pin zou een gewijzigd of
# afgebroken artefact ongemerkt de build in glijden.
ARG TARGETARCH
ARG CARGO_CHEF_VERSION=0.1.77
ARG CARGO_CHEF_SHA256_AMD64=a3733ab416c3ffddd37914cd13919ca05fee1a1cf654f3016dcfe7f399d89cd1
ARG CARGO_CHEF_SHA256_ARM64=3bff2e7ff979db837c7e820dfd8d51d2d97b9c94283506c6e596ae4887ef178b
RUN set -eu; \
    case "${TARGETARCH:-}" in \
      amd64) arch=x86_64; sha="${CARGO_CHEF_SHA256_AMD64}" ;; \
      arm64) arch=aarch64; sha="${CARGO_CHEF_SHA256_ARM64}" ;; \
      *) echo "geen gepinde cargo-chef-binary voor TARGETARCH='${TARGETARCH:-}'" >&2; exit 1 ;; \
    esac; \
    dir="cargo-chef-${arch}-unknown-linux-musl"; \
    curl -fsSLo "/tmp/${dir}.tar.xz" \
      "https://github.com/LukeMathWalker/cargo-chef/releases/download/v${CARGO_CHEF_VERSION}/${dir}.tar.xz"; \
    echo "${sha}  /tmp/${dir}.tar.xz" | sha256sum -c -; \
    tar -xJf "/tmp/${dir}.tar.xz" -C /tmp; \
    install -m0755 "/tmp/${dir}/cargo-chef" /usr/local/bin/cargo-chef; \
    rm -rf "/tmp/${dir}.tar.xz" "/tmp/${dir}"; \
    cargo chef --version
WORKDIR /build

# Stage 2: Plan dependencies
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
# Trim workspace members to those actually built by this Dockerfile so chef
# does not try to resolve crates whose source we do not COPY. The `grep`
# assertion ensures the multi-line members format is intact; otherwise the
# `sed` patterns would silently no-op.
# When adding a new workspace member that this image does NOT build, add it
# to this exclusion list as well; otherwise chef will fail to find its source.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"arch-extract"/d; /"cel"/d; /"editor-api"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d; /"poc-wetsgeschiedenis"/d' Cargo.toml
COPY packages/admin/ admin/
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build Rust binary (deps cached separately from source)
FROM chef AS deps
COPY packages/Cargo.lock ./
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Up to here only the dependencies, as a stage of its own: warm-buildcache.yml
# builds just this stage on every push to main, so the cook layer is in the
# cache before the next PR needs it, whether or not main deploys.
FROM deps AS builder
# `cargo chef cook` rewrites the workspace Cargo.toml without
# `[workspace.lints]`, which breaks members that declare `[lints] workspace
# = true`. Re-COPY the real workspace manifest before the final build.
COPY --from=planner /build/Cargo.toml ./Cargo.toml

COPY packages/admin/ admin/
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
# regelrecht-engine (validate feature, pulled in by pipeline's law_convert)
# embeds the JSON schemas via `include_str!("../../../schema/...")`, relative
# to engine/src/. The crate lives at /build/engine here (not
# /build/packages/engine), so that literal resolves to /schema/... — same
# layout trick as frontend/Dockerfile.
COPY schema/ /schema/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-admin

# Stage 4: Runtime
FROM alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-admin /usr/local/bin/

WORKDIR /app
USER app
EXPOSE 8000
CMD ["regelrecht-admin"]
