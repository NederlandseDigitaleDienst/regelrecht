# syntax=docker/dockerfile:1
# Parameterized Dockerfile for all three pipeline images. The build stages are
# shared; select the image with --target and tell the builder which binary to
# compile with --build-arg BIN. Valid combinations (build from the repo root):
#
#   --target pipeline-api      --build-arg BIN=regelrecht-pipeline-api   (default)
#   --target harvester-worker  --build-arg BIN=regelrecht-harvest-worker
#   --target enrich-worker     --build-arg BIN=regelrecht-enrich-worker
#
# BIN must match the --target: each runtime stage copies a hardcoded binary
# path, so a non-default --target without its matching BIN compiles the wrong
# binary and then fails at `COPY --from=builder` (fail-loud, but confusing).
#
# Example:
#   docker build -f packages/pipeline/Dockerfile \
#     --build-arg BIN=regelrecht-harvest-worker --target harvester-worker .

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
    sed -i '/"admin"/d; /"arch-extract"/d; /"cel"/d; /"auth"/d; /"editor-api"/d; /"poc-napp"/d; /"poc-portal"/d; /"poc-chronolexografie"/d; /"tui"/d' Cargo.toml
COPY packages/corpus/ corpus/
COPY packages/github/ github/
COPY packages/engine/ engine/
COPY packages/law-model/ law-model/
COPY packages/pipeline/ pipeline/
COPY packages/harvester/ harvester/
COPY packages/shared/ shared/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build (deps cached separately from source). Everything up to the
# final `cargo build` layer is identical for all BIN variants, so the
# expensive `cargo chef cook` layer is shared between the three images.
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
ARG BIN=regelrecht-pipeline-api
# Invalidate cargo's fingerprint cache so it recompiles with real source
# instead of using the stub binary from cargo-chef cook.
RUN find . -name '*.rs' -exec touch {} + && \
    cargo build --release --bin "${BIN}"

# Stage 4: Shared runtime base
FROM alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS runtime-base
RUN apk add --no-cache ca-certificates && \
    addgroup -S app && adduser -S app -G app

# Runtime: harvester-worker
# tini runs as PID 1 so reparented git grandchildren (merge-base,
# git-remote-https, the askpass helper) get reaped instead of piling up as
# zombies against the container's low pids limit — the accumulation that wedges
# the worker into a "cannot fork() / Resource temporarily unavailable" loop on
# long harvest runs.
FROM runtime-base AS harvester-worker
RUN apk add --no-cache git tini

COPY --from=builder /build/target/release/regelrecht-harvest-worker /usr/local/bin/
COPY packages/pipeline/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

USER app
EXPOSE 8000

ENV REGULATION_REPO_PATH=/tmp/regulation-repo
ENV REGULATION_OUTPUT_BASE=regulation/nl

ENTRYPOINT ["/sbin/tini", "--", "entrypoint.sh"]

# Runtime: enrich-worker
# tini runs as PID 1 so reparented subprocesses are reaped instead of leaking as
# zombies. The enrich worker kills the LLM CLI (claude/opencode) on timeout,
# which orphans its child processes — without an init those accumulate against
# the container's low pids limit and eventually cause fork()/EAGAIN failures.
FROM runtime-base AS enrich-worker
# `pandoc-cli` (docx/html/…) and `poppler-utils` (pdftotext) give the
# document-convert agent real, pre-installed converters. Without them the agent's
# only route on this network-restricted Alpine image is to install a tool at
# runtime (e.g. `pip install pypandoc-binary`) — but there is no pip/python here
# and egress is locked down, so it flails and produces no output. Baking the
# tools in makes docx/PDF conversion deterministic and needs no runtime egress.
RUN apk add --no-cache git nodejs npm tini pandoc-cli poppler-utils

# Install opencode and claude CLI globally (pinned versions for reproducibility)
RUN npm install -g --ignore-scripts opencode-ai@1.2.25 @anthropic-ai/claude-code@2.1.74 && \
    npm cache clean --force

# Copy opencode provider config (VLAM) — the API key is injected at runtime
COPY packages/pipeline/opencode.json /etc/opencode/opencode.json

# Install opencode provider plugin in a shared location
COPY packages/pipeline/opencode-plugins-package.json /opt/opencode-plugins/package.json
COPY packages/pipeline/opencode-plugins-package-lock.json /opt/opencode-plugins/package-lock.json
RUN cd /opt/opencode-plugins && \
    npm ci --ignore-scripts && \
    npm cache clean --force

COPY --from=builder /build/target/release/regelrecht-enrich-worker /usr/local/bin/
COPY packages/pipeline/entrypoint-enrich.sh /usr/local/bin/entrypoint-enrich.sh
RUN chmod +x /usr/local/bin/entrypoint-enrich.sh

# Bake skill files into the image so the LLM can read them at runtime.
# ensure_skills() in enrich.rs symlinks these into each corpus checkout.
COPY .claude/skills/law-generate/   /opt/skills/.claude/skills/law-generate/
COPY .claude/skills/law-mvt-research/ /opt/skills/.claude/skills/law-mvt-research/
COPY .claude/skills/law-reverse-validate/ /opt/skills/.claude/skills/law-reverse-validate/

# The schema the agent writes against. `link_schema` in enrich.rs looks for
# `<SKILLS_DIR>/schema` and symlinks it into each checkout; without it the
# function takes its "the agent will work without the contract" branch on
# every production run, and an agent that goes looking, finds nothing and
# invents fields the schema forbids. The builder stage has its own copy at
# /schema for `include_str!`; that one does not survive into this image.
COPY schema/ /opt/skills/schema/

USER app
EXPOSE 8000

ENV REGULATION_REPO_PATH=/tmp/regulation-repo
ENV REGULATION_OUTPUT_BASE=regulation/nl

ENTRYPOINT ["/sbin/tini", "--", "entrypoint-enrich.sh"]

# Runtime: pipeline-api (last stage, so also the default build target)
FROM runtime-base AS pipeline-api
COPY --from=builder /build/target/release/regelrecht-pipeline-api /usr/local/bin/

USER app
EXPOSE 8000

CMD ["regelrecht-pipeline-api"]
