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
# server fails to start. SCCACHE_SKIP_CACHE_CHECK would let the server start,
# but then every compile waits on a storage that answers each request with an
# error, and nothing on the run says the cache is gone. So the decision is
# made here, once per job: start the server, and only on success export
# RUSTC_WRAPPER. On failure the job builds with plain rustc and says so in a
# warning.
#
# action.yml runs this with SCCACHE_GHA_ENABLED=true and SCCACHE_IDLE_TIMEOUT=0
# in the step environment, so the server started here keeps those. The script
# writes to GITHUB_ENV. SCCACHE_BIN overrides the sccache binary for
# start-server.test.sh.
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
    # Only matters if the server dies and a compile restarts it: the new
    # server then uses the same backend and also never stops on idle.
    echo "SCCACHE_GHA_ENABLED=true"
    echo "SCCACHE_IDLE_TIMEOUT=0"
  } >>"$GITHUB_ENV"
  exit 0
fi

# RUSTC_WRAPPER stays unset, so cargo calls rustc directly. The post step of
# sccache-action runs `sccache --show-stats`, which finds no server and prints
# empty stats without touching the storage.
echo "$output" >&2
echo "::warning title=sccache::build cache unreachable, building without sccache"
exit 0
