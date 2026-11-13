#!/usr/bin/env bash
# Posts the schema diff of script/schema-diff.mjs on a pull request, as one
# comment that each push updates instead of adding another.
#
# The comment is found by the marker line `<!-- schema-diff -->` that the
# script puts first, and by its author: only a comment written by the workflow
# token counts. Selecting on the marker alone would let anyone who comments on
# the pull request with that line have their comment overwritten, or deleted.
#
# An empty BODY_FILE means the pull request adds no schema version (any more):
# an earlier comment is then deleted, because a diff of a version the pull
# request no longer adds says something that is not true.
#
# A GitHub error turns the job red with the error in the log. The job is not a
# required check, so that blocks nothing; it only makes a missing or stale
# comment visible instead of leaving it to look current.
set -uo pipefail

: "${REPO:?REPO is required}"
: "${PR_NUMBER:?PR_NUMBER is required}"
: "${BODY_FILE:?BODY_FILE is required}"

readonly MARKER='<!-- schema-diff -->'
readonly AUTHOR="${COMMENT_AUTHOR:-github-actions[bot]}"

done_with() {
    echo "::notice title=Schema-diff::$1"
    exit 0
}

fail() {
    echo "::error title=Schema-diff::$1"
    exit 1
}

stderr_file=$(mktemp "${TMPDIR:-/tmp}/schema-diff-comment.XXXXXX")
trap 'rm -f "${stderr_file:?}"' EXIT
gh_error() { tr '\n' ' ' <"$stderr_file"; }

if ! existing=$(gh api "repos/${REPO}/issues/${PR_NUMBER}/comments?per_page=100" --paginate \
    --jq ".[] | select(.user.login == \"${AUTHOR}\") | select(.body | startswith(\"${MARKER}\")) | .id" \
    2>"$stderr_file"); then
    fail "The comments of pull request ${PR_NUMBER} could not be read, so the schema diff is not posted. Error: $(gh_error)"
fi

# Keep the first one; a later copy (from two runs racing) is deleted.
comment_id=$(head -n1 <<<"$existing")
for id in $(tail -n +2 <<<"$existing"); do
    gh api -X DELETE "repos/${REPO}/issues/comments/${id}" >/dev/null 2>"$stderr_file" ||
        fail "The duplicate comment ${id} could not be deleted. Error: $(gh_error)"
done

if [ ! -s "$BODY_FILE" ]; then
    [ -n "$comment_id" ] ||
        done_with "This pull request adds no schema version; there is no comment to post."
    gh api -X DELETE "repos/${REPO}/issues/comments/${comment_id}" >/dev/null 2>"$stderr_file" ||
        fail "The comment ${comment_id} could not be deleted. Error: $(gh_error)"
    done_with "This pull request no longer adds a schema version; the earlier comment is deleted."
fi

# The body goes into JSON through jq, not by pasting strings: it holds quotes,
# backslashes and newlines from the schema descriptions.
if [ -n "$comment_id" ]; then
    jq -Rs '{body: .}' <"$BODY_FILE" |
        gh api -X PATCH "repos/${REPO}/issues/comments/${comment_id}" --input - >/dev/null 2>"$stderr_file" ||
        fail "The comment ${comment_id} could not be updated. Error: $(gh_error)"
    done_with "Updated the schema diff in comment ${comment_id}."
fi

jq -Rs '{body: .}' <"$BODY_FILE" |
    gh api -X POST "repos/${REPO}/issues/${PR_NUMBER}/comments" --input - >/dev/null 2>"$stderr_file" ||
    fail "The schema diff could not be posted. Error: $(gh_error)"
done_with "Posted the schema diff as a new comment."
