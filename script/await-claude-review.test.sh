#!/usr/bin/env bash
# Tests voor script/await-claude-review.sh.
#
# `gh` wordt vervangen door een stub die de endpoints kent die de poort gebruikt:
# de pull request, de blob-sha van het workflowbestand per ref, de jobs van de
# workflow-run en de comments van de PR. De jobs komen uit een reeks
# — één antwoord per aanroep — zodat een lopende review die later klaar is te
# simuleren valt. Zo is elk pad deterministisch te bewijzen zonder GitHub. Elk
# ander endpoint laat de stub falen.
#
# De stub schrijft elke aanroep weg in `$STUB_CALLS`. Daarmee is niet alleen te
# toetsen wát de poort besluit, maar ook in welke volgorde zij kijkt: de comments
# mogen pas worden opgevraagd nadat vaststaat dat er gereviewd is.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE="${HERE}/await-claude-review.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "${WORK}/bin"
cat >"${WORK}/bin/gh" <<'STUB'
#!/usr/bin/env bash
url="$2"
SLURP=0
for arg in "$@"; do [ "$arg" = --slurp ] && SLURP=1; done

# Zoals `gh api --paginate --slurp`: één array per pagina, gebundeld tot één
# array van arrays. De fixture bevat de pagina's als losse documenten.
fixture() {
  body=$(cat "$1")
  if [ "$body" = "ERROR" ]; then
    echo "gh: simulated API failure" >&2
    exit 1
  fi
  # Alleen bundelen als de aanroep erom vraagt, anders zou de stub een poort
  # zonder `--slurp` niet van een mét kunnen onderscheiden. Onleesbare fixtures
  # gaan ongemoeid door: die toetsen wat de poort doet met niet-JSON.
  if [ "$SLURP" = 1 ]; then
    printf '%s\n' "$body" | jq -s '.' 2>/dev/null || printf '%s\n' "$body"
  else
    printf '%s\n' "$body"
  fi
}

# `sequence <file> <counter>`: the next line of <file> on each call, and the
# last line once they run out.
sequence() {
  n=$(cat "$2")
  echo $((n + 1)) >"$2"
  # No mapfile: macOS ships bash 3.2, where the stub would otherwise answer
  # nothing and every case would sit out the gate's poll loop.
  responses=()
  while IFS= read -r l; do responses+=("$l"); done <"$1"
  idx=$n
  if [ "$idx" -ge "${#responses[@]}" ]; then
    idx=$((${#responses[@]} - 1))
  fi
  line="${responses[$idx]}"
  if [ "$line" = "ERROR" ]; then
    echo "gh: simulated API failure" >&2
    exit 1
  fi
  printf '%s\n' "$line"
}

case "$url" in
*/actions/runs/76/jobs*)
  # An older manual run of the same commit, already green.
  printf '{"jobs":[{"name":"claude-review (PR #1 @ c0ffee0000000000000000000000000000000000)","status":"completed","conclusion":"success","steps":[{"name":"Record that the review ran","conclusion":"success"}],"started_at":"2026-08-08T10:00:00Z","id":4,"html_url":"http://example.invalid/older"}]}\n'
  ;;
*/actions/runs/77/jobs*)
  # The jobs of the manual run in the fixtures below.
  echo 'dispatch-jobs' >>"$STUB_CALLS"
  sequence "$STUB_DISPATCH_JOBS" "$STUB_DISPATCH_JOBS_COUNTER"
  ;;
*/actions/runs/88/jobs*)
  # The jobs of another pull_request run on the same commit.
  echo 'sibling-jobs' >>"$STUB_CALLS"
  sequence "$STUB_SIBLING_JOBS" "$STUB_SIBLING_JOBS_COUNTER"
  ;;
*/actions/runs/*/jobs*)
  sequence "$STUB_JOBS" "$STUB_COUNTER"
  ;;
*/actions/workflows/claude-code-review.yml/runs\?event=pull_request\&head_sha=c0ffee0000000000000000000000000000000000*)
  echo 'sibling-runs' >>"$STUB_CALLS"
  fixture "$STUB_SIBLING_RUNS"
  ;;
*/compare/main...c0ffee0000000000000000000000000000000000\?per_page=1)
  fixture "$STUB_COMPARE"
  ;;
*/actions/workflows/claude-code-review.yml/runs\?event=workflow_dispatch\&branch=main*)
  echo 'dispatch-runs' >>"$STUB_CALLS"
  sequence "$STUB_DISPATCH_RUNS" "$STUB_DISPATCH_RUNS_COUNTER"
  ;;
