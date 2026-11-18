#!/usr/bin/env bash
# Covers every path in script/zad-wake.sh that decides between exit 0 and
# exit 1, with a `curl` stub on PATH. A wake helper that always reports "ready"
# would send a smoketest to a sleeping preview, which is the failure it exists
# to prevent.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/zad-wake.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp:?}"' EXIT

pass=0
fail=0

# The stub answers POST .../wake with $tmp/wake and each GET .../status with
# the next line of $tmp/status (the last line repeats). Each response is
# "<http code> <body>"; code 000 makes curl fail as on a transport error.
cat >"$tmp/curl" <<'STUB'
#!/usr/bin/env bash
dir="$(dirname "$0")"
method=GET url=""
while [ $# -gt 0 ]; do
    case "$1" in
        -X) method="$2"; shift 2 ;;
        -H) echo "$2" >>"$dir/headers"; shift 2 ;;
        -w | --max-time) shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
echo "$method $url" >>"$dir/calls"
case "$method $url" in
    "POST "*/wake) line="$(cat "$dir/wake")" ;;
    "GET "*/status)
        n=$(($(cat "$dir/polls" 2>/dev/null || echo 0) + 1))
        echo "$n" >"$dir/polls"
        total=$(wc -l <"$dir/status")
        [ "$n" -gt "$total" ] && n=$total
        line="$(sed -n "${n}p" "$dir/status")"
        ;;
    *) echo "stub: unexpected call: $method $url" >&2; exit 64 ;;
esac
code="${line%% *}"
if [ "$code" = "000" ]; then
    echo "curl: (7) Failed to connect" >&2
    printf '\n000'
    exit 7
fi
printf '%s\n%s' "${line#* }" "$code"
STUB
chmod +x "$tmp/curl"

check() { # name, expected exit, wake response, status responses (newline-separated), needle
    local name="$1" want="$2" wake="$3" statuses="$4" needle="${5:-}"
    printf '%s\n' "$wake" >"$tmp/wake"
    printf '%s\n' "$statuses" >"$tmp/status"
    rm -f "$tmp/polls" "$tmp/calls" "$tmp/headers"
    local out status
    out="$(
        PATH="$tmp:$PATH" ZAD_API_KEY=test-key ZAD_PROJECT_ID=proj \
            ZAD_API_URL=http://zad.example WAIT_SECONDS="${WAIT:-5}" \
            POLL_INTERVAL=0 bash "$script" pr42 2>&1
    )"
    status=$?
    if [ "$status" -ne "$want" ]; then
        echo "FAIL: $name: exit $status, expected $want"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    if [ -n "$needle" ] && ! grep -qF "$needle" <<<"$out"; then
        echo "FAIL: $name: '$needle' not in output"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

check "a started wake that becomes ready exits 0" 0 \
    '202 {"state":"waking","sleep_state":"waking"}' \
    '200 {"state":"starting","sleep_state":"waking"}
200 {"state":"ready","sleep_state":"awake"}' "ready: pr42 is awake"

# An awake deployment has been seen to report state=starting indefinitely;
# waiting for `ready` alone would then hang until the timeout.
check "awake with state=starting counts as awake" 0 \
    '200 {"state":"awake","sleep_state":"awake"}' \
    '200 {"state":"starting","sleep_state":"awake"}' "wake: no-op"

check "sleep-mode disabled means nothing to wake" 0 \
    '200 {"state":"disabled","sleep_state":"disabled"}' \
    '200 {"state":"starting","sleep_state":"disabled"}' "sleep-mode does not apply"

WAIT=0 check "a deployment that stays waking times out red" 1 \
    '202 {"state":"waking","sleep_state":"waking"}' \
    '200 {"state":"starting","sleep_state":"waking"}' "not awake after"

check "an unknown deployment is red" 1 \
    '404 {"detail":"Deployment not found"}' \
    '404 {"detail":"Deployment not found"}' "HTTP 404"

check "a rejected API key is red" 1 \
    '401 {"detail":"Unauthorized"}' \
    '401 {"detail":"Unauthorized"}' "HTTP 401"

check "an unreachable API is red" 1 \
    '000 ' '000 ' "HTTP 000"

# A 200 without sleep_state would otherwise loop until the timeout as if the
# deployment were still waking.
check "a status without sleep_state is red" 1 \
    '202 {"state":"waking","sleep_state":"waking"}' \
    '200 {"detail":"something else"}' "has no sleep_state"

check "a failing status poll is retried" 0 \
    '202 {"state":"waking","sleep_state":"waking"}' \
    '503 upstream
200 {"state":"ready","sleep_state":"awake"}' "ready: pr42"

# The request has to reach the right deployment with the key.
check "calls the wake and status endpoints of the deployment" 0 \
    '202 {"state":"waking","sleep_state":"waking"}' \
    '200 {"state":"ready","sleep_state":"awake"}'
if grep -qx "POST http://zad.example/api/sleep-mode/proj/pr42/wake" "$tmp/calls" &&
    grep -qx "GET http://zad.example/api/sleep-mode/proj/pr42/status" "$tmp/calls" &&
    grep -qx "X-API-Key: test-key" "$tmp/headers"; then
    echo "ok: request paths and key"
    pass=$((pass + 1))
else
    echo "FAIL: request paths and key"
    sed 's/^/    /' "$tmp/calls" "$tmp/headers"
    fail=$((fail + 1))
fi

out="$(PATH="$tmp:$PATH" ZAD_API_KEY=k bash "$script" 2>&1)"
if [ $? -eq 2 ] && grep -qF "usage:" <<<"$out"; then
    echo "ok: no deployment is a usage error"
    pass=$((pass + 1))
else
    echo "FAIL: no deployment is a usage error"
    fail=$((fail + 1))
fi

out="$(env -u ZAD_API_KEY -u RIG_API_KEY PATH="$tmp:$PATH" bash "$script" pr42 2>&1)"
if [ $? -eq 2 ] && grep -qF "ZAD_API_KEY" <<<"$out"; then
    echo "ok: no API key is a usage error"
    pass=$((pass + 1))
else
    echo "FAIL: no API key is a usage error"
    fail=$((fail + 1))
fi

echo
echo "passed: $pass, failed: $fail"
[ "$fail" -eq 0 ]
