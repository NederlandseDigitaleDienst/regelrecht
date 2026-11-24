# syntax=docker/dockerfile:1
# The citizen's own app in the browser (frontend-regelhet, `--mode
# burgerapp`): a static site that talks to RegelHets over FSC voor Burgers,
# with the kern of the chronolexografie poc as WebAssembly. No backend: the
# app's keys stay in the citizen's browser. Built from the root of the
# repository by .forgejo/workflows/burgerapp-image.yml; deployed with
# charts/burgerapp.
#
# Stage 1: the kern as WASM. Mirrors the wasm-builder stage of
# packages/poc-portal/Dockerfile: the same pinned wasm-bindgen-cli, the same
# trimming of the workspace.
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:a41f7740f8b45d45795624eec13a8b42263cc700f19f7e4e86e04d3dda08a479 AS wasm-builder
RUN apk add --no-cache musl-dev curl
RUN rustup target add wasm32-unknown-unknown
ARG TARGETARCH
ARG WASM_BINDGEN_VERSION=0.2.114
ARG WASM_BINDGEN_SHA256_AMD64=ce21be002bdbc22ede2a5cf250cb801d2106956fc3c529c75d9452729db3490c
ARG WASM_BINDGEN_SHA256_ARM64=39103878ad3c0016f2d04b7b4a6206c974240286852abac5736d017a43703958
RUN set -eu; \
    case "${TARGETARCH:-}" in \
      amd64) arch=x86_64; sha="${WASM_BINDGEN_SHA256_AMD64}" ;; \
      arm64) arch=aarch64; sha="${WASM_BINDGEN_SHA256_ARM64}" ;; \
      *) echo "no pinned wasm-bindgen-cli binary for TARGETARCH='${TARGETARCH:-}'" >&2; exit 1 ;; \
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
RUN set -eu; \
    locked=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock \
      | sed -n '/^version = /{s/^version = "\(.*\)"$/\1/p;q;}'); \
    if [ "$locked" != "${WASM_BINDGEN_VERSION}" ]; then \
      echo "wasm-bindgen-cli is pinned at ${WASM_BINDGEN_VERSION}, packages/Cargo.lock has '${locked}'." >&2; \
      echo "Update WASM_BINDGEN_VERSION and both WASM_BINDGEN_SHA256_* here to ${locked}; the checksums are beside the release at" >&2; \
      echo "  https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/${locked}" >&2; \
      exit 1; \
    fi
# Only the crates the kern needs stay members. A new workspace member that
# this image does not build goes on this list as well.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"tui"/d; /"regelhet"/d; /"regelhet-app"/d; /"fsc4b"/d; /"poc-wetsgeschiedenis"/d' Cargo.toml
COPY packages/cel/ cel/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-chronolexografie/ poc-chronolexografie/
COPY packages/shared/ shared/
# A dev-dependency of the kern's tests that the WASM build needs not (see
# packages/poc-portal/Dockerfile).
RUN grep -q '^regelrecht-corpus = ' poc-chronolexografie/Cargo.toml && \
    sed -i '/^regelrecht-corpus = /d' poc-chronolexografie/Cargo.toml
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-poc-chronolexografie && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_poc_chronolexografie.wasm

# Stage 2: the app.
FROM node:24-alpine@sha256:01743339035a5c3c11a373cd7c83aeab6ed1457b55da6a69e014a95ac4e4700b AS builder
WORKDIR /app
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend-regelhet/package.json frontend-regelhet/
# --ignore-scripts: a hijacked release may not run its install hooks here.
RUN npm ci --ignore-scripts
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend-regelhet/ frontend-regelhet/
COPY --from=wasm-builder /wasm-pkg/ frontend-regelhet/burgerapp/wasm/pkg/
RUN npm run build:burgerapp -w frontend-regelhet

# Stage 3: unprivileged nginx. The deployment mounts regelhets.json, the list
# of RegelHets, over the default, and sets BURGERAPP_CONNECT_SRC to their
# origins: the browser may call those and nothing else.
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
# The default list points at `just regelhet` on this machine; the chart sets
# both for a deployment.
ENV BURGERAPP_CONNECT_SRC="http://localhost:7180"
COPY frontend-regelhet/burgerapp/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY deploy/nginx/security-headers.conf /etc/nginx/security-headers.conf
COPY --from=builder /app/frontend-regelhet/dist-burgerapp /usr/share/nginx/html
EXPOSE 8000