*/issues/*/comments*)
  echo 'sticky-comments' >>"$STUB_CALLS"
  fixture "$STUB_STICKY_COMMENTS"
  ;;
*/pulls/*/comments*)
  echo 'inline-comments' >>"$STUB_CALLS"
  fixture "$STUB_INLINE_COMMENTS"
  ;;
*/pulls/*/reviews*)
  echo 'reviews' >>"$STUB_CALLS"
  fixture "$STUB_REVIEWS"
  ;;
*/actions/runs/*)
  body=$(cat "$STUB_RUN")
  if [ "$body" = "ERROR" ]; then
    echo "gh: simulated API failure" >&2
    exit 1
  fi
  printf '%s\n' "$body"
  ;;
*/contents/*)
  # De opgevraagde ref wordt vastgelegd, zodat een test kan asserten dat de
  # poort de merge-ref van déze PR vergelijkt en niet zomaar een commit.
  printf '%s\n' "${url#*ref=}" >>"$STUB_REFS"
  case "$url" in
  *ref=main) body=$(cat "$STUB_WORKFLOW_BASE") ;;
  *ref=refs/pull/1/merge) body=$(cat "$STUB_WORKFLOW_RUN") ;;
  *ref=d15a7c4000000000000000000000000000000000) body=$(cat "$STUB_WORKFLOW_DISPATCH") ;;
  *ref=c0ffee0000000000000000000000000000000000) body=$(cat "$STUB_WORKFLOW_HEAD") ;;
  *ref=ba5e000000000000000000000000000000000000) body=$(cat "$STUB_WORKFLOW_MERGE_BASE") ;;
  *)
    echo "gh: HTTP 404: onverwachte ref in ${url}" >&2
    exit 1
    ;;
  esac
  if [ "$body" = "ERROR" ]; then
    echo "gh: HTTP 404" >&2
    exit 1
  fi
  if [ "$body" = "NETWERK" ]; then
    echo "gh: connection reset by peer" >&2
    exit 1
  fi
  printf '{"sha":"%s"}\n' "$body"
  ;;
*/pulls/*)
  body=$(cat "$STUB_PR")
  # From the second call on, what the PR has become since (if set).
  n=$(cat "$STUB_PR_COUNTER")
  echo $((n + 1)) >"$STUB_PR_COUNTER"
  if [ "$n" -gt 0 ] && [ -s "$STUB_PR_LATER" ]; then
    body=$(cat "$STUB_PR_LATER")
  fi
  if [ "$body" = "ERROR" ]; then
    echo "gh: HTTP 403" >&2
    exit 1
  fi
  # WARN: geslaagde aanroep die ook naar stderr schrijft.
  if [ "$body" = "WARN" ]; then
    echo "gh: warning: this API is deprecated" >&2
    cat "$STUB_PR_WARN_BODY"
    exit 0
  fi
  printf '%s\n' "$body"
  ;;
*)
  echo "gh: onverwachte URL: $url" >&2
  exit 1
  ;;
esac
STUB
chmod +x "${WORK}/bin/gh"
export PATH="${WORK}/bin:${PATH}"

export STUB_COUNTER="${WORK}/counter"
export STUB_JOBS="${WORK}/jobs"
export STUB_PR="${WORK}/pr"
export STUB_PR_WARN_BODY="${WORK}/pr-warn-body"
export STUB_PR_LATER="${WORK}/pr-later"
export STUB_PR_COUNTER="${WORK}/pr-counter"
export STUB_RUN="${WORK}/run"
export STUB_REFS="${WORK}/refs"
export STUB_WORKFLOW_RUN="${WORK}/workflow-run"
export STUB_WORKFLOW_BASE="${WORK}/workflow-base"
export STUB_STICKY_COMMENTS="${WORK}/sticky-comments"
export STUB_INLINE_COMMENTS="${WORK}/inline-comments"
export STUB_REVIEWS="${WORK}/reviews"
export STUB_CALLS="${WORK}/calls"
export STUB_DISPATCH_RUNS="${WORK}/dispatch-runs"
export STUB_DISPATCH_RUNS_COUNTER="${WORK}/dispatch-runs-counter"
export STUB_DISPATCH_JOBS="${WORK}/dispatch-jobs"
export STUB_DISPATCH_JOBS_COUNTER="${WORK}/dispatch-jobs-counter"
export STUB_WORKFLOW_DISPATCH="${WORK}/workflow-dispatch"
export STUB_WORKFLOW_HEAD="${WORK}/workflow-head"
export STUB_WORKFLOW_MERGE_BASE="${WORK}/workflow-merge-base"
export STUB_COMPARE="${WORK}/compare"
export STUB_SIBLING_RUNS="${WORK}/sibling-runs"
export STUB_SIBLING_JOBS="${WORK}/sibling-jobs"
export STUB_SIBLING_JOBS_COUNTER="${WORK}/sibling-jobs-counter"

PROOF='Record that the review ran'
CRITICAL='🔴 **Critical**'
TAG='<!-- claude-review -->'
HEAD_SHA='c0ffee0000000000000000000000000000000000'

# pull_request <full_name van de head-repo> <draft> <auteur>
pull_request() {
  printf '{"draft":%s,"user":{"login":"%s"},"head":{"sha":"%s","repo":{"full_name":"%s"}},"base":{"ref":"%s","repo":{"default_branch":"main"}}}' \
    "${2:-false}" "${3:-iemand}" "$HEAD_SHA" "${1:-example-org/example-repo}" "${4:-main}"
}

PR_NORMAAL=$(pull_request)
PR_FORK=$(pull_request 'iemand-anders/example-repo')
PR_DRAFT=$(pull_request 'example-org/example-repo' true)
PR_DEPENDABOT=$(pull_request 'example-org/example-repo' false 'dependabot[bot]')
PR_ZONDER_DEFAULT_BRANCH='{"draft":false,"user":{"login":"iemand"},"head":{"sha":"c0ffee0000000000000000000000000000000000","repo":{"full_name":"example-org/example-repo"}},"base":{"repo":{}}}'
# De head-repository is verwijderd; `head.repo` is dan null.
PR_ZONDER_HEAD_REPO='{"draft":false,"user":{"login":"iemand"},"head":{"sha":"c0ffee0000000000000000000000000000000000","repo":null},"base":{"repo":{"default_branch":"main"}}}'
GARBAGE='dit is geen JSON'

RUN_VAN_DEZE_PR=$(printf '{"head_sha":"%s"}' "$HEAD_SHA")
RUN_VAN_EEN_ANDERE_PR='{"head_sha":"beefbeef000000000000000000000000000000ff"}'

# De stappen van de review-job. `$1` is de conclusie van de bewijsstap; laat hem
# leeg voor een job die die stap helemaal niet kent.
steps_json() {
  if [ -z "${1:-}" ]; then
    printf '[{"name":"Run Claude Code Review","conclusion":"success"}]'
  else
    printf '[{"name":"Run Claude Code Review","conclusion":"success"},{"name":"%s","conclusion":"%s"}]' \
      "$PROOF" "$1"
  fi
}

# Een job zonder `steps`-sleutel; die is in het API-schema optioneel.
JOB_ZONDER_STEPS='{"jobs":[{"name":"claude-review","status":"completed","conclusion":"success","html_url":"http://example.invalid/job"}]}'
# Vlak na het afronden geeft de API de job soms met een lege stappenlijst.
JOB_LEGE_STEPS='{"jobs":[{"name":"claude-review","status":"completed","conclusion":"success","steps":[],"html_url":"http://example.invalid/job"}]}'
# Een `steps`-waarde die geen lijst is: dan valt er niets te tellen.
JOB_ONLEESBARE_STEPS='{"jobs":[{"name":"claude-review","status":"completed","conclusion":"success","steps":"geen lijst","html_url":"http://example.invalid/job"}]}'
# De bewijsstap staat er wel, maar zonder conclusie.
JOB_STAP_ZONDER_CONCLUSIE="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":[{\"name\":\"${PROOF}\",\"conclusion\":null}],\"html_url\":\"http://example.invalid/job\"}]}"
# Twee stappen met dezelfde naam: een decoy naast het echte bewijs.
JOB_DUBBELE_STAP="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":[{\"name\":\"${PROOF}\",\"conclusion\":\"skipped\"},{\"name\":\"${PROOF}\",\"conclusion\":\"success\"}],\"html_url\":\"http://example.invalid/job\"}]}"

# De starttijd van de review-job scheidt wat déze review schreef van wat er van
# een vorige is blijven staan.
JOB_START='2026-08-08T10:00:00Z'
TOEN='2026-08-08T09:00:00Z'
NU='2026-08-08T10:05:00Z'

job() {
  printf '{"jobs":[{"name":"claude-review","status":"%s","conclusion":%s,"steps":%s,"started_at":"%s","html_url":"http://example.invalid/job"}]}\n' \
    "$1" "$2" "$(steps_json "${3-success}")" "$JOB_START"
}

RUNNING=$(job in_progress null)
DONE_OK=$(job completed '"success"')
DONE_NEUTRAL=$(job completed '"neutral"')
DONE_FAIL=$(job completed '"failure"')
DONE_CANCELLED=$(job completed '"cancelled"')
DONE_SKIPPED=$(job completed '"skipped"')
# De review-actie stapte eruit voordat zij iets reviewde: de bewijsstap is
# overgeslagen, terwijl de job groen afsluit.
DONE_NO_REVIEW=$(job completed '"success"' skipped)
# Een workflow van vóór de bewijsstap; de job kent de stap niet.
DONE_NO_PROOF_STEP=$(job completed '"success"' '')
DONE_PROOF_FAILED=$(job completed '"success"' failure)
# Zonder starttijd is niet uit elkaar te houden wat van deze review is.
DONE_ZONDER_STARTTIJD="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":$(steps_json success),\"html_url\":\"http://example.invalid/job\"}]}"
NO_JOB='{"jobs":[{"name":"iets-anders","status":"completed","conclusion":"success"}]}'
# Na "Re-run this job" op de poort staan er meerdere attempts van dezelfde job
# op één run-id; de hoogste id is de meest recente.
ATTEMPTS="{\"jobs\":[{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"success\",\"steps\":$(steps_json success),\"started_at\":\"${JOB_START}\",\"id\":9,\"html_url\":\"http://example.invalid/9\"},{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"failure\",\"steps\":$(steps_json skipped),\"started_at\":\"${JOB_START}\",\"id\":1,\"html_url\":\"http://example.invalid/1\"}]}"

# What GitHub lists for a job that was skipped: completed, conclusion skipped,
# and no steps, ever. Next to it the job that decided to skip it. This is the
# run from PR 1801, whose gate waited 35 minutes for steps that never come.
TARGET_DONE='{"name":"Resolve the commit to review","status":"completed","conclusion":"success","id":2}'
OWN_SKIPPED="{\"jobs\":[${TARGET_DONE},{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"skipped\",\"steps\":[],\"id\":3,\"html_url\":\"http://example.invalid/own\"}]}"
OWN_CANCELLED="{\"jobs\":[${TARGET_DONE},{\"name\":\"claude-review\",\"status\":\"completed\",\"conclusion\":\"cancelled\",\"steps\":$(steps_json ''),\"started_at\":\"${JOB_START}\",\"id\":3,\"html_url\":\"http://example.invalid/own\"}]}"
# `target` is done but `claude-review` never appeared.
OWN_ABSENT="{\"jobs\":[${TARGET_DONE}]}"
# `target` is still waiting for a runner; `claude-review` cannot exist yet.
TARGET_QUEUED='{"jobs":[{"name":"Resolve the commit to review","status":"queued","conclusion":null,"id":2}]}'

# A manual run (workflow_dispatch) on the default branch. Its own head_sha is a
# commit on main; what it reviewed is in the name of its review job.
DISPATCH_SHA='d15a7c4000000000000000000000000000000000'
# dispatch_run <head_branch> <event> <titel> <pad> <repo>
dispatch_run() {
  printf '{"workflow_runs":[{"id":77,"event":"%s","path":"%s","head_branch":"%s","head_sha":"%s","head_repository":{"full_name":"%s"},"display_title":"%s","html_url":"http://example.invalid/run/77"}]}' \
    "${2:-workflow_dispatch}" "${4:-.github/workflows/claude-code-review.yml}" "${1:-main}" \
    "$DISPATCH_SHA" "${5:-example-org/example-repo}" "${3:-Claude Code Review: PR #1}"
}
GEEN_DISPATCH='{"workflow_runs":[]}'
DISPATCH=$(dispatch_run)
DISPATCH_OTHER_BRANCH=$(dispatch_run 'feature')
DISPATCH_OTHER_EVENT=$(dispatch_run main push)
DISPATCH_OTHER_PATH=$(dispatch_run main workflow_dispatch 'Claude Code Review: PR #1' '.github/workflows/iets-anders.yml')
DISPATCH_OTHER_REPO=$(dispatch_run main workflow_dispatch 'Claude Code Review: PR #1' '.github/workflows/claude-code-review.yml' 'iemand-anders/example-repo')

# dispatch_job <naam> <status> <conclusie> [bewijsstap]
dispatch_job() {
  printf '{"jobs":[{"name":"%s","status":"%s","conclusion":%s,"steps":%s,"started_at":"%s","id":5,"html_url":"http://example.invalid/dispatch-job"}]}\n' \
    "$1" "$2" "$3" "$(steps_json "${4-success}")" "$JOB_START"
}
DISPATCH_NAME="claude-review (PR #1 @ ${HEAD_SHA})"
DISPATCH_DONE_OK=$(dispatch_job "$DISPATCH_NAME" completed '"success"')
DISPATCH_RUNNING=$(dispatch_job "$DISPATCH_NAME" in_progress null)
DISPATCH_NO_REVIEW=$(dispatch_job "$DISPATCH_NAME" completed '"success"' skipped)
DISPATCH_FAILED=$(dispatch_job "$DISPATCH_NAME" completed '"failure"')
# A manual run of the same PR, but for an earlier commit.
DISPATCH_OLD_COMMIT=$(dispatch_job 'claude-review (PR #1 @ beefbeef000000000000000000000000000000ff)' completed '"success"')
# A manual run of another PR that happens to be at the same commit.
DISPATCH_OTHER_PR=$(dispatch_job "claude-review (PR #2 @ ${HEAD_SHA})" completed '"success"')

# De comments zoals de API ze teruggeeft.
# `comment <auteur> <tekst> [tijdstip]`, standaard geschreven tijdens deze run.
comment() {
  printf '[{"user":{"login":"%s"},"body":"%s\\n\\n%s","updated_at":"%s","html_url":"http://example.invalid/comment"}]' \
    "$1" "$2" "$TAG" "${3:-$NU}"
}
# Een ingediende review draagt `submitted_at` in plaats van `updated_at`.
review() {
  printf '[{"user":{"login":"%s"},"state":"COMMENTED","body":"%s\\n\\n%s","submitted_at":"%s","html_url":"http://example.invalid/review"}]' \
    "$1" "$2" "$TAG" "${3:-$NU}"
}
GEEN_COMMENTS='[]'
# What a review with nothing to report leaves: a summary that says so.
COMMENT_SCHOON=$(comment 'claude[bot]' 'No issues found in the added or modified lines.')
COMMENT_MINOR=$(comment 'claude[bot]' '## Kwaliteit\n\n🟡 **Minor** — deze naam dekt de lading niet.')
COMMENT_CRITICAL=$(comment 'claude[bot]' "## Correctheid\n\n${CRITICAL} — hier gaat een uitkering de verkeerde kant op.")
# Een bevinding uit een vorige run: geschreven vóór deze review-job begon. De
# opruimstap haalt zulke comments weg, maar een ingediende review is niet te
# verwijderen, en een mislukte opruiming mag geen gerepareerde bevinding voor
# altijd blijven blokkeren.
COMMENT_CRITICAL_VAN_TOEN=$(comment 'claude[bot]' "## Correctheid\n\n${CRITICAL} — dit is al gerepareerd." "$TOEN")
REVIEW_CRITICAL=$(review 'claude[bot]' "## Correctheid\n\n${CRITICAL} — dit stond alleen in de body van de review.")
# Zonder tijdstempel valt een comment buiten het venster en zou hij stil worden
# overgeslagen: fail-open, en onzichtbaar.
COMMENT_ZONDER_TIJD='[{"user":{"login":"claude[bot]"},"body":"iets\n\n<!-- claude-review -->","html_url":"http://example.invalid/comment"}]'
# Een review die nog niet is ingediend heeft terecht geen `submitted_at`.
REVIEW_PENDING='[{"user":{"login":"claude[bot]"},"state":"PENDING","body":"nog niet ingediend\n\n<!-- claude-review -->","html_url":"http://example.invalid/review"}]'
REVIEW_CRITICAL_VAN_TOEN=$(review 'claude[bot]' "${CRITICAL} — uit een vorige run." "$TOEN")
# Dezelfde markering, maar van een mens die de bevinding bespreekt. Wie erover
# praat blokkeert de merge niet; alleen wat de review zelf schreef telt.
COMMENT_CRITICAL_VAN_EEN_MENS=$(comment 'iemand' "Ik denk dat die ${CRITICAL} onterecht is.")

passed=0
failed=0

jobs_are() { printf '%s\n' "$@" >"$STUB_JOBS"; }
pr_is() { printf '%s\n' "$1" >"$STUB_PR"; }
run_is() { printf '%s\n' "$1" >"$STUB_RUN"; }
workflow_is() { printf '%s\n' "$1" >"$STUB_WORKFLOW_RUN"; }
sticky_is() { printf '%s\n' "$1" >"$STUB_STICKY_COMMENTS"; }
inline_is() { printf '%s\n' "$1" >"$STUB_INLINE_COMMENTS"; }
reviews_are() { printf '%s\n' "$1" >"$STUB_REVIEWS"; }
comments_were_read() { grep -qxE 'sticky-comments|inline-comments|reviews' "$STUB_CALLS"; }
all_comments_were_read() {
  grep -qxF 'sticky-comments' "$STUB_CALLS" &&
    grep -qxF 'inline-comments' "$STUB_CALLS" &&
    grep -qxF 'reviews' "$STUB_CALLS"
}
dispatch_runs_are() { printf '%s\n' "$@" >"$STUB_DISPATCH_RUNS"; }
dispatch_jobs_are() { printf '%s\n' "$@" >"$STUB_DISPATCH_JOBS"; }

# Elke test start van dezelfde grond: een gewone PR waarvan het workflowbestand
# gelijk is aan dat op de default branch, en één afgeronde, geslaagde review
# waarvan de bewijsstap gedraaid heeft. Een test zet alleen wat hij zelf wil
# bewijzen en kan niet per ongeluk slagen op de fixtures van zijn voorganger.
reset_fixtures() {
  jobs_are "$DONE_OK"
  pr_is "$PR_NORMAAL"
  printf '%s\n' "$PR_NORMAAL" >"$STUB_PR_WARN_BODY"
  run_is "$RUN_VAN_DEZE_PR"
  workflow_is 'blob-van-main'
  printf '%s\n' 'blob-van-main' >"$STUB_WORKFLOW_BASE"
  sticky_is "$COMMENT_SCHOON"
  inline_is "$GEEN_COMMENTS"
  reviews_are "$GEEN_COMMENTS"
  echo 0 >"$STUB_COUNTER"
  echo 0 >"$STUB_PR_COUNTER"
  : >"$STUB_PR_LATER"
  dispatch_runs_are "$GEEN_DISPATCH"
  dispatch_jobs_are "$DISPATCH_DONE_OK"
  printf '%s\n' 'blob-van-main' >"$STUB_WORKFLOW_DISPATCH"
  echo 0 >"$STUB_DISPATCH_RUNS_COUNTER"
  echo 0 >"$STUB_DISPATCH_JOBS_COUNTER"
  printf '%s\n' 'blob-van-main' >"$STUB_WORKFLOW_HEAD"
  printf '%s\n' 'blob-van-main' >"$STUB_WORKFLOW_MERGE_BASE"
  printf '%s\n' '{"merge_base_commit":{"sha":"ba5e000000000000000000000000000000000000"}}' >"$STUB_COMPARE"
  printf '%s\n' '{"workflow_runs":[]}' >"$STUB_SIBLING_RUNS"
  printf '%s\n' "$DONE_OK" >"$STUB_SIBLING_JOBS"
  echo 0 >"$STUB_SIBLING_JOBS_COUNTER"
  : >"$STUB_REFS"
  : >"$STUB_CALLS"
  : >"${WORK}/summary.md"
}

# check <naam> <exitcode> <tekst in de output> <tekst in de step summary> -- env=waarde...
check() {
  local name="$1" want_code="$2" want_text="$3" want_summary="$4"
  shift 5

  local out code guard=()
  # A gate that waits where it should not would sit out MAX_WAIT_SECONDS; cut
  # that short where `timeout` exists, so the test fails instead of hanging.
  command -v timeout >/dev/null && guard=(timeout 60)
  out=$(${guard[@]+"${guard[@]}"} env DRAFT_SETTLE_SECONDS=0 "$@" \
    REPO='example-org/example-repo' \
    RUN_ID='42' \
    PR_NUMBER='1' \
    POLL_SECONDS=0 \
    GITHUB_STEP_SUMMARY="${WORK}/summary.md" \
    "$GATE" 2>&1)
  code=$?

  local problem=''
  if [ "$code" -ne "$want_code" ]; then
    problem="exitcode ${code}, verwacht ${want_code}"
  elif ! grep -qF -- "$want_text" <<<"$out"; then
    problem="output mist \"${want_text}\""
  elif ! grep -qF -- "$want_summary" "${WORK}/summary.md"; then
    problem="step summary mist \"${want_summary}\""
  fi

  if [ -n "$problem" ]; then
    echo "FAIL ${name}: ${problem}"
    printf '%s\n' "$out" | sed 's/^/     | /'
    failed=$((failed + 1))
    return
  fi
  echo "ok   ${name}"
  passed=$((passed + 1))
}

echo "== niet van toepassing =="
# De review draait hier nog; de poort mag daar niet op gaan wachten. Fork, draft
# en auteur haalt de poort zelf op bij de pull-request-API, niet uit de omgeving
# die het workflowbestand van de PR meegeeft.
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_FORK"
check "cross-repo-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "andere repository" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_DRAFT"
check "draft-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "draft" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_DEPENDABOT"
check "dependabot-PR wacht niet en blokkeert niet" 0 "niet van toepassing" "dependabot" -- MAX_WAIT_SECONDS=0
# PR 1801: de run van een push leest "draft", en seconden later is de PR ready.
# Dan mag deze poort niet groen "niet van toepassing" zeggen: dat werd anders
# het enige oordeel over de head-commit, zonder review.
reset_fixtures
jobs_are "$OWN_SKIPPED"
pr_is "$PR_DRAFT"
printf '%s\n' "$PR_NORMAAL" >"$STUB_PR_LATER"
check "draft die binnen de wachttijd ready wordt: geen groen, maar rood met wat te doen" 1 "intussen ready for review" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0 DRAFT_SETTLE_SECONDS=30
reset_fixtures
jobs_are "$OWN_SKIPPED"
pr_is "$PR_DRAFT"
printf '%s\n' "$PR_NORMAAL" >"$STUB_PR_LATER"
dispatch_runs_are "$DISPATCH"
check "draft die ready wordt, met een handmatige review: groen" 0 "review afgerond (success)" "handmatige run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0 DRAFT_SETTLE_SECONDS=30
reset_fixtures
pr_is "$PR_DRAFT"
printf '%s\n' "$(pull_request 'example-org/example-repo' false | sed 's/c0ffee0000000000000000000000000000000000/beefbeef000000000000000000000000000000ff/')" >"$STUB_PR_LATER"
check "draft die ready wordt op een nieuwere commit: rood, over deze commit" 1 "staat nu op commit" "beoordeelt die" \
  -- MAX_WAIT_SECONDS=0 DRAFT_SETTLE_SECONDS=30
reset_fixtures
jobs_are "$RUNNING"
pr_is "$PR_DRAFT"
check "draft die draft blijft: na de wachttijd niet van toepassing" 0 "niet van toepassing" "draft" \
  -- MAX_WAIT_SECONDS=0 DRAFT_SETTLE_SECONDS=1
# Een PR die de poort zelf een uitzondering probeert aan te praten via het
# env-blok van het workflowbestand: de poort leest die variabelen niet.
reset_fixtures
check "env-blok van de PR maakt geen uitzondering" 0 "review afgerond (success)" "groen" \
  -- IS_DRAFT=true IS_CROSS_REPO=true PR_AUTHOR='dependabot[bot]' MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_NO_REVIEW"
check "env-blok van de PR wijst de poort niet naar een andere stap" 1 "overgeslagen" "overgeslagen" \
  -- PROOF_STEP='Run Claude Code Review' JOB_NAME='review-gate' WORKFLOW_FILE='README.md' MAX_WAIT_SECONDS=0

echo "== de poort sluit =="
reset_fixtures
pr_is ERROR
check "PR niet op te halen: rood, niet als 'geen review' gemeld" 1 "is niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
pr_is "$GARBAGE"
check "onparseerbaar antwoord op de PR: rood met een eigen melding" 1 "is niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
pr_is "$PR_ZONDER_DEFAULT_BRANCH"
check "PR zonder default branch: rood, want er valt niets te vergelijken" 1 "default branch" "niet te vergelijken" -- MAX_WAIT_SECONDS=0
# De drie coordinaten moeten over hetzelfde gaan: een run-id dat naar de run van
# een andere PR wijst, mag geen groen opleveren op andermans review.
reset_fixtures
run_is "$RUN_VAN_EEN_ANDERE_PR"
check "run hoort bij een andere commit: rood" 1 "draait op commit" "andere wijziging" -- MAX_WAIT_SECONDS=0
reset_fixtures
run_is ERROR
check "workflow-run niet op te halen: rood" 1 "Workflow-run 42 is niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
pr_is "$PR_ZONDER_HEAD_REPO"
check "verwijderde head-repo: niet van toepassing, met de juiste reden" 0 "bestaat niet meer" "bestaat niet meer" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
check "review loopt nog bij deadline: rood" 1 "nog niet klaar" "nog niet klaar" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$NO_JOB"
check "review-job zit niet in de run: rood" 1 "zit er geen job" "zit er geen job" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_FAIL"
check "review gefaald: rood" 1 'eindigde op `failure`' 'eindigde op `failure`' -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_CANCELLED"
check "review geannuleerd: rood" 1 'eindigde op `cancelled`' 'eindigde op `cancelled`' -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_SKIPPED"
check "review overgeslagen zonder geldige reden: rood" 1 "is overgeslagen" "is overgeslagen" -- MAX_WAIT_SECONDS=0

reset_fixtures
jobs_are ERROR
check "jobs-API blijft onleesbaar tot de deadline: rood, niet als 'job ontbreekt'" 1 "niet op te halen" "niet op te halen" -- MAX_WAIT_SECONDS=0

echo "== een workflowbestand dat afwijkt van de default branch =="
# De claude-code-action weigert te draaien zodra dat bestand afwijkt: zij stapt
# uit met conclusie success zonder te reviewen. De poort stelt die afwijking zelf
# vast en wacht niet eerst een halfuur op een review die niet komt.
reset_fixtures
jobs_are "$RUNNING"
workflow_is 'blob-van-de-pr'
check "workflowbestand wijkt af: rood, zonder te wachten" 1 "is in deze run een andere versie" "een mens de wijziging nalopen" -- MAX_WAIT_SECONDS=0
# Vergeleken wordt de merge-ref van deze PR: dat is het bestand dat de run
# werkelijk draaide. De stub weigert elke andere ref, dus deze test valt om zodra
# de poort op de head-commit gaat vergelijken.
if grep -qxF 'refs/pull/1/merge' "$STUB_REFS" && grep -qxF 'main' "$STUB_REFS"; then
  echo "ok   vergeleken wordt de merge-ref van deze PR tegen de default branch"
  passed=$((passed + 1))
else
  echo "FAIL de poort vroeg de refs $(tr '\n' ' ' <"$STUB_REFS")op, niet refs/pull/1/merge en main"
  failed=$((failed + 1))
fi
reset_fixtures
jobs_are "$RUNNING"
workflow_is ERROR
check "workflowbestand ontbreekt op de merge-ref: rood, opnieuw draaien helpt niet" 1 "bestaat niet op" "helpt hier niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
workflow_is NETWERK
check "workflowbestand onleesbaar door een leesfout: rood, draai opnieuw" 1 "niet op te halen voor" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
printf '%s\n' ERROR >"$STUB_WORKFLOW_BASE"
check "workflowbestand ontbreekt op de default branch: rood" 1 "bestaat niet op branch" "helpt hier niet" -- MAX_WAIT_SECONDS=0

echo "== een groene job die niets reviewde =="
# Het workflowbestand is hier gelijk aan dat op de default branch, dus de
# workflow-validation-skip is uitgesloten en de melding zegt dat er ook bij.
reset_fixtures
jobs_are "$DONE_NO_REVIEW"
check "bewijsstap overgeslagen: rood" 1 "is in die job overgeslagen" "verklaart dit niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_NO_PROOF_STEP"
check "job zonder bewijsstap: rood" 1 "kent geen stap" "kent geen stap" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$JOB_ZONDER_STEPS"
check "job zonder stappenlijst, ook na het wachten: rood, draai opnieuw" 1 "nog geen stappen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$JOB_LEGE_STEPS"
check "job met lege stappenlijst, ook na het wachten: rood, draai opnieuw" 1 "nog geen stappen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
# De API levert de stappen een poll later: de poort wacht erop in plaats van
# te concluderen dat de bewijsstap ontbreekt.
reset_fixtures
jobs_are "$JOB_LEGE_STEPS" "$DONE_OK"
check "stappen komen een poll later: groen" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=60
reset_fixtures
jobs_are "$JOB_ZONDER_STEPS" "$DONE_OK"
check "stappenlijst komt een poll later: groen" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=60
# Twee stappen met dezelfde naam: de poort kiest daar geen winnaar uit, want dan
# zou één toegevoegde stap het bewijs kunnen overstemmen.
reset_fixtures
jobs_are "$JOB_DUBBELE_STAP"
check "twee stappen met de bewijsnaam: rood" 1 "2 stappen met de naam" "kan de poort niet uitmaken" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_PROOF_FAILED"
check "bewijsstap gefaald: rood" 1 'eindigde op `failure`' 'eindigde op `failure`' -- MAX_WAIT_SECONDS=0
# De twee "we weten het niet"-armen horen rood te zijn, niet groen bij gebrek aan
# een leesbare uitkomst.
reset_fixtures
jobs_are "$JOB_ONLEESBARE_STEPS"
check "stappen niet te lezen: rood" 1 "zijn niet uit het antwoord" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$JOB_STAP_ZONDER_CONCLUSIE"
check "bewijsstap zonder conclusie: rood" 1 "geen leesbare conclusie" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0

echo "== een kritieke bevinding =="
reset_fixtures
sticky_is "$COMMENT_CRITICAL"
check "kritieke bevinding in de samenvattende review: rood" 1 "heeft op commit" "Repareer wat er staat" -- MAX_WAIT_SECONDS=0
reset_fixtures
inline_is "$COMMENT_CRITICAL"
check "kritieke bevinding in een inline comment: rood" 1 "heeft op commit" "Repareer wat er staat" -- MAX_WAIT_SECONDS=0
# Er is geen uitweg, en de melding hoort er ook geen te suggereren.
reset_fixtures
sticky_is "$COMMENT_CRITICAL"
check "de melding wijst naar repareren en pushen, niet naar een uitweg" 1 "Repareer wat er staat en push opnieuw" "Repareer wat er staat en push opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$REVIEW_CRITICAL"
check "kritieke bevinding in de body van de ingediende review: rood" 1 "heeft op commit" "Repareer wat er staat" -- MAX_WAIT_SECONDS=0
# De melding wijst aan waar de bevinding staat, anders is een onterechte rood
# niet te vinden.
reset_fixtures
inline_is "$COMMENT_CRITICAL"
check "de melding noemt waar de bevinding staat" 1 "http://example.invalid/comment" "http://example.invalid/comment" -- MAX_WAIT_SECONDS=0

# Alleen wat déze run schreef telt. Een ingediende review is niet te verwijderen
# en een mislukte opruiming laat comments staan; zonder deze grens zou een
# gerepareerde bevinding voor altijd blijven blokkeren.
reset_fixtures
sticky_is "$COMMENT_CRITICAL_VAN_TOEN"
inline_is "$COMMENT_SCHOON"
check "kritieke bevinding van vóór deze review-job blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$REVIEW_CRITICAL_VAN_TOEN"
check "review-body uit een vorige run blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$COMMENT_ZONDER_TIJD"
check "comment zonder tijdstempel: rood, niet stil overgeslagen" 1 "zonder bruikbaar tijdstempel" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$REVIEW_PENDING"
check "een nog niet ingediende review blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$DONE_ZONDER_STARTTIJD"
sticky_is "$COMMENT_CRITICAL"
check "job zonder starttijd: rood, want de grens ontbreekt" 1 "starttijd van" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0

reset_fixtures
sticky_is "$COMMENT_MINOR"
check "bevinding van lagere ernst blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$COMMENT_CRITICAL_VAN_EEN_MENS"
inline_is "$COMMENT_SCHOON"
check "een mens die de markering citeert blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0

# `claude.yml` antwoordt als dezelfde bot op `@claude`. Zo'n antwoord draagt de
# tag niet en is geen bevinding, ook niet als het de markering aanhaalt.
reset_fixtures
sticky_is '[{"user":{"login":"claude[bot]"},"body":"Die 🔴 **Critical** ging over regel 12.","updated_at":"'"$NU"'","html_url":"http://example.invalid/antwoord"}]'
inline_is "$COMMENT_SCHOON"
check "een @claude-antwoord zonder de tag blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0

# `gh api --paginate --slurp` levert één array per pagina. Werd daar per pagina
# geteld, dan las de poort "0\n0" als "niet nul" en blokkeerde hij een PR die
# genoeg runs heeft gehad om over de honderd reviews te komen.
reset_fixtures
sticky_is "$COMMENT_SCHOON
[]"
check "meer dan één pagina blokkeert niet" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0

# Niet kunnen kijken is geen schone uitslag: dat is niet te onderscheiden van
# "er staat niets", en dan hoort de poort dicht te blijven.
reset_fixtures
sticky_is ERROR
check "samenvattende comments niet op te halen: rood" 1 "zijn niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
inline_is ERROR
check "inline comments niet op te halen: rood" 1 "zijn niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are ERROR
check "ingediende reviews niet op te halen: rood" 1 "zijn niet op te halen" "draai deze job opnieuw" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$GARBAGE"
check "onleesbaar antwoord op de samenvattende comments: rood" 1 "geen leesbare lijst" "geen leesbare lijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
inline_is "$GARBAGE"
check "onleesbaar antwoord op de inline comments: rood" 1 "geen leesbare lijst" "geen leesbare lijst" -- MAX_WAIT_SECONDS=0
reset_fixtures
reviews_are "$GARBAGE"
check "onleesbaar antwoord op de ingediende reviews: rood" 1 "geen leesbare lijst" "geen leesbare lijst" -- MAX_WAIT_SECONDS=0

echo "== de bevindingen worden pas gelezen als vaststaat dat er gereviewd is =="
# De review-job wist eerst de comments van de vorige review en schrijft daarna de
# nieuwe. Wie in dat venster gaat tellen leest nul en noemt dat schoon. De poort
# mag de comments dus pas opvragen nadat de bewijsstap is vastgesteld, en deze
# tests bewijzen dat aan de aanroepen die de stub optekent.
reset_fixtures
jobs_are "$DONE_NO_REVIEW"
sticky_is "$COMMENT_CRITICAL"
check "bewijsstap overgeslagen: rood op de bewijsstap, niet op de bevinding" 1 "is in die job overgeslagen" "verklaart dit niet" -- MAX_WAIT_SECONDS=0
if comments_were_read; then
  echo "FAIL de poort las de comments terwijl nog niet vaststond dat er gereviewd was"
  failed=$((failed + 1))
else
  echo "ok   geen bewijs dat er gereviewd is: de comments blijven ongelezen"
  passed=$((passed + 1))
fi
reset_fixtures
jobs_are "$RUNNING"
check "review loopt nog: de comments blijven ongelezen" 1 "nog niet klaar" "nog niet klaar" -- MAX_WAIT_SECONDS=0
if comments_were_read; then
  echo "FAIL de poort las de comments van een review die nog liep"
  failed=$((failed + 1))
else
  echo "ok   review nog niet klaar: de comments blijven ongelezen"
  passed=$((passed + 1))
fi

echo "== de review moet iets hebben achtergelaten =="
# PR 1779, 23 oktober: de review-job was in 15 seconden groen, de bewijsstap
# had gedraaid, en op de PR stond niets. De actie zet in deze modus zelf niets
# op de PR; of er gepost wordt, beslist het model. Zonder iets van deze run is
# er geen review om op af te gaan.
reset_fixtures
sticky_is "$GEEN_COMMENTS"
check "review gedraaid, maar niets gepost: rood, met de hele run opnieuw" 1 "niets op pull request 1" "Draai de hele run opnieuw met \`gh run rerun 42\`" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$(comment 'claude[bot]' 'No issues found.' "$TOEN")"
check "alleen een samenvatting van een vorige run: rood" 1 "niets op pull request 1" "telt niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is '[{"user":{"login":"claude[bot]"},"body":"No issues found.","updated_at":"'"$NU"'","html_url":"http://example.invalid/zonder-tag"}]'
check "alleen een comment zonder de tag: rood" 1 "niets op pull request 1" "telt niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$COMMENT_CRITICAL_VAN_EEN_MENS"
check "alleen een mens schreef iets: rood" 1 "niets op pull request 1" "telt niet" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$GEEN_COMMENTS"
inline_is "$COMMENT_MINOR"
check "alleen een inline comment van deze run: groen" 0 "review afgerond (success)" "1 keer iets met" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$GEEN_COMMENTS"
reviews_are "$(review 'claude[bot]' 'No issues found.')"
check "alleen de body van een ingediende review van deze run: groen" 0 "review afgerond (success)" "1 keer iets met" -- MAX_WAIT_SECONDS=0
reset_fixtures
sticky_is "$GEEN_COMMENTS"
reviews_are "$REVIEW_PENDING"
check "alleen een nog niet ingediende review: rood" 1 "niets op pull request 1" "telt niet" -- MAX_WAIT_SECONDS=0

echo "== de poort opent =="
# Het geval uit issue 1178: de review draaide volledig af en had niets aan te
# merken. Dat is een schone PR, geen ontbrekende review: nul bevindingen telt
# als nul bevindingen, juist omdat de bewijsstap er al voor stond. Sinds PR 1779
# zegt de review dat wel in een samenvatting; die is hier het enige comment.
reset_fixtures
check "review af met nul bevindingen: groen" 0 "review afgerond (success)" "Wat hij niet bewijst" -- MAX_WAIT_SECONDS=0
if all_comments_were_read; then
  echo "ok   met bewijs van de review worden de comments wél gelezen"
  passed=$((passed + 1))
else
  echo "FAIL de poort werd groen zonder de comments te lezen"
  failed=$((failed + 1))
fi
reset_fixtures
jobs_are "$DONE_NEUTRAL"
check "conclusie neutral telt als afgerond: groen" 0 "review afgerond (neutral)" "groen" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$ATTEMPTS"
check "meerdere attempts: de nieuwste job telt" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=0
reset_fixtures
jobs_are "$RUNNING" "$RUNNING" "$DONE_OK"
check "wacht door tot de review klaar is" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=60
reset_fixtures
jobs_are ERROR "$DONE_OK"
check "API-fout op de jobs is geen conclusie, er wordt doorgepolld" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=60
reset_fixtures
pr_is WARN
check "waarschuwing op stderr bederft een geslaagde aanroep niet: groen" 0 "review afgerond (success)" "groen" -- MAX_WAIT_SECONDS=0

echo "== geen review onderweg: snel rood, met wat je moet doen =="
# PR 1801: de run van de push sloeg `claude-review` over (zijn payload zei nog
# draft), en de poort wachtte 35 minuten op stappen die een overgeslagen job
# nooit krijgt. MAX_WAIT_SECONDS staat hier op de echte waarde: een poort die
# toch gaat wachten, loopt tegen de time-out van `check` aan.
reset_fixtures
jobs_are "$OWN_SKIPPED"
check "eigen review overgeslagen, geen handmatige run: meteen rood" 1 "is overgeslagen in workflow-run 42" 'gh workflow run "Claude Code Review" -f pr=1' \
  -- MAX_WAIT_SECONDS=2100 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
check "de melding noemt ook het opnieuw draaien van de poort en draft naar ready" 1 "gh run rerun 42 --failed" "weer op ready for review" \
  -- MAX_WAIT_SECONDS=2100 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_ABSENT"
check "target klaar maar geen review-job: meteen rood" 1 "niet gestart" "gh workflow run" \
  -- MAX_WAIT_SECONDS=2100 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_CANCELLED"
check "eigen review geannuleerd, geen handmatige run: rood" 1 'eindigde op `cancelled`' "gh workflow run" \
  -- MAX_WAIT_SECONDS=2100 NO_REVIEW_GRACE_SECONDS=0
# Zolang `target` nog niet klaar is, kan `claude-review` er niet zijn: dat is
# gewoon wachten, geen reden om snel rood te worden.
reset_fixtures
jobs_are "$TARGET_QUEUED" "$TARGET_QUEUED" "$DONE_OK"
check "target staat nog in de rij: de poort wacht" 0 "review afgerond (success)" "groen" \
  -- MAX_WAIT_SECONDS=60 NO_REVIEW_GRACE_SECONDS=0
# Een handmatige run die vlak na het opnieuw draaien van de poort verschijnt,
# wordt binnen de wachttijd nog gevonden.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$GEEN_DISPATCH" "$DISPATCH"
check "handmatige run verschijnt binnen de wachttijd: overgenomen" 0 "review afgerond (success)" "handmatige run" \
  -- MAX_WAIT_SECONDS=2100 NO_REVIEW_GRACE_SECONDS=60
# Niet kunnen kijken is niet hetzelfde als "er is niets": dan geen snelle
# uitspraak, maar de gewone deadline met de leesfout erbij.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are ERROR
check "handmatige runs niet op te halen: geen snel oordeel, rood op de deadline" 1 "handmatig gestarte runs" "draai deze job opnieuw" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$RUNNING"
check "eigen review loopt: de handmatige runs worden niet bekeken" 1 "nog niet klaar" "nog niet klaar" -- MAX_WAIT_SECONDS=0
if grep -qxF 'dispatch-runs' "$STUB_CALLS"; then
  echo "FAIL de poort zocht een handmatige run terwijl de eigen review nog liep"
  failed=$((failed + 1))
else
  echo "ok   een lopende eigen review gaat voor"
  passed=$((passed + 1))
fi

echo "== een andere run op dezelfde commit neemt het over =="
# Het gewone geval: push op een draft, seconden later "ready for review". De
# push-run sloeg zijn review over; de ready_for_review-run reviewt dezelfde
# commit. De poort van de push-run wacht daarop in plaats van rood te worden.
SIBLING='{"workflow_runs":[{"id":88,"event":"pull_request","path":".github/workflows/claude-code-review.yml","head_sha":"c0ffee0000000000000000000000000000000000","head_repository":{"full_name":"example-org/example-repo"},"html_url":"http://example.invalid/run/88"}]}'
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
printf '%s\n' "$RUNNING" "$DONE_OK" >"$STUB_SIBLING_JOBS"
check "andere run reviewt dezelfde commit: de poort wacht erop en neemt hem over" 0 "review afgerond (success)" "run http://example.invalid/run/88 op dezelfde commit" \
  -- MAX_WAIT_SECONDS=60 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
inline_is "$COMMENT_CRITICAL"
check "andere run met een kritieke bevinding: rood" 1 "heeft op commit" "Repareer wat er staat" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
printf '%s\n' "$DONE_SKIPPED" >"$STUB_SIBLING_JOBS"
check "andere run sloeg ook over: rood met wat te doen" 1 "geen andere run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
# Alleen als de PR het workflowbestand niet wijzigt: dan draaide elke testmerge
# met de default branch een versie van main.
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
printf '%s\n' 'blob-van-de-pr' >"$STUB_WORKFLOW_HEAD"
check "PR wijzigt het workflowbestand: een andere run telt niet" 1 "geen andere run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
if grep -qxF 'sibling-runs' "$STUB_CALLS"; then
  echo "FAIL de poort zocht andere runs terwijl de PR het workflowbestand wijzigt"
  failed=$((failed + 1))
else
  echo "ok   wijzigt de PR het workflowbestand, dan blijven andere runs ongelezen"
  passed=$((passed + 1))
fi
reset_fixtures
jobs_are "$OWN_SKIPPED"
pr_is "$(pull_request 'example-org/example-repo' false iemand 'release')"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
check "PR met een andere base dan de default branch: een andere run telt niet" 1 "geen andere run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" | sed 's/"event":"pull_request"/"event":"push"/' >"$STUB_SIBLING_RUNS"
check "run van een ander event telt niet als andere run" 1 "geen andere run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' ERROR >"$STUB_COMPARE"
check "niet vast te stellen of de PR het workflowbestand wijzigt: geen andere run, wel snel rood" 1 "geen andere run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0

# Een overgenomen review die niets postte: opnieuw draaien van deze run helpt
# dan niet, een nieuwe review starten wel.
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
sticky_is "$GEEN_COMMENTS"
check "overgenomen review postte niets: rood, met een nieuwe review starten" 1 "niets op pull request 1" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
if grep -qF "hele run opnieuw" "${WORK}/summary.md"; then
  echo "FAIL bij een overgenomen review raadt de poort aan deze run opnieuw te draaien"
  failed=$((failed + 1))
else
  echo "ok   bij een overgenomen review raadt de poort niet aan deze run opnieuw te draaien"
  passed=$((passed + 1))
fi
# Alleen een pull_request-run van dit workflowbestand, op deze commit, in deze
# repository, en niet deze run zelf.
for variant in 's/"head_repository":{"full_name":"example-org\/example-repo"}/"head_repository":{"full_name":"iemand-anders\/example-repo"}/' \
  's/claude-code-review.yml"/iets-anders.yml"/' \
  's/"head_sha":"c0ffee0000000000000000000000000000000000"/"head_sha":"beefbeef000000000000000000000000000000ff"/' \
  's/"id":88/"id":42/'; do
  reset_fixtures
  jobs_are "$OWN_SKIPPED"
  printf '%s\n' "$SIBLING" | sed "$variant" >"$STUB_SIBLING_RUNS"
  check "andere run met ${variant%%/*/*}… telt niet" 1 "geen andere run" "gh workflow run" \
    -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
  if grep -qxF 'sibling-jobs' "$STUB_CALLS"; then
    echo "FAIL bij variant ${variant} las de poort toch de jobs van de andere run"
    failed=$((failed + 1))
  else
    echo "ok   bij variant ${variant} blijven de jobs van de andere run ongelezen"
    passed=$((passed + 1))
  fi
