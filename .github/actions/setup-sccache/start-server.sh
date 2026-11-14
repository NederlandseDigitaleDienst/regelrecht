#!/usr/bin/env bash
# Start the sccache server and turn on RUSTC_WRAPPER only when it came up.
#
# sccache checks its cache storage when the server starts (a read of
# `.sccache_check`). When that read fails with anything but a rate limit, the
# server refuses to start, and every `sccache rustc ...` call fails with
# "Server startup failed: cache storage failed to read". cargo calls the
# wrapper for its `rustc -vV` probe before it compiles anything, so a job with
# RUSTC_WRAPPER=sccache fails before a single test runs. The GitHub Actions
# cache returning 503 ServerBusy ("Egress is over the account limit") is the
# case that produced this script.
#
# SCCACHE_IGNORE_SERVER_IO_ERROR does not cover it: sccache 0.19 reads that
# variable only when a running server drops a compile response, not when the
# server fails to start. So the decision is made here, once per job: start the
# server, and only on success export RUSTC_WRAPPER. On failure the job builds
# with plain rustc and says so in a warning.
#
# The action runs it with SCCACHE_GHA_ENABLED and SCCACHE_IDLE_TIMEOUT set; it
# writes to GITHUB_ENV. SCCACHE_BIN overrides the sccache binary, for testing.
set -uo pipefail

: "${GITHUB_ENV:?GITHUB_ENV must point at the step environment file}"
sccache_bin="${SCCACHE_BIN:-sccache}"

# Incremental compilation must be off for sccache to cache anything; without
# sccache it only costs disk on a fresh runner, so it is off either way.
echo "CARGO_INCREMENTAL=0" >>"$GITHUB_ENV"

if output=$("$sccache_bin" --start-server 2>&1); then
  echo "$output"
  {
    echo "RUSTC_WRAPPER=sccache"
    echo "SCCACHE_GHA_ENABLED=true"
    # Keep the server for the whole job. A server that stops after the default
    # 600 s idle timeout restarts on the next compile and runs the storage
    # check again, which would undo the decision made here.
    echo "SCCACHE_IDLE_TIMEOUT=0"
  } >>"$GITHUB_ENV"
  exit 0
fi

echo "$output" >&2
echo "::warning title=sccache::build cache unreachable, building without sccache"
# RUSTC_WRAPPER stays unset. The action's post step still runs
# `sccache --show-stats`, which starts a server of its own; with the GHA
# backend off it uses the local disk and cannot fail on the same storage.
echo "SCCACHE_GHA_ENABLED=false" >>"$GITHUB_ENV"
exit 0
