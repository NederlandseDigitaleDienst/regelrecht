#!/usr/bin/env bash
# Wake a sleeping ZAD preview deployment and wait until it is awake.
#
#   script/zad-wake.sh pr1704
#
# Previews (`pr*`) on ZAD go to sleep 48 hours after their last deploy. A
# smoketest against a sleeping preview gets the waker page or a 503 and reports
# that as a failure of the change under test. Call this first.
#
# zad-cli has no sleep-mode command, so this talks to the Operations Manager
# API directly. The environment variable names match zad-cli's:
#   ZAD_API_KEY     project API key (falls back to RIG_API_KEY)
#   ZAD_PROJECT_ID  defaults to regel-k4c
#   ZAD_API_URL     defaults to the RIG Operations Manager
#   WAIT_SECONDS    how long to wait for the deployment (default 600)
#   POLL_INTERVAL   seconds between status polls (default 10)
#
# Exit status: 0 awake (or sleep-mode does not apply), 1 not awake or the API
# could not be reached, 2 usage error.
set -uo pipefail

deployment="${1:-}"
if [ -z "$deployment" ]; then
    echo "usage: $0 <deployment>   (for example pr1704)" >&2
    exit 2
fi

key="${ZAD_API_KEY:-${RIG_API_KEY:-}}"
if [ -z "$key" ]; then
    echo "error: set ZAD_API_KEY (or RIG_API_KEY) to the project API key" >&2
    exit 2
fi

project="${ZAD_PROJECT_ID:-regel-k4c}"
base="${ZAD_API_URL:-https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl}"
base="${base%/}"
wait_seconds="${WAIT_SECONDS:-600}"
poll_interval="${POLL_INTERVAL:-10}"
url="$base/api/sleep-mode/$project/$deployment"

# Prints "<http code> <body>" on one line. A transport failure yields code 000.
call() {
    local method="$1" path="$2" out code
    out="$(curl -sS --max-time 30 -X "$method" -H "X-API-Key: $key" \
        -w '\n%{http_code}' "$url/$path" 2>&1)"
    code="${out##*$'\n'}"
    printf '%s %s\n' "$code" "${out%$'\n'*}" | tr -d '\r' | tr '\n' ' ' | sed 's/ *$//'
    echo
}

field() { # json, field name
    jq -r --arg f "$2" '.[$f] // empty' <<<"$1" 2>/dev/null
}

response="$(call POST wake)"
code="${response%% *}"
body="${response#* }"
case "$code" in
    200) echo "wake: no-op for $deployment, already awake or waking ($body)" ;;
    202) echo "wake: started for $deployment ($body)" ;;
    *)
        echo "error: wake request for $deployment returned HTTP $code: $body" >&2
        exit 1
        ;;
esac

# `state` is the waker page's poll contract (starting | ready). An awake
# deployment has been seen to report `starting` indefinitely, so `sleep_state`
# decides as well: `awake` means ZAD has the deployment running again.
deadline=$(($(date +%s) + wait_seconds))
while :; do
    response="$(call GET status)"
    code="${response%% *}"
    body="${response#* }"
    if [ "$code" = "200" ]; then
        state="$(field "$body" state)"
        sleep_state="$(field "$body" sleep_state)"
        if [ "$state" = "ready" ] || [ "$sleep_state" = "awake" ]; then
            echo "ready: $deployment is awake (state=$state, sleep_state=$sleep_state)"
            exit 0
        fi
        if [ "$sleep_state" = "disabled" ]; then
            echo "ready: sleep-mode does not apply to $deployment, nothing to wake"
            exit 0
        fi
        if [ -z "$sleep_state" ]; then
            echo "error: status for $deployment has no sleep_state: $body" >&2
            exit 1
        fi
        echo "waiting: state=$state, sleep_state=$sleep_state"
    else
        echo "waiting: status returned HTTP $code: $body"
    fi
    if [ "$(date +%s)" -ge "$deadline" ]; then
        echo "error: $deployment not awake after ${wait_seconds}s; last status HTTP $code: $body" >&2
        exit 1
    fi
    sleep "$poll_interval"
done