done
reset_fixtures
jobs_are "$OWN_SKIPPED"
printf '%s\n' "$SIBLING" >"$STUB_SIBLING_RUNS"
printf '%s\n' ERROR >"$STUB_SIBLING_JOBS"
check "jobs van de andere run niet op te halen: geen snel oordeel" 1 "niet op te halen" "draai deze job opnieuw" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0

echo "== een handmatig gestarte review neemt het over =="
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
check "eigen review overgeslagen, handmatige review van deze commit: groen" 0 "review afgerond (success)" "handmatige run http://example.invalid/run/77" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_CANCELLED"
dispatch_runs_are "$DISPATCH"
check "eigen review vervangen door een handmatige: groen" 0 "review afgerond (success)" "handmatige run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_ABSENT"
dispatch_runs_are "$DISPATCH"
dispatch_jobs_are "$DISPATCH_RUNNING" "$DISPATCH_RUNNING" "$DISPATCH_DONE_OK"
check "handmatige review loopt nog: de poort wacht erop" 0 "review afgerond (success)" "handmatige run" \
  -- MAX_WAIT_SECONDS=60 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
dispatch_jobs_are "$DISPATCH_RUNNING"
check "handmatige review loopt nog bij de deadline: rood, niet groen" 1 "nog niet klaar" "handmatige run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
# Dezelfde eisen als aan de eigen job: bewijsstap en geen Critical.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
dispatch_jobs_are "$DISPATCH_NO_REVIEW"
check "handmatige review zonder bewijs: rood" 1 "is in die job overgeslagen" "verklaart dit niet" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
inline_is "$COMMENT_CRITICAL"
check "handmatige review met een kritieke bevinding: rood" 1 "heeft op commit" "Repareer wat er staat" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
dispatch_jobs_are "$DISPATCH_FAILED"
check "handmatige review gefaald: rood, met waar hij eindigde" 1 'eindigde op `failure`' "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0

