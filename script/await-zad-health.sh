#!/usr/bin/env bash
# Waits until ZAD reports a deployment as healthy on the images this run sent.
#
# The deploy step turns green as soon as ZAD accepts the rollout. ZAD can
# accept one and still report a component as unhealthy, for instance when the
# registry did not deliver an image, and zad-cli exits 0 on that (#1651). The
# cluster keeps pulling by itself, so the answer at the end of the task is
# often not the final one. This script asks again until the deployment is
# healthy or the window closes.
#
# It asks the platform and not the application: workers and other components
# without a URL are covered, and nothing has to be built into an image.
#
# The status is that of the whole deployment. A component this run did not
# touch and that is broken keeps the deployment from Healthy, and so fails
# this check too.
set -uo pipefail

: "${DEPLOYMENT:?DEPLOYMENT is required (the ZAD deployment name)}"
: "${COMPONENTS:?COMPONENTS is required (JSON array of {name, image})}"

# 20 x 15 seconds: the platform's own message on a failed pull asks to report
# it when it lasts longer than a few minutes.
ATTEMPTS="${ATTEMPTS:-20}"
DELAY="${DELAY:-15}"

fail() {
    echo "::error title=ZAD health::$1"
    exit 1
}

if ! jq -e 'type == "array" and length > 0 and all(.[]; type == "object" and (.name | type == "string") and (.image | type == "string"))' \
    >/dev/null 2>&1 <<<"$COMPONENTS"; then
    fail "COMPONENTS is not a non-empty JSON array of {name, image}, so there is nothing to compare the deployment against."
fi

# Reads one `deployment describe` document and prints a verdict:
#   {"state": "healthy" | "waiting", "reason": "...", "errors": [...], "deviations": [...]}
#
# Everything that is not provably healthy is "waiting", so an answer of a shape
# this script does not know can never pass as healthy.
#
# Only Healthy passes. The API collapses sync and health into one status in
# which OutOfSync outranks Progressing, so an OutOfSync deployment can be in
# the middle of a rollout.
#
# `components` is what ZAD has saved, not what runs; `status` and `errors`
# describe the cluster. What ties the two together is that the deploy step
# before this one waits for the sync of what it saved. `rolled_out_revision`
# and `running_sync` say it outright, but exist only from zad-cli v0.13 on;
# with the older CLI the deploy action installs they are absent.
#
# `pending_rollout` is left out on purpose: it counts the whole project, so a
# change saved for a preview would fail production.
judge() {
    jq -c --argjson want "$COMPONENTS" '
        def text: tostring | gsub("[\r\n]+"; " ") | gsub("##\\["; "## [");
        def objects_or_null: if type == "array" and all(.[]; type == "object") then . else null end;
        if type != "object" then
            {state: "waiting", reason: "the answer is not a JSON object"}
        else
            ((.components | objects_or_null) // [] | map({key: (.name // "" | tostring), value: (.image // "")}) | from_entries) as $have
            | [$want[] | select($have[.name] != .image) | .name] as $stale
            | ((.status // "") | text) as $status
            | (.errors | objects_or_null) as $raw_errors
            | ((.deviations // []) | objects_or_null) as $raw_deviations
            | [$raw_errors // [] | .[]
                | "\(.category // "Unknown" | text) \(.resource // "?" | text): \(.message // "" | text)"] as $errors
            | [$raw_deviations // [] | .[]
                | "\(.resource // "?" | text): \(.reason // "" | text)"] as $deviations
            | (if ($stale | length) > 0 then
                  "ZAD does not have the image of this run for: \($stale | join(", "))"
               elif $raw_errors == null or $raw_deviations == null then
                  "the answer has no readable list of errors"
               elif (.running_sync? != null) then
                  "a sync is still running"
               elif (.rolled_out_revision? != null and .sync_revision? != null
                     and .rolled_out_revision != .sync_revision) then
                  "the newest revision is not rolled out yet"
               elif $status != "Healthy" then
                  "status is \(if $status == "" then "missing" else $status end)"
               elif ($errors | length) > 0 then
                  "status is Healthy while ZAD reports errors"
               else
                  null
               end) as $reason
            | {state: (if $reason == null then "healthy" else "waiting" end),
               reason: $reason, status: $status, errors: $errors, deviations: $deviations}
        end' 2>/dev/null
}

# Foreign text (cluster messages, what the CLI writes) is printed behind a
# prefix, so that no line of it can start with `::` and be read by the runner
# as a workflow command.
print_list() {
    jq -r --arg key "$1" '.[$key][]? | "  - \(.)"' <<<"$2"
}

zad_stderr=$(mktemp)
trap 'rm -f "$zad_stderr"' EXIT

# A describe that hangs must not eat the window: the job would end on its own
# timeout, without a word about why. macOS ships no timeout(1); there the call
# runs without a limit, which only matters for someone running this by hand.
describe() {
    if command -v timeout >/dev/null 2>&1; then
        timeout 60 zad --output json deployment describe "$DEPLOYMENT"
    else
        zad --output json deployment describe "$DEPLOYMENT"
    fi
}

verdict=""
reason="ZAD gave no answer"
for attempt in $(seq 1 "$ATTEMPTS"); do
    if answer=$(describe 2>"$zad_stderr") &&
        candidate=$(judge <<<"$answer") && [ -n "$candidate" ]; then
        verdict="$candidate"
        reason=$(jq -r '.reason // ""' <<<"$verdict")
        if [ "$(jq -r '.state' <<<"$verdict")" = "healthy" ]; then
            status=$(jq -r '.status' <<<"$verdict")
            echo "${DEPLOYMENT}: ${status}, and ZAD has the images of this run (attempt ${attempt})."
            exit 0
        fi
    else
        # The last readable verdict is kept for the report at the end.
        reason="ZAD gave no readable answer"
        # In JSON mode the CLI puts its diagnosis on stdout and the rest on stderr.
        { head -c 2000 "$zad_stderr"; printf '%s\n' "${answer:-}" | head -c 2000; } |
            tr -d '\r' | sed -e '/^[[:space:]]*$/d' -e 's/##\[/## [/g' -e 's/^/    zad: /' | head -n 12
    fi
    echo "  attempt ${attempt}/${ATTEMPTS}: ${reason}"
    [ "$attempt" -lt "$ATTEMPTS" ] && sleep "$DELAY"
done

if [ -n "$verdict" ] && [ "$(jq -r '(.errors + .deviations) | length' <<<"$verdict")" -gt 0 ]; then
    echo "What ZAD reports for ${DEPLOYMENT}:"
    print_list errors "$verdict"
    print_list deviations "$verdict"
fi
# The reason is built from this script's own words plus component names from
# COMPONENTS and a status with its line breaks removed.
fail "${DEPLOYMENT} is not healthy after ${ATTEMPTS} attempts: ${reason}. See the lines above and 'zad deployment describe ${DEPLOYMENT}'."
