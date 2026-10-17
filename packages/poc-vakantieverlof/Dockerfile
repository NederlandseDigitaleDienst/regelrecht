# syntax=docker/dockerfile:1
# vakantieverlof: het proces van vakantieverlof (Leerplichtwet 1969 art. 13a)
# naspelen op de cel van een fictieve school (chronolexografie).
#
# Eigen component, niet gepubliceerd: alleen het poc-portaal bereikt hem, op
# http://{deployment}-vakantieverlof:8000. Hij draait de cel (packages/cel) en
# de engine in een eigen proces, met de kroniek in het geheugen, en past
# daarom niet in het statische poc-image. Zelfde opzet als packages/poc-napp.

# Stage 1: de frontend.
FROM node:26-alpine@sha256:2d984a15c9b54fd0aeb608b8e0d0d83529eb34d2966db27a1fb4f1edc3d298a3 AS frontend-builder
WORKDIR /app
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend-poc-vakantieverlof/package.json frontend-poc-vakantieverlof/
RUN npm ci --ignore-scripts
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend-poc-vakantieverlof/ frontend-poc-vakantieverlof/
# Zelfde voorvoegsel als VERLOF_BASE_PATH hieronder: het portaal stuurt het pad
# ongewijzigd door.
RUN POC_BASE=/vakantieverlof/ npm run build -w poc-vakantieverlof

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

# Stage 3: dependencies plannen.
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"tui"/d' Cargo.toml
COPY packages/cel/ cel/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-vakantieverlof/ poc-vakantieverlof/
COPY packages/shared/ shared/
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
COPY packages/cel/ cel/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-vakantieverlof/ poc-vakantieverlof/
COPY packages/shared/ shared/
COPY schema/ /schema/
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-poc-vakantieverlof

# Stage 5: runtime.
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-poc-vakantieverlof /usr/local/bin/
COPY --from=frontend-builder /app/frontend-poc-vakantieverlof/dist /app/static
# Alleen de regelwerken die de cel draait, en de cel van de school met haar
# reductie. Dezelfde lijst als REGELWERKEN in src/kern.rs.
COPY corpus/regulation/nl/wet/leerplichtwet_1969/ /app/corpus/regulation/nl/wet/leerplichtwet_1969/
COPY corpus/regulation/nl/wet/algemene_wet_bestuursrecht/ /app/corpus/regulation/nl/wet/algemene_wet_bestuursrecht/
COPY corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaar_obs_de_klimop/ /app/corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaar_obs_de_klimop/
COPY corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaargrenzen_obs_de_klimop/ /app/corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaargrenzen_obs_de_klimop/
COPY corpus/cells/obs_de_klimop/ /app/corpus/cells/obs_de_klimop/

WORKDIR /app
ENV VERLOF_CORPUS_DIR=/app/corpus \
    VERLOF_STATIC_DIR=/app/static \
    VERLOF_PORT=8000 \
    VERLOF_BASE_PATH=/vakantieverlof/
USER app
EXPOSE 8000
CMD ["regelrecht-poc-vakantieverlof"]