# Twee handmatige runs voor dezelfde commit: de nieuwste telt, ook als een
# oudere al groen is. De nieuwste vervangt via de concurrency-groep de oudere.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are '{"workflow_runs":[{"id":76,"event":"workflow_dispatch","path":".github/workflows/claude-code-review.yml","head_branch":"main","head_sha":"d15a7c4000000000000000000000000000000000","head_repository":{"full_name":"example-org/example-repo"},"display_title":"Claude Code Review: PR #1","html_url":"http://example.invalid/run/76"},{"id":77,"event":"workflow_dispatch","path":".github/workflows/claude-code-review.yml","head_branch":"main","head_sha":"d15a7c4000000000000000000000000000000000","head_repository":{"full_name":"example-org/example-repo"},"display_title":"Claude Code Review: PR #1","html_url":"http://example.invalid/run/77"}]}'
dispatch_jobs_are "$DISPATCH_RUNNING"
check "twee handmatige runs: de nieuwste telt" 1 "nog niet klaar" "handmatige run http://example.invalid/run/77" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0

# Zijn de jobs van de nieuwste niet te lezen, dan valt de poort niet terug op
# een oudere groene.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are '{"workflow_runs":[{"id":76,"event":"workflow_dispatch","path":".github/workflows/claude-code-review.yml","head_branch":"main","head_sha":"d15a7c4000000000000000000000000000000000","head_repository":{"full_name":"example-org/example-repo"},"display_title":"Claude Code Review: PR #1","html_url":"http://example.invalid/run/76"},{"id":77,"event":"workflow_dispatch","path":".github/workflows/claude-code-review.yml","head_branch":"main","head_sha":"d15a7c4000000000000000000000000000000000","head_repository":{"full_name":"example-org/example-repo"},"display_title":"Claude Code Review: PR #1","html_url":"http://example.invalid/run/77"}]}'
dispatch_jobs_are ERROR
check "jobs van de nieuwste handmatige run onleesbaar: geen terugval op een oudere" 1 "niet op te halen" "draai deze job opnieuw" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0

