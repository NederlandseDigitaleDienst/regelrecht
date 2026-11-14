# syntax=docker/dockerfile:1
# De wetsgeschiedenis: de pagina, en een dienst die een wet ophaalt vanaf haar
# BWB-id en haar Kamerdossiers afspeelt. Draait als eigen ZAD-component
# (`wetsgeschiedenis`), niet gepubliceerd; het poc-portaal stuurt
# /wetsgeschiedenis/... ongewijzigd door, dus de dienst serveert zichzelf onder
# dat voorvoegsel (WG_BASE_PATH).
#
# De dienst is Rust (axum); de afspeelketen is Python (werker.py met
# afspeler/), als eigen proces per opdracht, en haalt de stukken met curl. De
# basisimages zijn dezelfde, met dezelfde digest, als in
# packages/poc-chronolexografie/Dockerfile.

# Stage 1: de frontend, met het voorvoegsel van het portaal.
FROM node:26-alpine@sha256:2d984a15c9b54fd0aeb608b8e0d0d83529eb34d2966db27a1fb4f1edc3d298a3 AS frontend-builder
WORKDIR /app
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend-poc-wetsgeschiedenis/package.json frontend-poc-wetsgeschiedenis/
RUN npm ci --ignore-scripts
COPY corpus-poc/wetsgeschiedenis/ corpus-poc/wetsgeschiedenis/
COPY frontend-poc-wetsgeschiedenis/ frontend-poc-wetsgeschiedenis/
RUN POC_BASE=/wetsgeschiedenis/ npm run build -w poc-wetsgeschiedenis

# Stage 2: chef-basis.
# Rust version: keep in sync with rust-toolchain.toml (source of truth).
FROM rust:1.96-alpine@sha256:a41f7740f8b45d45795624eec13a8b42263cc700f19f7e4e86e04d3dda08a479 AS chef
RUN apk add --no-cache musl-dev curl xz
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

# Stage 3: dependencies plannen. De crate hangt aan geen ander lid van de
# workspace; de rest knippen we weg.
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"auth"/d; /"cel"/d; /"corpus"/d; /"editor-api"/d; /"engine"/d; /"github"/d; /"harvester"/d; /"law-model"/d; /"pipeline"/d; /"poc-chronolexografie"/d; /"poc-napp"/d; /"poc-portal"/d; /"shared"/d; /"tui"/d' Cargo.toml
COPY packages/poc-wetsgeschiedenis/ poc-wetsgeschiedenis/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: de binary.
FROM chef AS builder
COPY packages/Cargo.lock ./
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# `cargo chef cook` schrijft de workspace-Cargo.toml zonder
# `[workspace.lints]`, wat members met `[lints] workspace = true` breekt.
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY packages/poc-wetsgeschiedenis/ poc-wetsgeschiedenis/
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-poc-wetsgeschiedenis

# Stage 5: runtime. Python en curl zijn voor de werker.
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN apk add --no-cache python3 py3-yaml curl ca-certificates && \
    addgroup -S app && adduser -S app -G app && \
    mkdir -p /data && chown app:app /data
WORKDIR /app
COPY --from=builder /build/target/release/regelrecht-poc-wetsgeschiedenis /usr/local/bin/
COPY packages/poc-wetsgeschiedenis/werker.py ./
COPY packages/poc-wetsgeschiedenis/afspeler/ afspeler/
COPY corpus-poc/wetsgeschiedenis/gegevens/ vast/
COPY --from=frontend-builder /app/frontend-poc-wetsgeschiedenis/dist static/

# /data houdt wat bezoekers ophalen. Zonder volume is het na een herstart weg;
# de dossiers in vast/ blijven altijd.
ENV WG_PORT=8000 \
    WG_BASE_PATH=/wetsgeschiedenis/ \
    WG_STATIC_DIR=/app/static \
    WG_VAST_DIR=/app/vast \
    WG_DATA=/data \
    WG_WERKER=/app/werker.py \
    PYTHONDONTWRITEBYTECODE=1
USER app
EXPOSE 8000
CMD ["regelrecht-poc-wetsgeschiedenis"]
