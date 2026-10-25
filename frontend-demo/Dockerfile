# syntax=docker/dockerfile:1
# Demo (demo.regelrecht.rijks.app): a static Vue app that runs the regelrecht
# engine as WASM in the browser, over the demo corpus in corpus/demo. No backend.
#
# Stage 1a: WASM engine. Mirrors frontend/Dockerfile's wasm-builder stage; the
# two must stay identical (same pinned wasm-bindgen-cli, same member trimming).
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:2ea3db105d38fdfa4e31f366674287fcaa828087e2fe3973befdc537f2d443b1 AS wasm-builder
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
RUN set -eu; \
    locked=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock \
      | sed -n '/^version = /{s/^version = "\(.*\)"$/\1/p;q;}'); \
    if [ -z "$locked" ]; then \
      echo "kon wasm-bindgen-versie niet uit packages/Cargo.lock lezen" >&2; exit 1; \
    fi; \
    if [ "$locked" != "${WASM_BINDGEN_VERSION}" ]; then \
      echo "versie-mismatch: wasm-bindgen-cli is gepind op ${WASM_BINDGEN_VERSION}, packages/Cargo.lock heeft ${locked}." >&2; \
      echo "Werk WASM_BINDGEN_VERSION en beide WASM_BINDGEN_SHA256_* in frontend-demo/Dockerfile bij naar ${locked}; de checksums staan naast de release op" >&2; \
      echo "  https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/${locked}" >&2; \
      exit 1; \
    fi
# Trim workspace members to engine + law-model + shared. When adding a new
# workspace member that this image does NOT build, add it here as well.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d; /"poc-wetsgeschiedenis"/d' Cargo.toml
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 1b: the Vue bundle (npm workspace root builds the frontend-demo workspace)
FROM node:24-alpine@sha256:01743339035a5c3c11a373cd7c83aeab6ed1457b55da6a69e014a95ac4e4700b AS builder
WORKDIR /app
# Workspace manifests first so `npm ci` is cached independently of source.
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend/package.json frontend/
COPY frontend-lawmaking/package.json frontend-lawmaking/
COPY frontend-demo/package.json frontend-demo/
# --ignore-scripts: een gekaapte release mag zijn install-hooks hier niet
# draaien; de build eronder heeft ze niet nodig.
RUN npm ci --ignore-scripts
# Source: the shared package, this frontend, and the demo corpus it bundles
# (prebuild copies corpus/demo into public/data).
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend-demo/ frontend-demo/
COPY corpus/demo/ corpus/demo/
COPY --from=wasm-builder /wasm-pkg/ frontend-demo/public/wasm/pkg/
# The recorded walkthrough's videos: release assets named and checksummed in
# corpus/demo/walkthrough/timeline.json. Without a timeline this does nothing.
RUN node frontend-demo/scripts/fetch-walkthrough-media.mjs
RUN npm run build -w frontend-demo

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
# The "why" button's backend (server/why.mjs) runs the Claude Code CLI under
# Node. Root only for the install; the image runs as the nginx user again below.
USER root
RUN apk add --no-cache nodejs curl bash
# Pinned, like the poc-portal image: a build of the same commit should not
# switch CLI versions. The installer detects musl and fetches the matching build.
# `bash -s` because the installer is a bash script, and `install` rather than
# `mv` because ~/.local/bin/claude is a symlink into /root, which the nginx user
# cannot follow.
ARG CLAUDE_CODE_VERSION=2.1.236
RUN curl -fsSL https://claude.ai/install.sh | bash -s -- "${CLAUDE_CODE_VERSION}" && \
    install -m0755 "$(readlink -f /root/.local/bin/claude)" /usr/local/bin/claude && \
    rm -rf /root/.local && \
    claude --version
ENV DISABLE_AUTOUPDATER=1 \
    CLAUDE_CODE_SKIP_PROMPT_HISTORY=true
COPY frontend-demo/server/why.mjs frontend-demo/server/prompt.mjs /app/why/
COPY frontend-demo/start.sh /usr/local/bin/start-demo
RUN chmod 0755 /usr/local/bin/start-demo
# Numeric, as the base image sets it: a named user fails Kubernetes'
# runAsNonRoot check when no runAsUser is given.
USER 101

COPY frontend-demo/nginx.conf /etc/nginx/conf.d/default.conf
COPY deploy/nginx/security-headers.conf /etc/nginx/security-headers.conf
COPY --from=builder /app/frontend-demo/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["start-demo"]