echo "== wat een handmatige run niet overneemt =="
# De naam van de review-job is het enige vastgelegde bewijs van wat de run
# reviewde; een andere commit of een andere PR telt niet.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
dispatch_jobs_are "$DISPATCH_OLD_COMMIT"
check "handmatige review van een eerdere commit: rood" 1 "geen handmatig gestarte run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
dispatch_jobs_are "$DISPATCH_OTHER_PR"
check "handmatige review van een andere PR op dezelfde commit: rood" 1 "geen handmatig gestarte run" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
# Alleen een run van dit workflowbestand, gestart op de default branch van deze
# repository: daar draait de code die de naam schrijft.
for variant in OTHER_BRANCH OTHER_EVENT OTHER_PATH OTHER_REPO; do
  reset_fixtures
  jobs_are "$OWN_SKIPPED"
  eval "dispatch_runs_are \"\$DISPATCH_${variant}\""
  check "run met ${variant} telt niet als handmatige review" 1 "geen handmatig gestarte run" "gh workflow run" \
    -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
  if grep -qxF 'dispatch-jobs' "$STUB_CALLS"; then
    echo "FAIL bij ${variant} las de poort toch de jobs van de run"
    failed=$((failed + 1))
  else
    echo "ok   bij ${variant} blijven de jobs van de run ongelezen"
    passed=$((passed + 1))
  fi
