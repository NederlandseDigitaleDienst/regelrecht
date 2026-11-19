#!/usr/bin/env bash
# Covers every path that decides green or red in script/await-zad-health.sh,
# with a `zad` stub on PATH. Without this test a check that lets everything
# through cannot be told apart from one that works.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/await-zad-health.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# The stub answers call N with the Nth argument given here, and repeats the
# last one after that. An answer of `EXIT` makes that call fail without output.
stub_zad() {
    rm -f "$tmp"/answer-* "$tmp/calls"
    local n=0 answer
    for answer in "$@"; do
        n=$((n + 1))
        printf '%s' "$answer" >"$tmp/answer-$n"
    done
    echo "$n" >"$tmp/last"
    cat >"$tmp/zad" <<STUB
#!/usr/bin/env bash
echo "\$*" >>"$tmp/args"
n=\$(( \$(cat "$tmp/calls" 2>/dev/null || echo 0) + 1 ))
echo "\$n" >"$tmp/calls"
last=\$(cat "$tmp/last")
[ "\$n" -gt "\$last" ] && n=\$last
answer=\$(cat "$tmp/answer-\$n")
if [ "\$answer" = "EXIT" ]; then
    echo "zad: boom" >&2
    exit 2
fi
printf '%s' "\$answer"
STUB
    chmod +x "$tmp/zad"
}

want='[{"name":"editor","image":"ghcr.io/example-org/editor:sha-new"},{"name":"docs","image":"ghcr.io/example-org/docs:sha-new"}]'

# A `deployment describe` document. $1 = status, $2 = jq filter applied on top.
doc() {
    jq -c "${2:-.}" <<JSON
{
  "deployment": "example",
  "components": [
    {"name": "editor", "image": "ghcr.io/example-org/editor:sha-new"},
    {"name": "docs", "image": "ghcr.io/example-org/docs:sha-new"},
    {"name": "worker", "image": "ghcr.io/example-org/worker:sha-old"}
  ],
  "pending_rollout": {"count": 0},
  "status": "$1",
  "sync_revision": "aaa",
  "rolled_out_revision": "aaa",
  "running_sync": null,
  "errors": [],
  "deviations": []
}
JSON
}

pull_error='.errors = [{"category":"ImagePull","resource":"Pod/docs-abc","message":"Back-off pulling image"}]'

