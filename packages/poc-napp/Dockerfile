# syntax=docker/dockerfile:1
# napp: het subsidieproces uit het wetsvoorstel Wet op de politieke partijen.
#
# Eigen component, niet gepubliceerd: alleen het poc-portaal bereikt hem, op
# http://{deployment}-napp:8000. Hij heeft een eigen proces, een eigen SQLite
# en een eigen login, en past daarom niet in het statische poc-image.
#
# Anders dan in de losse PoC-repo klont dit image regelrecht niet uit GitHub:
# engine en auth zijn workspace-members. Dat is de hele winst van de verhuizing
# — de engine-versie is die van deze commit, en drift valt om in CI.

# Stage 1: WASM-engine voor de in-browser scenariorunner van de beoordelaar.
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
    if [ "$locked" != "${WASM_BINDGEN_VERSION}" ]; then \
      echo "versie-mismatch: wasm-bindgen-cli is gepind op ${WASM_BINDGEN_VERSION}, packages/Cargo.lock heeft ${locked}." >&2; \
      exit 1; \
    fi
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d' Cargo.toml
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 2: de drie portalen (publiek, aanvrager, beoordelaar).
FROM node:26-alpine@sha256:2d984a15c9b54fd0aeb608b8e0d0d83529eb34d2966db27a1fb4f1edc3d298a3 AS frontend-builder
WORKDIR /app
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend/package.json frontend/
COPY frontend-lawmaking/package.json frontend-lawmaking/
COPY frontend-poc-portal/package.json frontend-poc-portal/
COPY frontend-poc-terugbetaalregimes/package.json frontend-poc-terugbetaalregimes/
COPY frontend-poc-nieuwkomersbekostiging/package.json frontend-poc-nieuwkomersbekostiging/
COPY frontend-poc-napp/package.json frontend-poc-napp/
RUN npm ci --ignore-scripts
# De wetten en scenario's worden at build-time meegebundeld (import ... ?raw en
# import.meta.glob), dus ze moeten er staan vóór de build.
COPY corpus-poc/napp/ corpus-poc/napp/
COPY frontend-poc-napp/ frontend-poc-napp/
COPY --from=wasm-builder /wasm-pkg/ frontend-poc-napp/public/wasm/pkg/
# Zelfde voorvoegsel als NAPP_BASE_PATH hieronder: het portaal stuurt het pad
# ongewijzigd door, dus de bundel moet met /napp/ naar zichzelf verwijzen.
RUN POC_BASE=/napp/ npm run build -w poc-napp

# Stage 3: chef-basis.
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

# Stage 4: dependencies plannen.
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"editor-api"/d; /"harvester"/d; /"pipeline"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d' Cargo.toml
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-napp/ poc-napp/
COPY packages/shared/ shared/
COPY corpus-poc/ /corpus-poc/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 5: de binary.
FROM chef AS deps
COPY packages/Cargo.lock ./
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Up to here only the dependencies, as a stage of its own: warm-buildcache.yml
# builds just this stage on every push to main, so the cook layer is in the
# cache before the next PR needs it, whether or not main deploys.
FROM deps AS builder
# `cargo chef cook` schrijft de workspace-Cargo.toml zonder
# `[workspace.lints]`, wat members met `[lints] workspace = true` breekt.
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-napp/ poc-napp/
COPY packages/shared/ shared/
COPY schema/ /schema/
# napp bakt zijn wetten in met include_str! vanuit poc-napp/src/, drie niveaus
# omhoog: /corpus-poc/napp/law/...
COPY corpus-poc/ /corpus-poc/
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-poc-napp

# Stage 6: runtime.
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app && \
    mkdir /data && chown app:app /data

COPY --from=builder /build/target/release/regelrecht-poc-napp /usr/local/bin/
COPY --from=frontend-builder /app/frontend-poc-napp/dist /app/static
COPY corpus-poc/napp/law/ /app/law/

WORKDIR /app
ENV NAPP_LAW_DIR=/app/law \
    NAPP_STATIC_DIR=/app/static \
    NAPP_PORT=8000 \
    # Gelijk aan POC_BASE hierboven: het portaal stuurt /napp/... ongewijzigd
    # door, dus napp moet zichzelf onder datzelfde voorvoegsel serveren.
    NAPP_BASE_PATH=/napp/ \
    DATABASE_URL="sqlite:/data/napp.db?mode=rwc"
USER app
EXPOSE 8000
CMD ["regelrecht-poc-napp"]