done
# Het workflowbestand op de commit van de run moet de blob zijn die nu op de
# default branch staat; anders draaide er andere code dan die de poort kent.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
printf '%s\n' 'blob-van-vroeger' >"$STUB_WORKFLOW_DISPATCH"
check "handmatige run met een andere versie van het workflowbestand: rood" 1 "andere versie" "gh workflow run" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0
# Een handmatige run kent alleen de PR en de commit; dat de eigen run over
# dezelfde commit gaat als de PR blijft gewoon gelden.
reset_fixtures
jobs_are "$OWN_SKIPPED"
dispatch_runs_are "$DISPATCH"
run_is "$RUN_VAN_EEN_ANDERE_PR"
check "run van een andere commit wordt niet gered door een handmatige review" 1 "draait op commit" "andere wijziging" \
  -- MAX_WAIT_SECONDS=0 NO_REVIEW_GRACE_SECONDS=0

echo "== de workflow levert het bewijs dat de poort zoekt =="
# De poort kent de naam van de bewijsstap als vaste waarde; hernoemen in de
# workflow zonder de poort mee te nemen zet elke PR op rood. Deze test bindt de
# twee aan elkaar.
WORKFLOW="$(cd "${HERE}/.." && pwd)/.github/workflows/claude-code-review.yml"
if grep -qF -- "- name: ${PROOF}" "$WORKFLOW" &&
  grep -qF -- "if: steps.claude-review.outputs.execution_file != ''" "$WORKFLOW"; then
  echo "ok   de bewijsstap staat onder die naam in het workflowbestand"
  passed=$((passed + 1))
