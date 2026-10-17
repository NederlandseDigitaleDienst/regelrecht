# Stage 0: WASM engine. Mirrors frontend/Dockerfile's and frontend-demo's
# wasm-builder stage; the three must stay identical (same pinned
# wasm-bindgen-cli, same member trimming). The landing page runs the zorgtoeslag
# scenario in the visitor's browser, so the engine ships with the site.
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
      echo "Werk WASM_BINDGEN_VERSION en beide WASM_BINDGEN_SHA256_* in docs/Dockerfile bij naar ${locked}; de checksums staan naast de release op" >&2; \
      echo "  https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/${locked}" >&2; \
      exit 1; \
    fi
# Trim workspace members to engine + law-model + shared. When adding a new
# workspace member that this image does NOT build, add it here as well.
RUN grep -qE '^members = \[$' Cargo.toml && \
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"auth"/d; /"corpus"/d; /"editor-api"/d; /"github"/d; /"harvester"/d; /"pipeline"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-vakantieverlof"/d; /"tui"/d' Cargo.toml
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/shared/ shared/
RUN cargo build --target wasm32-unknown-unknown --release --features wasm -p regelrecht-engine && \
    wasm-bindgen --target web --out-dir /wasm-pkg \
      target/wasm32-unknown-unknown/release/regelrecht_engine.wasm

# Stage 1: Build
# Builder is node:24-bookworm-slim (glibc), not node:24-alpine (musl):
# rehype-mermaid renders ```mermaid blocks to inline SVG at build time via
# a headless Chromium driven by Playwright, and `playwright install
# --with-deps chromium` only resolves its system dependencies on a
# Debian/glibc base. Digest-pinned like the runtime stage. The runtime
# stage is unchanged (unprivileged nginx).
FROM node:24-bookworm-slim@sha256:24dc26ef1e3c3690f27ebc4136c9c186c3133b25563ae4d7f0692e4d1fe5db0e AS builder
WORKDIR /app
COPY docs/package.json docs/package-lock.json* docs/.npmrc ./
# --ignore-scripts: een gekaapte release mag zijn install-hooks hier niet
# draaien. Deze stage draait alleen `astro build`; de enige browser die daarbij
# nodig is komt van de expliciete `playwright install` hieronder. Puppeteer
# (dat zijn Chrome wél in een postinstall haalt) wordt hier niet gebruikt —
# pa11y draait in de CI-baan, niet in dit image.
RUN npm ci --ignore-scripts
COPY docs/ .
# The landing page reads the law, its scenario and its annotations straight out
# of the corpus at build time, so that what it shows cannot drift from what the
# repository holds (see docs/src/lib/landing-demo.ts). Those files live outside
# docs/, and the build resolves them one level above the project directory, so
# the layout here mirrors the repository: /app is docs/, /corpus is corpus/.
COPY corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/ /corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/
COPY corpus/annotations/wet_op_de_zorgtoeslag/ /corpus/annotations/wet_op_de_zorgtoeslag/

# What the page needs to run the scenario in the visitor's browser: the engine
# from stage 0, and the laws, the scenario and the canonical-grammar runner that
# script/landing-laws.sh assembles out of the repository. Both trees are
# gitignored build products, so they are produced here rather than copied from a
# working directory that may or may not have them.
COPY --from=wasm-builder /wasm-pkg/ ./public/wasm/pkg/
COPY corpus/regulation/ /corpus/regulation/
COPY packages/frontend-shared/src/ /packages/frontend-shared/src/
COPY script/landing-laws.sh /script/landing-laws.sh
RUN LANDING_REPO_ROOT=/ LANDING_DOCS_ROOT=/app /script/landing-laws.sh

RUN npx playwright install --with-deps chromium
RUN npm run build

# Stage 2: Production (unprivileged nginx for Kubernetes/OpenShift)
FROM nginxinc/nginx-unprivileged:1.27-alpine@sha256:65e3e85dbaed8ba248841d9d58a899b6197106c23cb0ff1a132b7bfe0547e4c0
COPY docs/nginx.conf /etc/nginx/conf.d/default.conf
COPY deploy/nginx/security-headers.conf /etc/nginx/security-headers.conf
COPY --from=builder /app/dist /usr/share/nginx/html
EXPOSE 8000
CMD ["nginx", "-g", "daemon off;"]