# name, expected exit, needle (may be empty), attempts, components, answers...
check() {
    local name="$1" expect="$2" needle="$3" attempts="$4" components="$5"
    shift 5
    stub_zad "$@"
    local out st
    out="$(PATH="$tmp:$PATH" DEPLOYMENT=example COMPONENTS="$components" ATTEMPTS="$attempts" DELAY=0 bash "$gate" 2>&1)"
    st=$?
    last_out="$out"

    if [ "$st" -ne "$expect" ]; then
        echo "FAIL: $name: exit $st, expected $expect"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    if [ -n "$needle" ] && ! grep -qF -- "$needle" <<<"$out"; then
        echo "FAIL: $name: '$needle' not in the output"
        echo "$out" | sed 's/^/    /'
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

expect_calls() { # name, number
    local got
    got=$(cat "$tmp/calls" 2>/dev/null || echo 0)
    if [ "$got" -eq "$2" ]; then
        echo "ok: $1"
        pass=$((pass + 1))
    else
        echo "FAIL: $1: $got calls, expected $2"
        fail=$((fail + 1))
    fi
}

check "healthy on the first answer is green" 0 "Healthy, and ZAD has the images of this run" 3 "$want" \
    "$(doc Healthy)"
expect_calls "a healthy answer ends the waiting" 1

rm -f "$tmp/args"
check "the deployment is asked by name, as JSON" 0 "" 1 "$want" "$(doc Healthy)"
if grep -qxF -- "--output json deployment describe example" "$tmp/args"; then
    echo "ok: the call names the deployment"
    pass=$((pass + 1))
else
    echo "FAIL: unexpected call: $(cat "$tmp/args")"
    fail=$((fail + 1))
fi

# The case this script exists for: the pull fails at first and recovers.
check "a failed pull that recovers is green" 0 "attempt 3" 5 "$want" \
    "$(doc Degraded "$pull_error")" "$(doc Progressing)" "$(doc Healthy)"
expect_calls "it asked until the deployment was healthy" 3

check "a pull that keeps failing is red" 1 "ImagePull Pod/docs-abc: Back-off pulling image" 3 "$want" \
    "$(doc Degraded "$pull_error")"
expect_calls "it used every attempt" 3
check "the red outcome names the status" 1 "status is Degraded" 2 "$want" \
    "$(doc Degraded "$pull_error")"

for status in Progressing Pending Missing Suspended Disabled Unavailable Unknown; do
    check "status $status is not healthy" 1 "status is $status" 1 "$want" "$(doc "$status")"
done
check "a status this script does not know is not healthy" 1 "status is Splendid" 1 "$want" "$(doc Splendid)"
check "a missing status is not healthy" 1 "status is missing" 1 "$want" "$(doc Healthy 'del(.status)')"

# Healthy says nothing when it is about the previous images.
check "healthy on an older image is red" 1 "does not have the image of this run for: docs" 1 "$want" \
    "$(doc Healthy '.components[1].image = "ghcr.io/example-org/docs:sha-old"')"
check "a component ZAD does not list is red" 1 "does not have the image of this run for: docs" 1 "$want" \
    "$(doc Healthy 'del(.components[1])')"
check "a component this run did not send is left alone" 0 "" 1 "$want" "$(doc Healthy)"

# The count is the project's: a change saved for a preview is not production's.
check "a change waiting elsewhere in the project is left alone" 0 "" 1 "$want" \
    "$(doc Healthy '.pending_rollout.count = 2')"
check "a running sync is red" 1 "a sync is still running" 1 "$want" \
    "$(doc Healthy '.running_sync = {"phase":"Running"}')"
check "a revision that is not rolled out is red" 1 "newest revision is not rolled out" 1 "$want" \
    "$(doc Healthy '.rolled_out_revision = "bbb"')"

# zad-cli before v0.13 has neither field; their absence must not block.
check "an older CLI without the sync fields is green" 0 "" 1 "$want" \
    "$(doc Healthy 'del(.rolled_out_revision, .running_sync, .pending_rollout)')"

# OutOfSync outranks Progressing in the API's status, so it can be a rollout
# that is still under way. Only Healthy passes.
check "out of sync with only leftovers is red" 1 "Job/old-migrate: wacht op opruiming" 1 "$want" \
    "$(doc OutOfSync '.deviations = [{"kind":"Job","resource":"Job/old-migrate","reason":"wacht op opruiming"}]')"
check "out of sync without anything listed is red" 1 "status is OutOfSync" 1 "$want" "$(doc OutOfSync)"
check "out of sync with an error is red" 1 "status is OutOfSync" 1 "$want" \
    "$(doc OutOfSync "$pull_error")"

# Healthy with errors contradicts the API's own description; not provably healthy.
check "healthy with errors listed is red" 1 "Healthy while ZAD reports errors" 1 "$want" \
    "$(doc Healthy "$pull_error")"
check "healthy with errors lists them" 1 "ImagePull Pod/docs-abc" 1 "$want" "$(doc Healthy "$pull_error")"

# An error list of a shape this script cannot read must not count as empty.
check "errors that are strings are red" 1 "no readable list of errors" 1 "$want" \
    "$(doc Healthy '.errors = ["ImagePull docs: back-off"]')"
check "errors that are no list are red" 1 "no readable list of errors" 1 "$want" \
    "$(doc Healthy '.errors = "boom"')"
check "a missing error list is red" 1 "no readable list of errors" 1 "$want" \
    "$(doc Healthy 'del(.errors)')"
check "deviations that are no list are red" 1 "no readable list of errors" 1 "$want" \
    "$(doc Healthy '.deviations = "boom"')"
check "components that are no list are red" 1 "does not have the image of this run" 1 "$want" \
    "$(doc Healthy '.components = "boom"')"

# The cause of a failing CLI is in what it writes, and that reaches the log.
check "a failing CLI shows what it wrote" 1 "zad: zad: boom" 1 "$want" EXIT
check "an unreadable last answer keeps the errors of the one before" 1 "ImagePull Pod/docs-abc" 2 "$want" \
    "$(doc Degraded "$pull_error")" EXIT

check "a CLI that keeps failing is red" 1 "no readable answer" 2 "$want" EXIT
check "a CLI that fails once is tried again" 0 "attempt 2" 3 "$want" EXIT "$(doc Healthy)"
check "an answer that is not JSON is red" 1 "no readable answer" 1 "$want" "<html>502</html>"
check "an answer that is not an object is red" 1 "not a JSON object" 1 "$want" '[]'
check "an empty answer is red" 1 "no readable answer" 1 "$want" ""

check "an empty component list is red" 1 "nothing to compare" 1 '[]' "$(doc Healthy)"
check "a component list that is no array is red" 1 "nothing to compare" 1 '{"editor":"x"}' "$(doc Healthy)"
check "a component without an image is red" 1 "nothing to compare" 1 '[{"name":"editor"}]' "$(doc Healthy)"
check "a component list that is not JSON is red" 1 "nothing to compare" 1 'editor' "$(doc Healthy)"

# A cluster message is foreign text. A line break in it must not let the rest
# start a line, where the runner would read `::add-mask::` as a command.
hostile='.errors = [{"category":"CrashLoop","resource":"Pod/x","message":"boom\n::add-mask::secret\r\n::error::forged"}]'
check "a hostile cluster message is red" 1 "CrashLoop Pod/x: boom" 1 "$want" "$(doc Degraded "$hostile")"
if grep -qE '^[[:space:]]*::(add-mask|error)::(secret|forged)' <<<"$last_out"; then
    echo "FAIL: a cluster message started a line with a workflow command"
    fail=$((fail + 1))
else
    echo "ok: a cluster message cannot start a workflow command"
    pass=$((pass + 1))
fi
legacy='.errors = [{"category":"CrashLoop","resource":"Pod/x","message":"boom ##[add-mask]secret"}]'
check "a legacy workflow command in a message is red" 1 "## [add-mask]secret" 1 "$want" "$(doc Degraded "$legacy")"
if grep -qF '##[' <<<"$last_out"; then
    echo "FAIL: a cluster message kept a legacy workflow command"
    fail=$((fail + 1))
else
    echo "ok: a legacy workflow command is taken apart"
    pass=$((pass + 1))
fi
hostile_status='.status = "Bad\n::error::forged"'
check "a status with a line break is red" 1 "" 1 "$want" "$(doc Healthy "$hostile_status")"
if grep -qE '^[[:space:]]*::error::forged' <<<"$last_out"; then
    echo "FAIL: a status started a line with a workflow command"
    fail=$((fail + 1))
else
    echo "ok: a status cannot start a workflow command"
    pass=$((pass + 1))
fi

missing="$(PATH="$tmp:$PATH" COMPONENTS="$want" bash "$gate" 2>&1)"
if [ $? -ne 0 ] && grep -qF "DEPLOYMENT is required" <<<"$missing"; then
    echo "ok: a missing deployment name is refused"
    pass=$((pass + 1))
else
    echo "FAIL: a missing deployment name was accepted"
    fail=$((fail + 1))
fi

echo
echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
