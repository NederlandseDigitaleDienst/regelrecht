# syntax=docker/dockerfile:1
# Het poc-portaal: één adres met de proof-of-concepts erachter, elk achter een
# eigen wachtwoord. Serveert de statische pocs uit dit image en stuurt de
# `proxy`-pocs in-cluster door.
#
# De WASM- en chef-trappen zijn bewust gelijk aan frontend/Dockerfile, tot de
# gepinde versies en checksums aan toe. Dat is geen kopieerwerk uit gemak: de
# versie-assertie tussen wasm-bindgen-cli en de crate in Cargo.lock is
# load-bearing, en twee images die dezelfde engine bouwen horen dat op dezelfde
# manier te doen. Wijzigt de ene, dan de andere ook —
# script/dockerfile-consistency.test.mjs bewaakt de workspace-kant daarvan.

# Stage 1a: WASM voor de browser-pocs: de engine, en chronolexografie (de cel,
# de engine en het spel, packages/poc-chronolexografie).
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
    if [ -z "$locked" ]; then \
      echo "kon wasm-bindgen-versie niet uit packages/Cargo.lock lezen" >&2; exit 1; \
    fi; \
    if [ "$locked" != "${WASM_BINDGEN_VERSION}" ]; then \
      echo "versie-mismatch: wasm-bindgen-cli is gepind op ${WASM_BINDGEN_VERSION}, packages/Cargo.lock heeft ${locked}." >&2; \
      echo "Werk WASM_BINDGEN_VERSION en beide WASM_BINDGEN_SHA256_* in packages/poc-portal/Dockerfile bij naar ${locked}; de checksums staan naast de release op" >&2; \
      echo "  https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/${locked}" >&2; \
      exit 1; \
    fi
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"tui"/d' Cargo.toml
COPY packages/cel/ cel/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-chronolexografie/ poc-chronolexografie/
COPY packages/shared/ shared/
# The tests of chronolexografie load a source with the portal's own loader, a
# dev-dependency on poc-portal. Cargo reads the manifest of every dependency,
# dev ones too, and the portal with its own dependencies is not in this stage;
# the WASM build needs none of it, so the line goes.
RUN grep -q '^regelrecht-poc-portal = ' poc-chronolexografie/Cargo.toml && \
    sed -i '/^regelrecht-poc-portal = /d' poc-chronolexografie/Cargo.toml
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-poc-chronolexografie && \
    wasm-bindgen --target web --out-dir /wasm-chronolexografie \
      target/wasm32-unknown-unknown/release/regelrecht_poc_chronolexografie.wasm

# Stage 1b: de frontends — het portaal zelf plus elke statische poc.
FROM node:26-alpine@sha256:2d984a15c9b54fd0aeb608b8e0d0d83529eb34d2966db27a1fb4f1edc3d298a3 AS frontend-builder
WORKDIR /app
COPY package.json package-lock.json .npmrc ./
COPY packages/frontend-shared/package.json packages/frontend-shared/
COPY frontend/package.json frontend/
COPY frontend-lawmaking/package.json frontend-lawmaking/
COPY frontend-poc-portal/package.json frontend-poc-portal/
COPY frontend-poc-terugbetaalregimes/package.json frontend-poc-terugbetaalregimes/
COPY frontend-poc-nieuwkomersbekostiging/package.json frontend-poc-nieuwkomersbekostiging/
COPY packages/poc-chronolexografie/frontend/package.json packages/poc-chronolexografie/frontend/
RUN npm ci --ignore-scripts

# De bron van het gedeelde pakket. npm ci hierboven legt alleen de symlink in
# node_modules; zonder deze kopie staat daar een lege map en valt de build om
# op de imports uit de pocs (Panel, de Gherkin-runner, usePersistedState).
COPY packages/frontend-shared/ packages/frontend-shared/

# Het portaal: alleen het ontwerpsysteem voor zijn eigen twee pagina's.
COPY frontend-poc-portal/ frontend-poc-portal/
RUN npm run build -w poc-portal-assets

# Elke statische poc met zijn eigen basis. POC_BASE moet gelijk zijn aan de
# `slug` in pocs/registry.yaml: het portaal serveert /<slug>/ uit
# /app/static/<slug>, en de bundel verwijst met dat voorvoegsel naar zichzelf.
COPY corpus-poc/ corpus-poc/
COPY frontend-poc-terugbetaalregimes/ frontend-poc-terugbetaalregimes/
COPY frontend-poc-nieuwkomersbekostiging/ frontend-poc-nieuwkomersbekostiging/
COPY --from=wasm-builder /wasm-pkg/ frontend/public/wasm/pkg/
RUN POC_BASE=/terugbetaalregimes/ npm run build -w poc-terugbetaalregimes
RUN POC_BASE=/nieuwkomersbekostiging/ npm run build -w poc-nieuwkomersbekostiging

# chronolexografie draait helemaal in de browser. De bundel die de app
# meekrijgt, is de map bij `corpus` in pocs/registry.yaml, het eigen corpus
# van de poc (scripts/copy-assets.js); hier gaan het register en dat corpus
# mee.
COPY pocs/ pocs/
COPY packages/poc-chronolexografie/corpus/ packages/poc-chronolexografie/corpus/
COPY packages/poc-chronolexografie/frontend/ packages/poc-chronolexografie/frontend/
COPY --from=wasm-builder /wasm-chronolexografie/ packages/poc-chronolexografie/frontend/public/wasm/pkg/
RUN POC_BASE=/chronolexografie/ npm run build -w poc-chronolexografie

