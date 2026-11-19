# syntax=docker/dockerfile:1
# wetsproces: een wetsprocessysteem dat één wetsproces uitvoert op de cellen
# van de regelrecht-methode, voor alle gebruikers tegelijk. Wat het uitvoert
# staat in het manifest packages/wetsproces/bron.yaml; het image neemt dat
# manifest mee, met de mappen die het noemt, op hun plek in de repository.
#
# Zelfde opzet als de andere Rust-images: cargo-chef voor de dependencies,
# een deps-stage die warm-buildcache.yml vooraf bouwt, en een kale runtime.

# Stage 1: chef-basis.
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

# Stage 2: dependencies plannen.
FROM chef AS planner
COPY packages/Cargo.lock packages/Cargo.toml ./
# Members die dit image niet bouwt eruit knippen, zodat chef hun bronpad niet
# hoeft te vinden. De grep-assertie valt om als de members-array ooit op één
# regel wordt gezet; dan zou de sed er stilletjes langs lopen.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"editor-api"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"tui"/d; /"wetsproces-app"/d; /"poc-wetsgeschiedenis"/d' Cargo.toml
# The login: OIDC against Keycloak and the session keys (src/login.rs).
COPY packages/auth/ auth/
COPY packages/cel/ cel/
COPY packages/corpus/ corpus/
# regelrecht-corpus hangt optioneel aan github; wetsproces zet die feature
# uit, maar cargo leest het manifest van elke path-dependency.
COPY packages/github/ github/
COPY packages/engine/ engine/
# The protocol of FSC voor Burgers, shared with the app.
COPY packages/fsc4b/ fsc4b/
COPY packages/law-model/ law-model/
COPY packages/poc-chronolexografie/ poc-chronolexografie/
COPY packages/shared/ shared/
COPY packages/wetsproces/ wetsproces/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: alleen de dependencies.
FROM chef AS deps
COPY packages/Cargo.lock ./
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Up to here only the dependencies, as a stage of its own: warm-buildcache.yml
# builds just this stage on every push to main, so the cook layer is in the
# cache before the next PR needs it.
FROM deps AS builder
# `cargo chef cook` schrijft de workspace-Cargo.toml zonder
# `[workspace.lints]`, wat members met `[lints] workspace = true` breekt.
COPY --from=planner /build/Cargo.toml ./Cargo.toml
COPY packages/auth/ auth/
COPY packages/cel/ cel/
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
# The protocol of FSC voor Burgers, shared with the app.
COPY packages/fsc4b/ fsc4b/
COPY packages/law-model/ law-model/
COPY packages/poc-chronolexografie/ poc-chronolexografie/
COPY packages/shared/ shared/
COPY packages/wetsproces/ wetsproces/
COPY schema/ /schema/
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-wetsproces

# Stage 4: the screens (frontend-wetsproces), built in the npm workspace of
# the repository: one build per surface, in dist/burger, dist/backoffice and
# dist/all. The server serves the build of its WETSPROCES_SURFACE beside its
# API, so one image runs as the environment of citizens and as that of the
# back office.
FROM node:24-alpine@sha256:01743339035a5c3c11a373cd7c83aeab6ed1457b55da6a69e014a95ac4e4700b AS frontend
WORKDIR /app
# Workspace manifests first so `npm ci` is cached independently of source.
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend-wetsproces/package.json frontend-wetsproces/
# --ignore-scripts: a hijacked release may not run its install hooks here; the
# build below needs none of them.
RUN npm ci --ignore-scripts
COPY packages/frontend-shared/ packages/frontend-shared/
COPY frontend-wetsproces/ frontend-wetsproces/
RUN npm run build -w frontend-wetsproces

# Stage 5: runtime.
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

COPY --from=builder /build/target/release/regelrecht-wetsproces /usr/local/bin/
COPY --from=frontend /app/frontend-wetsproces/dist /app/frontend
# Het manifest en de mappen die het noemt, op hun plek onder /app zoals in de
# repository: `repo_root: ../..` vanuit /app/packages/wetsproces wijst dan naar
# /app. Het hele corpus van de chronolexografie-poc gaat mee, zodat een map
# die het manifest erbij noemt geen wijziging hier vraagt; de server leest
# alleen wat het manifest noemt.
COPY packages/wetsproces/bron.yaml /app/packages/wetsproces/bron.yaml
COPY packages/wetsproces/casussen/ /app/packages/wetsproces/casussen/
# The UI spec says which field of the application the login fills.
COPY packages/wetsproces/ui_spec.yaml /app/packages/wetsproces/ui_spec.yaml
COPY packages/poc-chronolexografie/corpus/ /app/packages/poc-chronolexografie/corpus/

WORKDIR /app
# Geen WETSPROCES_MODE: de modus beslist over de klok en over wie de server
# mag gebruiken, dus de deployment zet hem zelf (src/config.rs). Zonder weigert
# de server te starten. WETSPROCES_STORE en DATABASE_URL (of
# DATABASE_SERVER_FULL) kiezen waar de kroniek staat; ook die zet de
# deployment.
# The login (OIDC_*, BASE_URL, WETSPROCES_SUBJECT_CLAIM) is set by the
# deployment as well; stable mode refuses to start without it. So is
# WETSPROCES_SURFACE: stable mode refuses to start without `burger` or
# `backoffice`, one deployment per side.
ENV WETSPROCES_MANIFEST=/app/packages/wetsproces/bron.yaml \
    WETSPROCES_FRONTEND=/app/frontend \
    WETSPROCES_PORT=8000
USER app
EXPOSE 8000
CMD ["regelrecht-wetsproces"]