else
  echo "FAIL de bewijsstap staat niet onder die naam in ${WORKFLOW}, of niet met de verwachte conditie"
  failed=$((failed + 1))
fi

# De poort zoekt de job op onder de naam `claude-review`, en die van een
# handmatige run onder `claude-review (PR #<n> @ <sha>)`. Beide komen uit één
# `name:`-expressie; wijkt die af van wat de poort zoekt, dan vindt zij de
# review niet en wordt elke PR rood.
name_expr="format('claude-review (PR #{0} @ {1})', needs.target.outputs.pr, needs.target.outputs.sha)"
if grep -qxF '  claude-review:' "$WORKFLOW" &&
  grep -qF -- "&& ${name_expr}" "$WORKFLOW" &&
  grep -qF -- "|| 'claude-review' }}" "$WORKFLOW" &&
  grep -qF -- 'readonly dispatch_job_name="${JOB_NAME} (PR #${PR_NUMBER} @ ${head_sha})"' "$GATE"; then
  echo "ok   de review-job heet in de API zoals de poort hem zoekt, ook in een handmatige run"
  passed=$((passed + 1))
else
  echo "FAIL de naam van de job \`claude-review\` in ${WORKFLOW} sluit niet aan op wat ${GATE} zoekt"
  failed=$((failed + 1))
fi

# De titel van een handmatige run en de naam van `target` zoekt de poort op als
# vaste tekst.
if grep -qF -- "format('Claude Code Review: PR #{0}', inputs.pr)" "$WORKFLOW" &&
  grep -qF -- 'readonly dispatch_title="${WORKFLOW_NAME}: PR #${PR_NUMBER}"' "$GATE" &&
  grep -qxF 'name: Claude Code Review' "$WORKFLOW" &&
  grep -qF -- "readonly WORKFLOW_NAME='Claude Code Review'" "$GATE" &&
  grep -qxF '    name: Resolve the commit to review' "$WORKFLOW" &&
  grep -qF -- "readonly TARGET_JOB='Resolve the commit to review'" "$GATE"; then
  echo "ok   runtitel, workflownaam en de naam van target sluiten aan op de poort"
  passed=$((passed + 1))
