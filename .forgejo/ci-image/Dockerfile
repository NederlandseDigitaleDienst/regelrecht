# The job image for the Forgejo CI in .forgejo/workflows/ci.yml.
#
# The `default` runner runs a job as uid 1000 without root, so a job cannot
# install anything: every tool the checks need is in here. ci-image.yml builds
# this on the `vm` runner and pushes it as
# code.digitaledienst.overheid.nl/regelrecht/fieldlab-ci:main.
#
# Keep the Rust version equal to rust-toolchain.toml and the mold pin equal to
# .github/actions/setup-mold/action.yml.
FROM rust:1.96.0-bookworm

RUN rustup component add rustfmt clippy \
 && rustup target add wasm32-unknown-unknown

ARG MOLD_VERSION=2.42.1
ARG MOLD_SHA256=6ff270c9bf07d2bec5c98aa324eb7c4daf6a1a4d815c05ff1708049616047855
RUN curl -fsSL --retry 5 -o /tmp/mold.tar.gz \
      "https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-x86_64-linux.tar.gz" \
 && echo "${MOLD_SHA256}  /tmp/mold.tar.gz" | sha256sum -c - \
 && tar -C /usr/local --strip-components=1 --no-overwrite-dir -xzf /tmp/mold.tar.gz \
 && rm /tmp/mold.tar.gz \
 && mold --version

RUN cargo install --locked just@1.43.0 \
 && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

# Node 24 for actions/checkout (a JavaScript action) and the frontend checks.
COPY --from=node:24-bookworm /usr/local/bin/node /usr/local/bin/node
COPY --from=node:24-bookworm /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
 && node --version && npm --version

# The job runs as uid 1000 with HOME=/tmp. cargo writes its registry cache
# under CARGO_HOME, so that has to be writable for that user.
RUN chmod -R a+rwX /usr/local/cargo /usr/local/rustup
