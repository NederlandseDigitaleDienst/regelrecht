#!/usr/bin/env bash
# Covers script/schema-diff-comment.sh with a `gh` stub that serves a list of
# comments and records every write, so each case can check which call was made.
#
# What matters most here: a push updates the one comment instead of adding a
# second, a comment by someone else that happens to carry the marker is never
# touched, and a pull request without a schema version gets no comment.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/schema-diff-comment.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp:?}"' EXIT

pass=0
fail=0

# The stub applies the `--jq` filter to comments.json with the real jq, logs
# each write as "<METHOD> <endpoint>" in calls.log, and keeps the last body.
cat >"$tmp/gh" <<'STUB'
#!/usr/bin/env bash
method=GET endpoint="" filter="."
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
    case "${args[$i]}" in
    api) ;;
    -X) i=$((i + 1)); method="${args[$i]}" ;;
    --jq) i=$((i + 1)); filter="${args[$i]}" ;;
    --input | --paginate) [ "${args[$i]}" = --input ] && i=$((i + 1)) ;;
    *) [ -z "$endpoint" ] && endpoint="${args[$i]}" ;;
    esac
done
if [ "$method" = GET ]; then
    [ -f "$FIXTURES/fail-get" ] && { echo "HTTP 502" >&2; exit 1; }
    jq -r "$filter" <"$FIXTURES/comments.json"
    exit 0
fi
[ -f "$FIXTURES/fail-write" ] && { echo "HTTP 403: Resource not accessible" >&2; exit 1; }
echo "$method $endpoint" >>"$FIXTURES/calls.log"
[ "$method" != DELETE ] && jq -j '.body' >"$FIXTURES/body.txt"
echo '{}'
STUB
chmod +x "$tmp/gh"

marker='<!-- schema-diff -->'
bot='github-actions[bot]'

# $1 name, $2 comments JSON, $3 body ("" = empty file), $4 expected calls
# (newline-separated, "" = none), $5 expected exit status (default 0)
check() {
    local name="$1" comments="$2" body="$3" expected="$4" status_expected="${5:-0}"
    rm -f "$tmp/calls.log" "$tmp/body.txt"
    printf '%s' "$comments" >"$tmp/comments.json"
    printf '%s' "$body" >"$tmp/body.md"

    local out status
    out=$(PATH="$tmp:$PATH" FIXTURES="$tmp" REPO=o/r PR_NUMBER=7 BODY_FILE="$tmp/body.md" \
        bash "$script" 2>&1)
    status=$?

    local calls=''
    [ -f "$tmp/calls.log" ] && calls=$(cat "$tmp/calls.log")

    if [ "$calls" != "$expected" ] || [ "$status" != "$status_expected" ]; then
        echo "FAIL: $name"
        echo "    expected calls: ${expected:-<none>} (exit $status_expected)"
        echo "    got calls:      ${calls:-<none>} (exit $status)"
        sed 's/^/    | /' <<<"$out"
        fail=$((fail + 1))
        return
    fi
    if [ -n "$body" ] && [ "$status" = 0 ] && ! diff -q "$tmp/body.md" "$tmp/body.txt" >/dev/null; then
        echo "FAIL: $name — the posted body differs from BODY_FILE"
        fail=$((fail + 1))
        return
    fi
    echo "ok: $name"
    pass=$((pass + 1))
}

body="${marker}
## Schemadiff
Een \"aanhaling\", een backslash \\ en \`code\`."

ours=$(jq -n --arg m "$marker" --arg b "$bot" '[{id: 11, user: {login: $b}, body: ($m + "\nold")}]')
other=$(jq -n --arg m "$marker" '[{id: 12, user: {login: "someone"}, body: ($m + "\ncopied")}]')
unrelated=$(jq -n --arg b "$bot" '[{id: 13, user: {login: $b}, body: "some other bot comment"}]')
two=$(jq -n --arg m "$marker" --arg b "$bot" \
    '[{id: 11, user: {login: $b}, body: ($m + "\na")}, {id: 14, user: {login: $b}, body: ($m + "\nb")}]')

check "a first run posts a new comment" '[]' "$body" "POST repos/o/r/issues/7/comments"
check "a later push updates the existing comment" "$ours" "$body" "PATCH repos/o/r/issues/comments/11"
check "a comment by someone else with the marker is left alone" "$other" "$body" \
    "POST repos/o/r/issues/7/comments"
check "a bot comment without the marker is left alone" "$unrelated" "$body" \
    "POST repos/o/r/issues/7/comments"
check "a duplicate from a race is removed, the first one updated" "$two" "$body" \
    "DELETE repos/o/r/issues/comments/14
PATCH repos/o/r/issues/comments/11"
check "no schema version and no comment: nothing is posted" '[]' "" ""
check "no schema version any more: the old comment is deleted" "$ours" "" \
    "DELETE repos/o/r/issues/comments/11"

touch "$tmp/fail-get"
check "comments that cannot be read fail the job" '[]' "$body" "" 1
rm -f "$tmp/fail-get"
touch "$tmp/fail-write"
check "a comment that cannot be posted fails the job" '[]' "$body" "" 1
rm -f "$tmp/fail-write"

echo
echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
