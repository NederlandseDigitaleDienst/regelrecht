#!/usr/bin/env bash
# Covers both outcomes of start-server.sh with an sccache stub: a server that
# starts turns RUSTC_WRAPPER on, a server that fails on its storage leaves it
# off and writes the warning. Without this test a script that always exports
# the wrapper looks the same as one that works, until the cache goes down.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/start-server.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp:?}"' EXIT

pass=0
fail=0

check() {
  local name="$1" cond="$2"
  if eval "$cond"; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "FAIL: $name"
  fi
}

# The failing stub prints what sccache 0.19 printed on the runner and exits 2.
cat >"$tmp/sccache-fails" <<'STUB'
#!/usr/bin/env bash
echo "sccache: error: Server startup failed: cache storage failed to read: Unexpected (temporary) at read => ServerBusy" >&2
exit 2
STUB
cat >"$tmp/sccache-starts" <<'STUB'
#!/usr/bin/env bash
echo "sccache: Listening on address 127.0.0.1:4226"
STUB
chmod +x "$tmp/sccache-fails" "$tmp/sccache-starts"

# Server starts: wrapper on, no warning.
: >"$tmp/env-ok"
out=$(GITHUB_ENV="$tmp/env-ok" SCCACHE_BIN="$tmp/sccache-starts" "$script" 2>&1)
code=$?
check "start: exit 0" '[ "$code" -eq 0 ]'
check "start: RUSTC_WRAPPER exported" 'grep -qx "RUSTC_WRAPPER=sccache" "$tmp/env-ok"'
check "start: idle timeout exported" 'grep -qx "SCCACHE_IDLE_TIMEOUT=0" "$tmp/env-ok"'
check "start: incremental off" 'grep -qx "CARGO_INCREMENTAL=0" "$tmp/env-ok"'
check "start: no warning" '! grep -q "::warning" <<<"$out"'

# Storage unreachable: wrapper off, warning, and the step still succeeds.
: >"$tmp/env-fail"
out=$(GITHUB_ENV="$tmp/env-fail" SCCACHE_BIN="$tmp/sccache-fails" "$script" 2>&1)
code=$?
check "fail: exit 0" '[ "$code" -eq 0 ]'
check "fail: no RUSTC_WRAPPER" '! grep -q "^RUSTC_WRAPPER=" "$tmp/env-fail"'
check "fail: incremental off" 'grep -qx "CARGO_INCREMENTAL=0" "$tmp/env-fail"'
check "fail: warning written" 'grep -qx "::warning title=sccache::build cache unreachable, building without sccache" <<<"$out"'
check "fail: sccache error shown" 'grep -q "Server startup failed" <<<"$out"'

# sccache missing from PATH counts as a failed start, not as an error.
: >"$tmp/env-missing"
out=$(GITHUB_ENV="$tmp/env-missing" SCCACHE_BIN="$tmp/does-not-exist" "$script" 2>&1)
code=$?
check "missing: exit 0" '[ "$code" -eq 0 ]'
check "missing: no RUSTC_WRAPPER" '! grep -q "^RUSTC_WRAPPER=" "$tmp/env-missing"'

# Without GITHUB_ENV the script cannot do its job and must say so.
out=$(env -u GITHUB_ENV SCCACHE_BIN="$tmp/sccache-starts" "$script" 2>&1)
code=$?
check "no GITHUB_ENV: exit non-zero" '[ "$code" -ne 0 ]'

echo "start-server.test.sh: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