else
  echo "FAIL runtitel, workflownaam of de naam van target in ${WORKFLOW} sluit niet aan op ${GATE}"
  failed=$((failed + 1))
fi

# Handmatig starten kan, met het PR-nummer als enige invoer, en de poort draait
# niet in zo'n run: die heeft geen PR om over te rapporteren.
if grep -qxF '  workflow_dispatch:' "$WORKFLOW" &&
  awk '/^  workflow_dispatch:/ {inside = 1; next} /^[a-z]/ {inside = 0} inside && /^      pr:/ {found = 1} END {exit !found}' "$WORKFLOW" &&
  awk '/^  review-gate:/ {inside = 1; next} /^  [a-z]/ {inside = 0} inside && /^    if: github.event_name == .pull_request.$/ {found = 1} END {exit !found}' "$WORKFLOW"; then
  echo "ok   de workflow is handmatig te starten met pr, en de poort draait alleen op pull_request"
  passed=$((passed + 1))
else
  echo "FAIL ${WORKFLOW} mist workflow_dispatch met input pr, of review-gate draait niet alleen op pull_request"
  failed=$((failed + 1))
fi

# Waar het vertrouwen in een handmatige run op rust, staat in het workflowbestand:
# de commit komt uit de API en niet uit de invoer, de naam draagt die commit
# alleen in een handmatige run, de review checkt precies die commit uit, en de
# configuratie en het commentaarscript komen van de default branch.
if grep -qF -- "head_sha=\$(field '.head.sha')" "$WORKFLOW" &&
  grep -qF -- 'echo "sha=${head_sha}"' "$WORKFLOW" &&
  ! grep -qF -- 'inputs.sha' "$WORKFLOW" &&
  grep -qxF -- "      \${{ github.event_name == 'workflow_dispatch'" "$WORKFLOW" &&
  grep -A1 -xF -- "      \${{ github.event_name == 'workflow_dispatch'" "$WORKFLOW" | grep -qF -- "&& ${name_expr}" &&
  grep -qxF -- "          ref: \${{ github.event_name == 'workflow_dispatch' && needs.target.outputs.sha || '' }}" "$WORKFLOW"; then
  echo "ok   een handmatige run reviewt de commit uit de API en draagt die in de jobnaam"
  passed=$((passed + 1))
else
  echo "FAIL ${WORKFLOW} haalt de te reviewen commit niet uit de API, of de jobnaam of de checkout volgt hem niet"
  failed=$((failed + 1))
fi
restore_line=$(grep -n -- '- name: Restore Claude configuration from the default branch' "$WORKFLOW" | cut -d: -f1)
first_snapshot=$(grep -n -- '- name: Snapshot the previous review' "$WORKFLOW" | cut -d: -f1)
if [ -n "$restore_line" ] && [ -n "$first_snapshot" ] && [ "$restore_line" -lt "$first_snapshot" ] &&
  awk '/- name: Restore Claude configuration from the default branch/ {inside = 1; next} /^      - name:/ {inside = 0} inside && /if: github.event_name == .workflow_dispatch./ {i = 1} inside && /\.claude / {c = 1} inside && /CLAUDE\.md / {m = 1} inside && /script\/claude-review-comments\.sh/ {s = 1} END {exit !(i && c && m && s)}' "$WORKFLOW"; then
  echo "ok   een handmatige run zet configuratie en commentaarscript terug van de default branch, vóór de momentopname"
  passed=$((passed + 1))
else
  echo "FAIL ${WORKFLOW} zet in een handmatige run de configuratie of het commentaarscript niet terug vóór de momentopname"
  failed=$((failed + 1))
fi

# Of er gereviewd wordt, beslist `target` op feiten van de API; een overgeslagen
# job komt zo niet in de concurrency-groep en kan geen review annuleren.
if awk '/^  claude-review:/ {inside = 1; next} /^  [a-z]/ {inside = 0} inside && /^    if: needs.target.outputs.review == .true.$/ {found = 1} END {exit !found}' "$WORKFLOW" &&
  ! grep -q '^concurrency:' "$WORKFLOW"; then
  echo "ok   claude-review hangt aan target, en er is geen concurrency op workflowniveau"
  passed=$((passed + 1))
else
  echo "FAIL claude-review in ${WORKFLOW} hangt niet aan target, of er staat weer concurrency op workflowniveau"
  failed=$((failed + 1))
fi

# Een kritieke bevinding kent geen uitweg. Een label, een commentaar-commando of
# een omgevingsvariabele zou een uitweg zijn die een agent net zo makkelijk zet
# als een mens, terwijl hij leest als een menselijk oordeel.
if grep -qF '/labels' "$GATE"; then
  echo "FAIL de poort leest labels, en dat kan alleen om er een uitweg aan op te hangen"
  failed=$((failed + 1))
else
  echo "ok   de poort leest geen labels om een kritieke bevinding te omzeilen"
  passed=$((passed + 1))
fi

# De markering waarop de poort rood wordt, is de markering die de prompt de
# review voorschrijft. Wijkt de een af, dan leest de poort langs elke bevinding
# heen en is dat niet aan de uitslag te zien.
if grep -qF -- "$CRITICAL" "$WORKFLOW" && grep -qF -- "$CRITICAL" "$GATE"; then
  echo "ok   poort en prompt gebruiken dezelfde markering voor een kritieke bevinding"
  passed=$((passed + 1))
else
  echo "FAIL de markering ${CRITICAL} staat niet in zowel ${WORKFLOW} als ${GATE}"
  failed=$((failed + 1))
fi

# Opruimen ná de review. Staat het ervoor, dan is een bevinding uit een geklapte
# run weg en is opnieuw pushen genoeg om de poort te laten vergeten.
snapshot_line=$(grep -n -- '- name: Snapshot the previous review' "$WORKFLOW" | cut -d: -f1)
review_line=$(grep -n -- '- name: Run Claude Code Review' "$WORKFLOW" | cut -d: -f1)
cleanup_line=$(grep -n -- '- name: Clean up the superseded review' "$WORKFLOW" | cut -d: -f1)
if [ -n "$snapshot_line" ] && [ -n "$review_line" ] && [ -n "$cleanup_line" ] &&
  [ "$snapshot_line" -lt "$review_line" ] && [ "$review_line" -lt "$cleanup_line" ]; then
  echo "ok   de momentopname staat vóór de review en het opruimen erna"
  passed=$((passed + 1))
else
  echo "FAIL ${WORKFLOW} ruimt niet in de volgorde momentopname, review, opruimen"
  failed=$((failed + 1))
fi

# Het opruimen mag alleen draaien als de review werkelijk gedraaid heeft; anders
# gooit een geklapte run de vorige bevinding alsnog weg.
if awk '/- name: Clean up the superseded review/ {inside = 1; next} /^      - name:/ {inside = 0} inside && /if: steps.claude-review.outputs.execution_file/ {found = 1} END {exit !found}' "$WORKFLOW"; then
  echo "ok   het opruimen hangt aan hetzelfde bewijs als de bewijsstap"
  passed=$((passed + 1))
else
  echo "FAIL de opruimstap in ${WORKFLOW} draait niet onder de conditie dat de review een uitvoerbestand opleverde"
  failed=$((failed + 1))
fi

# De poort eist dat de review iets op de PR zet. Dat werkt alleen als de prompt
# zegt dat de review zelf moet posten, ook zonder bevindingen.
if grep -qF -- 'gh pr comment ${{ needs.target.outputs.pr }}' "$WORKFLOW" &&
  grep -qF -- 'always, also when there is nothing to report' "$WORKFLOW"; then
  echo "ok   de prompt laat de review zelf een samenvatting posten, ook zonder bevindingen"
  passed=$((passed + 1))
else
  echo "FAIL de prompt in ${WORKFLOW} zegt niet dat de review altijd zelf een samenvatting post"
  failed=$((failed + 1))
fi

# De teksten uit de momentopname gaan als context de prompt in. Zonder die
# koppeling weet de volgende review niets van de vorige bevinding, en dan is
# blijven pushen genoeg om een blokkade kwijt te raken.
if grep -qF -- 'steps.snapshot.outputs.context' "$WORKFLOW"; then
  echo "ok   de vorige bevindingen gaan als context de prompt in"
  passed=$((passed + 1))
else
  echo "FAIL de prompt in ${WORKFLOW} krijgt de bevindingen van de vorige review niet mee"
  failed=$((failed + 1))
fi

echo
echo "${passed} geslaagd, ${failed} gefaald"
[ "$failed" -eq 0 ]