# De enige runtime-dependency van de beleidsassistent. Los geïnstalleerd en niet
# als workspace-member: hij draait naast het portaal in de runtime-trap, niet in
# een build, en heeft met de frontends niets te maken.
COPY packages/poc-assistent/package.json /assistent/package.json
RUN cd /assistent && npm install --omit=dev --ignore-scripts --no-package-lock

# Stage 2: chef-basis voor de Rust-kant.
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
# Members die dit image niet bouwt eruit knippen, zodat chef hun bronpad niet
# hoeft te vinden. De grep-assertie valt om als de members-array ooit op één
# regel wordt gezet — dan zou de sed er stilletjes langs lopen.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"editor-api"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-chronolexografie"/d; /"tui"/d' Cargo.toml
COPY packages/auth/ auth/
COPY packages/corpus/ corpus/
# regelrecht-corpus hangt aan github; het portaal gebruikt uit corpus alleen
# deployment_from_hostname, maar een dependency knip je niet weg met een sed.
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/poc-portal/ poc-portal/
COPY packages/shared/ shared/
COPY pocs/ /pocs/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 4: de binary.
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
COPY packages/poc-portal/ poc-portal/
COPY packages/shared/ shared/
# regelrecht-corpus embed het notitie-schema met include_str! relatief aan
# packages/corpus/src/, wat hier op /schema/... uitkomt.
COPY schema/ /schema/
# poc-portal embed pocs/registry.yaml op dezelfde manier: include_str! vanuit
# poc-portal/src/ met drie niveaus omhoog, dus /pocs/registry.yaml.
COPY pocs/ /pocs/
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin regelrecht-poc-portal

# Stage 5: runtime.
FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
# nodejs voor de beleidsassistent, curl om de Claude CLI te halen.
RUN apk add --no-cache ca-certificates nodejs curl bash && \
    addgroup -S app && adduser -S app -G app

# De Claude Code CLI, waarop de beleidsassistent draait. De installer detecteert
# musl zelf en haalt de linux-x64-musl-build, dus dit werkt op alpine.
#
# Gepind op een versie in plaats van `stable`: een image hoort niet stilletjes
# van CLI-versie te wisselen tussen twee builds van dezelfde commit.
ARG CLAUDE_CODE_VERSION=2.1.236
# `bash -s`, niet `sh -s`: de installer is een bash-script en alpine's ash
# valt er met een syntaxfout over.
#
# `install`, niet `mv`: de installer legt in ~/.local/bin een symlink naar de
# echte binary onder /root/.local/share, en een verplaatste symlink wijst nog
# steeds daarheen. Onder de niet-root gebruiker levert dat "Permission denied"
# op — de CLI lijkt geïnstalleerd en is onbruikbaar.
RUN curl -fsSL https://claude.ai/install.sh | bash -s -- "${CLAUDE_CODE_VERSION}" && \
    install -m0755 "$(readlink -f /root/.local/bin/claude)" /usr/local/bin/claude && \
    rm -rf /root/.local && \
    claude --version
# Het image is onveranderlijk; een zichzelf bijwerkende binary erin is ruis.
# En geen sessie-transcripts: elke aanroep is `claude -p`, eenmalig.
ENV DISABLE_AUTOUPDATER=1 \
    CLAUDE_CODE_SKIP_PROMPT_HISTORY=true

COPY --from=builder /build/target/release/regelrecht-poc-portal /usr/local/bin/
# Het portaal serveert /_assets uit de wortel en elke poc uit /app/static/<slug>.
COPY --from=frontend-builder /app/frontend-poc-portal/dist /app/static/_assets
# `base` herschrijft alleen de URL's in de bundel, niet de uitvoermap: de dist
# blijft plat. De map onder /app/static heet dus naar de slug, niet naar wat
# vite uitspuugt.
COPY --from=frontend-builder /app/frontend-poc-terugbetaalregimes/dist /app/static/terugbetaalregimes
COPY --from=frontend-builder /app/frontend-poc-nieuwkomersbekostiging/dist /app/static/nieuwkomersbekostiging
COPY --from=frontend-builder /app/packages/poc-chronolexografie/frontend/dist /app/static/chronolexografie

# De beleidsassistent: één server, per casus zijn eigen MCP-tools en prompt.
COPY packages/poc-assistent/ /app/assistent/
COPY --from=frontend-builder /assistent/node_modules /app/assistent/node_modules
# De simulatie- en patch-modules die de assistent met de browser deelt. Ze
# horen bij de app van de casus (sim/, lib/), niet bij de assistent, en
# start.sh wijst POC_APP_SRC hierheen. Alleen src/: de assistent draait geen
# Vue, hij importeert de rekenmodules.
COPY frontend-poc-terugbetaalregimes/src/ /app/assistent/app/terugbetaalregimes/src/
COPY frontend-poc-nieuwkomersbekostiging/src/ /app/assistent/app/nieuwkomersbekostiging/src/
# Het casus-corpus dat de assistent leest en bewerkt (overlays in een tmpdir).
COPY corpus-poc/ /app/casus/
COPY packages/poc-portal/start.sh /usr/local/bin/start-poc
RUN chmod 0755 /usr/local/bin/start-poc

WORKDIR /app
ENV POC_STATIC_DIR=/app/static
USER app
EXPOSE 8000
CMD ["start-poc"]
