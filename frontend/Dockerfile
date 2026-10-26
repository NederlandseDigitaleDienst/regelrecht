# syntax=docker/dockerfile:1
# Stage 1a: Build WASM module using cargo + wasm-bindgen-cli (no wasm-pack needed)
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:2ea3db105d38fdfa4e31f366674287fcaa828087e2fe3973befdc537f2d443b1 AS wasm-builder
RUN apk add --no-cache musl-dev curl
RUN rustup target add wasm32-unknown-unknown
# wasm-bindgen-cli als officiële prebuilt binary van de releasepagina van het
# project, in plaats van vanaf broncode te compileren. De sha256's zijn de
# gepubliceerde .sha256sum-bestanden van diezelfde release; zonder die pin zou
# een gewijzigd of afgebroken artefact ongemerkt de build in glijden.
ARG TARGETARCH
ARG WASM_BINDGEN_VERSION=0.2.114
ARG WASM_BINDGEN_SHA256_AMD64=ce21be002bdbc22ede2a5cf250cb801d2106956fc3c529c75d9452729db3490c
ARG WASM_BINDGEN_SHA256_ARM64=39103878ad3c0016f2d04b7b4a6206c974240286852abac5736d017a43703958
RUN set -eu; \
    case "${TARGETARCH:-}" in \
      amd64) arch=x86_64; sha="${WASM_BINDGEN_SHA256_AMD64}" ;; \
      arm64) arch=aarch64; sha="${WASM_BINDGEN_SHA256_ARM64}" ;; \
      *) echo "geen gepinde wasm-bindgen-cli-binary voor TARGETARCH='${TARGETARCH:-}'" >&2; exit 1 ;; \
    esac; \
    dir="wasm-bindgen-${WASM_BINDGEN_VERSION}-${arch}-unknown-linux-musl"; \
    curl -fsSLo "/tmp/${dir}.tar.gz" \
      "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/${dir}.tar.gz"; \
    echo "${sha}  /tmp/${dir}.tar.gz" | sha256sum -c -; \
    tar -xzf "/tmp/${dir}.tar.gz" -C /tmp; \
    install -m0755 "/tmp/${dir}/wasm-bindgen" /usr/local/bin/wasm-bindgen; \
    rm -rf "/tmp/${dir}.tar.gz" "/tmp/${dir}"; \
    wasm-bindgen --version
WORKDIR /build
COPY packages/Cargo.lock packages/Cargo.toml ./
# wasm-bindgen-cli en de wasm-bindgen-crate moeten exact dezelfde versie
# hebben; lopen ze uiteen dan faalt de bindgen-stap verderop met een cryptische
# melding of levert hij verkeerde bindings. Hier faalt het meteen, met de twee
# versies en de vindplaats van de nieuwe checksums erbij.
RUN set -eu; \
    locked=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock \
      | sed -n '/^version = /{s/^version = "\(.*\)"$/\1/p;q;}'); \
    if [ -z "$locked" ]; then \
      echo "kon wasm-bindgen-versie niet uit packages/Cargo.lock lezen" >&2; exit 1; \
    fi; \
    if [ "$locked" != "${WASM_BINDGEN_VERSION}" ]; then \
      echo "versie-mismatch: wasm-bindgen-cli is gepind op ${WASM_BINDGEN_VERSION}, packages/Cargo.lock heeft ${locked}." >&2; \
      echo "Werk WASM_BINDGEN_VERSION en beide WASM_BINDGEN_SHA256_* in frontend/Dockerfile bij naar ${locked}; de checksums staan naast de release op" >&2; \
      echo "  https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/${locked}" >&2; \
      exit 1; \
    fi
# Trim workspace members to engine + shared (the only crates the wasm build
# needs); everything else is dropped so chef does not try to resolve their
# source paths. The `grep` assertion fails the build loudly if
# packages/Cargo.toml is ever reformatted away from a multi-line members
# array — without it the `sed` patterns would silently no-op.
# When adding a new workspace member that this image does NOT build, add it
# to this exclusion list as well; otherwise chef will fail to find its source.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d' Cargo.toml
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 1b: Build frontend (with WASM module)
FROM node:26-alpine@sha256:0b36e8c136b94cd4fcf02188228e76c31ad5872eef3fec8cbd2eee500cfd9e80 AS frontend-builder
WORKDIR /app
# Workspace manifests first so `npm ci` is cached independently of source.
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend/package.json frontend/
COPY frontend-lawmaking/package.json frontend-lawmaking/
COPY frontend-demo/package.json frontend-demo/
COPY frontend-poc-portal/package.json frontend-poc-portal/
# --ignore-scripts: een gekaapte release mag zijn install-hooks hier niet
# draaien; de build eronder heeft ze niet nodig.
RUN npm ci --ignore-scripts
# Source: the shared package + the editor frontend, plus the WASM module.
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend/ frontend/
COPY --from=wasm-builder /wasm-pkg/ frontend/public/wasm/pkg/
RUN npm run build -w frontend

# Stage 2: Chef base
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:2ea3db105d38fdfa4e31f366674287fcaa828087e2fe3973befdc537f2d443b1 AS chef
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

# Stage 3: Plan dependencies
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
# Trim workspace members to those actually built by this Dockerfile so chef
# does not try to resolve crates whose source we do not COPY. The `grep`
# assertion ensures the multi-line members format is intact; otherwise the
# `sed` patterns would silently no-op.
# When adding a new workspace member that this image does NOT build, add it
# to this exclusion list as well; otherwise chef will fail to find its source.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d' Cargo.toml
COPY packages/auth/ auth/
COPY packages/editor-api/ editor-api/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/harvester/ harvester/
COPY packages/pipeline/ pipeline/
COPY packages/shared/ shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: Build Rust binary (deps cached separately from source)
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

COPY packages/auth/ auth/
COPY packages/editor-api/ editor-api/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/harvester/ harvester/
COPY packages/pipeline/ pipeline/
COPY packages/shared/ shared/
# regelrecht-corpus (annotation-validation feature) embeds the note schema
# via `include_str!("../../../schema/...")`, relative to packages/corpus/src/.
# WORKDIR is /build and the crate is COPYed to /build/corpus/, so that
# literal resolves to /schema/... here. Put the canonical schema there so
# the embed compiles; this is the same file the source tree has at repo
# root (no second copy — it is COPYed, not vendored).
COPY schema/ /schema/
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-editor-api

# Stage 5: Runtime
FROM alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
RUN apk add --no-cache ca-certificates git && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-editor-api /usr/local/bin/
COPY --from=frontend-builder /app/frontend/dist /app/static
COPY corpus-registry.yaml /app/corpus-registry.yaml
COPY corpus/regulation/ /app/corpus/regulation/

WORKDIR /app
ENV STATIC_DIR=/app/static
ENV CORPUS_REGISTRY_PATH=/app/corpus-registry.yaml
USER app
EXPOSE 8000
CMD ["regelrecht-editor-api"]
