#!/usr/bin/env bash
# Merge gate: blokkeert tot de Claude-review van deze workflow-run klaar is, en
# blijft rood als daar een `🔴 **Critical**` uit kwam.
#
# Nul comments bewijst niets: dat is net zo goed "draait nog" als "niets
# gevonden". Of er gereviewd is leest de poort daarom van de review-job zelf, en
# pas daarna kijkt hij naar de bevindingen. Die volgorde wordt afgedwongen, niet
# aangenomen: zonder `review_proven` weigert `assert_no_critical_finding`.
#
# Alles wat de poort als feit behandelt haalt hij zelf op. Het workflowbestand
# staat in de pull request, dus wat dat bestand meegeeft is geschreven door de
# auteur van de wijziging die onder review staat.
#
# Zie docs/src/content/docs/operations/ci-cd.md ("The Claude review gate") voor
# het volledige ontwerp en wat de poort niet afdekt.
set -uo pipefail

: "${REPO:?REPO is verplicht}"
: "${RUN_ID:?RUN_ID is verplicht}"
: "${PR_NUMBER:?PR_NUMBER is verplicht}"

MAX_WAIT_SECONDS="${MAX_WAIT_SECONDS:-2100}"
POLL_SECONDS="${POLL_SECONDS:-20}"
# How long to keep looking once it is established that this run's own review
# will not deliver, before blocking. Long enough for a manual run started just
# before a rerun of this job to show up; a PR can only shorten it to its own
# disadvantage.
NO_REVIEW_GRACE_SECONDS="${NO_REVIEW_GRACE_SECONDS:-120}"
# How long a draft must stay a draft before the gate calls the review not
# applicable; see the draft check below.
DRAFT_SETTLE_SECONDS="${DRAFT_SETTLE_SECONDS:-30}"
GITHUB_STEP_SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

# Vaste namen, bewust geen env-variabelen: ze staan in het workflowbestand dat de
# PR meebrengt, en een PR die ze mocht overschrijven kon de poort naar een stap
# wijzen die altijd draait.
readonly JOB_NAME='claude-review'
readonly PROOF_STEP='Record that the review ran'
readonly WORKFLOW_FILE='.github/workflows/claude-code-review.yml'
# The job that decides, before `claude-review`, whether there is anything to
# review. Once it is done, a missing `claude-review` is not coming any more.
readonly TARGET_JOB='Resolve the commit to review'
readonly WORKFLOW_NAME='Claude Code Review'
# Letterlijk zoals de prompt in ${WORKFLOW_FILE} hem voorschrijft; de testsuite
# bindt de twee. Alleen Critical blokkeert: Significant zegt zelf al
# "waarschijnlijk".
readonly CRITICAL_MARKER='🔴 **Critical**'
# `claude.yml` antwoordt als dezelfde bot op `@claude`. Zonder deze eis zet zo'n
# antwoord dat de markering aanhaalt de poort op rood.
readonly REVIEW_TAG='<!-- claude-review -->'
readonly REVIEW_AUTHOR='claude[bot]'

summary() { printf '%s\n' "$1" >>"$GITHUB_STEP_SUMMARY"; }

not_applicable() {
  echo "::notice title=Claude review gate::niet van toepassing — $1"
  summary "### Claude review gate: niet van toepassing"
  summary ""
  summary "$1"
  exit 0
}

blocked() {
  echo "::error title=Claude review gate::$1"
  summary "### Claude review gate: geblokkeerd"
  summary ""
  summary "$1"
  exit 1
}

# stderr apart houden: een waarschuwing van `gh` op een verder geslaagde aanroep
# zou de payload anders onparseerbaar maken, en dat kwam dan naar buiten als een
# uitspraak over de review in plaats van als leesprobleem.
gh_stderr=$(mktemp "${TMPDIR:-/tmp}/gate-stderr.XXXXXX")
trap 'rm -f "$gh_stderr"' EXIT

if ! pr=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}" 2>"$gh_stderr") ||
  ! head_sha=$(jq -er '.head.sha' <<<"$pr" 2>/dev/null); then
  blocked "Pull request ${PR_NUMBER} in ${REPO} is niet op te halen, dus er valt niets over de review vast te stellen. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi

pr_field() { jq -r "${1} // \"\"" <<<"$pr"; }

# De koppeling tussen de drie coördinaten: run ${RUN_ID} moet over dezelfde
# commit gaan als pull request ${PR_NUMBER}. Zonder die toets kan een PR de poort
# naar de run van een andere, al gereviewde PR wijzen en is elke uitspraak
# hieronder waar — over die andere PR.
if ! run=$(gh api "repos/${REPO}/actions/runs/${RUN_ID}" 2>"$gh_stderr") ||
  ! run_head=$(jq -er '.head_sha' <<<"$run" 2>/dev/null); then
  blocked "Workflow-run ${RUN_ID} is niet op te halen, dus of deze run over pull request ${PR_NUMBER} gaat is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi
if [ "$run_head" != "$head_sha" ]; then
  blocked "Workflow-run ${RUN_ID} draait op commit \`${run_head}\`, terwijl pull request ${PR_NUMBER} op \`${head_sha}\` staat. De poort zou dan over een andere wijziging rapporteren dan er wordt gemerged. Draai de workflow opnieuw op de actuele commit."
fi

if [ "$(jq -r '.head.repo' <<<"$pr")" = "null" ]; then
  not_applicable "De head-repository van deze PR bestaat niet meer, dus er valt geen commit te reviewen en te mergen valt er ook niets."
fi

if [ "$(pr_field '.head.repo.full_name')" != "$REPO" ]; then
  not_applicable "Deze PR komt uit een andere repository (een fork). Zulke PR's krijgen geen secrets, dus \`CLAUDE_CODE_OAUTH_TOKEN\` ontbreekt en \`${JOB_NAME}\` draait daar niet. Review deze wijziging met de hand voordat je merget."
fi

# A draft is not mergeable and needs no review yet. But the flag is read at one
# moment, and "ready for review" can follow a push within seconds. The run for
# that push then skips its review, and the ready_for_review run can carry the
# commit from before the push (PR 1801). A green "not applicable" here would
# then be the only verdict on the head commit, without any review. So the flag
# has to hold for DRAFT_SETTLE_SECONDS; if the PR turns ready meanwhile, the
# gate judges it like any other.
if [ "$(pr_field '.draft')" = "true" ]; then
  settle_until=$(($(date +%s) + DRAFT_SETTLE_SECONDS))
  while [ "$(pr_field '.draft')" = "true" ] && [ "$(date +%s)" -lt "$settle_until" ]; do
    sleep "$((POLL_SECONDS < 1 ? 1 : POLL_SECONDS < 5 ? POLL_SECONDS : 5))"
    # A failed read keeps the last answer: that is what the gate knew anyway.
    if refetched=$(gh api "repos/${REPO}/pulls/${PR_NUMBER}" 2>/dev/null) &&
      jq -e '.head.sha' <<<"$refetched" >/dev/null 2>&1; then
      pr="$refetched"
    fi
  done
  if [ "$(pr_field '.draft')" = "true" ]; then
    not_applicable "Deze PR staat op draft. \`${JOB_NAME}\` draait pas bij \"ready for review\", en een draft is niet mergebaar."
  fi
  if [ "$(pr_field '.head.sha')" != "$head_sha" ]; then
    blocked "Pull request ${PR_NUMBER} is intussen ready for review, maar staat nu op commit \`$(pr_field '.head.sha')\`, terwijl deze run over \`${head_sha}\` gaat. Deze poort rapporteert alleen over \`${head_sha}\`; de poort op de nieuwe commit beoordeelt die."
  fi
  echo "Pull request ${PR_NUMBER} is intussen ready for review; de poort beoordeelt commit ${head_sha}."
fi

if [ "$(pr_field '.user.login')" = "dependabot[bot]" ]; then
  not_applicable "Deze PR komt van dependabot. Die loopt via de \`claude-dependabot\`-workflow, niet via \`${JOB_NAME}\`."
fi

# De claude-code-action weigert te draaien zodra het workflowbestand afwijkt van
# dat op de default branch: zij stapt uit met conclusie `success` zonder ook maar
# iets te reviewen ("Exiting due to workflow validation skip"). Gemeten op PR
# 1157: veertien seconden, groen, geen review.
#
# Die vergelijking maakt de poort zelf, vóór ze op de review gaat wachten. Dat
# scheelt niet alleen een halfuur wachten op een review die er niet komt; het is
# ook wat de rest van deze poort draagt. Zijn de twee versies gelijk, dan is het
# workflowbestand dat deze run draait dat van de default branch, en niet iets wat
# met de PR is meegekomen. Pas dan zegt de uitkomst van een stap in die workflow
# iets.
#
# Vergeleken wordt de merge-ref, niet de head-commit: een `pull_request`-run
# draait het workflowbestand zoals het in de test-merge van PR en base staat. Een
# PR die het bestand zelf niet aanraakt heeft daar dus de versie van de base
# staan, ook als de branch al weken achterloopt. Op `head.sha` vergelijken zou
# precies die achterlopers rood zetten terwijl hun review gewoon draaide.
blob_sha() {
  local ref payload
  ref="$1"
  payload=$(gh api "repos/${REPO}/contents/${WORKFLOW_FILE}?ref=${ref}" 2>"$gh_stderr") || return 1
  jq -er '.sha' <<<"$payload" 2>/dev/null
}

missing_file() { grep -qE 'HTTP 404|Not Found' "$gh_stderr"; }

default_branch=$(pr_field '.base.repo.default_branch')
[ -n "$default_branch" ] || blocked "De default branch van ${REPO} staat niet in het antwoord van de pull-request-API, dus \`${WORKFLOW_FILE}\` is niet te vergelijken. Draai deze job opnieuw."

merge_ref="refs/pull/${PR_NUMBER}/merge"
if ! run_workflow=$(blob_sha "$merge_ref"); then
  if missing_file; then
    blocked "\`${WORKFLOW_FILE}\` bestaat niet op \`${merge_ref}\`, de samenvoeging van deze PR met de base-branch. Zonder dat bestand is er geen review-workflow en kan de poort niets vaststellen. Opnieuw draaien helpt hier niet; zet het bestand terug of los het merge-conflict op."
  fi
  blocked "\`${WORKFLOW_FILE}\` is niet op te halen voor \`${merge_ref}\`, dus of de review-actie op deze PR draait is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi
if ! base_workflow=$(blob_sha "$default_branch"); then
  if missing_file; then
    blocked "\`${WORKFLOW_FILE}\` bestaat niet op branch \`${default_branch}\`. De review-actie valideert daartegen, dus zonder dat bestand draait zij nergens. Opnieuw draaien helpt hier niet."
  fi
  blocked "\`${WORKFLOW_FILE}\` is niet op te halen voor branch \`${default_branch}\`, dus of de review-actie op deze PR draait is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
fi

if [ "$run_workflow" != "$base_workflow" ]; then
  blocked "\`${WORKFLOW_FILE}\` is in deze run een andere versie dan die op \`${default_branch}\`: de blob op \`${merge_ref}\` is \`${run_workflow}\`, die op \`${default_branch}\` is \`${base_workflow}\`. De review-actie valideert het workflowbestand tegen de default branch en stapt bij een verschil uit zonder te reviewen, dus deze PR krijgt geen automatische review en de poort kan er niet voor instaan. Wijzigt deze PR dat bestand, dan moet een mens de wijziging nalopen."
fi

# A manual run (workflow_dispatch) reviews this PR from the default branch, so
# its own head_sha is a commit on that branch and not ${head_sha}. What it
# reviewed is recorded in the name of its review job; see find_dispatch_review.
readonly dispatch_title="${WORKFLOW_NAME}: PR #${PR_NUMBER}"
readonly dispatch_job_name="${JOB_NAME} (PR #${PR_NUMBER} @ ${head_sha})"
readonly retrigger="Start een review van deze commit met \`gh workflow run \"${WORKFLOW_NAME}\" -f pr=${PR_NUMBER}\` en draai daarna deze job opnieuw met \`gh run rerun ${RUN_ID} --failed\`: de poort neemt die review dan over. Of zet de PR op draft en daarna weer op ready for review."

# Fills `dispatch_job` and `dispatch_run_url` with the review job of the newest
# manual run that reviewed ${head_sha} for this PR, and leaves them empty if
# there is none.
#
# Why such a job may stand in for this run's own. A run is only considered if
# its event is workflow_dispatch and it ran ${WORKFLOW_FILE} from
# ${default_branch} in this repository; GitHub sets those fields, not the
# workflow. On top of that, the file at the run's own head_sha must be the very
# blob that is on ${default_branch} now. Then the code that ran is the reviewed
# default-branch copy, whatever ref name it was started with. That code takes
# the commit to review from the API, never from the dispatch input, and writes
# it into the name of its review job. A job name comes from the workflow file
# alone: neither the PR, the person who started the run, nor the review itself
# can set it. A job called ${dispatch_job_name} in such a run is therefore a
# review of exactly ${head_sha}, and from there the same proof step and the same
# Critical check apply as to this run's own job. The run title is matched too,
# but only to keep the number of API calls down; it proves nothing.
dispatch_job=''
dispatch_run_url=''
dispatch_note=''
trusted_dispatch_heads=' '
find_dispatch_review() {
  local runs candidates id sha html jobs found blob
  dispatch_job=''
  dispatch_run_url=''
  dispatch_note=''
  if ! runs=$(gh api "repos/${REPO}/actions/workflows/${WORKFLOW_FILE##*/}/runs?event=workflow_dispatch&branch=${default_branch}&per_page=100" 2>/dev/null) ||
    ! candidates=$(jq -er --arg wf "$WORKFLOW_FILE" --arg branch "$default_branch" \
      --arg repo "$REPO" --arg title "$dispatch_title" '
        [.workflow_runs[]
         | select(.event == "workflow_dispatch" and .path == $wf
                  and .head_branch == $branch
                  and .head_repository.full_name == $repo
                  and .display_title == $title)]
        | sort_by(.id) | reverse | .[:5] | map("\(.id) \(.head_sha) \(.html_url)") | join("\n")' <<<"$runs" 2>/dev/null); then
    last_error="de handmatig gestarte runs van \`${WORKFLOW_FILE}\` waren niet op te halen of niet te lezen"
    return
  fi

  while read -r id sha html; do
    [ -n "$id" ] || continue
    if ! jobs=$(gh api "repos/${REPO}/actions/runs/${id}/jobs?filter=all&per_page=100" 2>/dev/null); then
      last_error="de jobs van handmatige run ${id} waren niet op te halen"
      return
    fi
    found=$(jq -c --arg name "$dispatch_job_name" '[.jobs[] | select(.name == $name)] | max_by(.id) // empty' <<<"$jobs" 2>/dev/null)
    [ -n "$found" ] || continue

    case "$trusted_dispatch_heads" in
    *" ${sha} "*) ;;
    *)
      if ! blob=$(blob_sha "$sha"); then
        last_error="\`${WORKFLOW_FILE}\` op commit ${sha} van handmatige run ${id} was niet op te halen"
        continue
      fi
      if [ "$blob" != "$base_workflow" ]; then
        dispatch_note=" Handmatige run ${html} reviewde deze commit wel, maar met een andere versie van \`${WORKFLOW_FILE}\` dan die nu op \`${default_branch}\` staat, dus die telt niet."
        continue
      fi
      trusted_dispatch_heads="${trusted_dispatch_heads}${sha} "
      ;;
    esac

    # A manual review that ended without a usable result is not taken over,
    # and neither is an older one behind it: keep looking within the grace
    # period, since a newer manual run may be about to replace it.
    if [ "$(jq -r '.status // ""' <<<"$found")" = completed ]; then
      case "$(jq -r '.conclusion // ""' <<<"$found")" in
      success | neutral | '') ;;
      *)
        dispatch_note=" Handmatige run ${html} reviewde deze commit, maar eindigde op \`$(jq -r '.conclusion' <<<"$found")\`."
        return
        ;;
      esac
    fi
    dispatch_job="$found"
    dispatch_run_url="$html"
    return
  done <<<"$candidates"
}

# Fills `sibling_job` and `sibling_run_url` with the review job of the newest
# other pull_request run of this workflow on ${head_sha}, if one is queued,
# running or done with a usable result.
#
# The common case: a push to a draft and "ready for review" seconds later. The
# push run skips its review, and the ready_for_review run reviews the same
# commit. Without this, the push run's gate would block while that review runs.
#
# Why such a job may stand in. A pull_request run executes the workflow file as
# it stands in the test merge of the PR and its base at that moment, not the
# copy this gate compared. That copy is only known to be trusted when the PR
# does not touch the file: then every test merge with ${default_branch} holds a
# default-branch version of it, and the action validates that version against
# the default branch itself. So a sibling counts only when the PR's base is
# ${default_branch} and the file at ${head_sha} is the same blob as at the merge
# base of the PR. A PR that changes the file already blocked above, unless its
# change equals what the default branch has now; this rule turns that case down
# too. A sibling for another PR on the same commit is not ruled out here, but
# its comments land on that PR, and the gate only counts what is written on
# this one.
sibling_job=''
sibling_run_url=''
sibling_trust=''
find_sibling_review() {
  local runs candidates id html jobs found compare merge_base head_blob base_blob
  sibling_job=''
  sibling_run_url=''

  if [ -z "$sibling_trust" ]; then
    if [ "$(pr_field '.base.ref')" != "$default_branch" ]; then
      sibling_trust=no
    elif ! compare=$(gh api "repos/${REPO}/compare/${default_branch}...${head_sha}?per_page=1" 2>/dev/null) ||
      ! merge_base=$(jq -er '.merge_base_commit.sha' <<<"$compare" 2>/dev/null) ||
      ! head_blob=$(blob_sha "$head_sha") || ! base_blob=$(blob_sha "$merge_base"); then
      # Not knowing only means no sibling is used this poll, which can make
      # the gate red but never green; try again on the next poll.
      return
    elif [ "$head_blob" = "$base_blob" ]; then
      sibling_trust=yes
    else
      sibling_trust=no
    fi
  fi
  [ "$sibling_trust" = yes ] || return

  if ! runs=$(gh api "repos/${REPO}/actions/workflows/${WORKFLOW_FILE##*/}/runs?event=pull_request&head_sha=${head_sha}&per_page=100" 2>/dev/null) ||
    ! candidates=$(jq -er --arg wf "$WORKFLOW_FILE" --arg sha "$head_sha" \
      --arg repo "$REPO" --argjson self "$RUN_ID" '
        [.workflow_runs[]
         | select(.event == "pull_request" and .path == $wf
                  and .head_sha == $sha
                  and .head_repository.full_name == $repo
                  and .id != $self)]
        | sort_by(.id) | reverse | .[:5] | map("\(.id) \(.html_url)") | join("\n")' <<<"$runs" 2>/dev/null); then
    last_error="de andere runs van \`${WORKFLOW_FILE}\` op commit ${head_sha} waren niet op te halen of niet te lezen"
    return
  fi

  while read -r id html; do
    [ -n "$id" ] || continue
    if ! jobs=$(gh api "repos/${REPO}/actions/runs/${id}/jobs?filter=all&per_page=100" 2>/dev/null); then
      last_error="de jobs van run ${id} waren niet op te halen"
      return
    fi
    found=$(jq -c --arg name "$JOB_NAME" '[.jobs[] | select(.name == $name)] | max_by(.id) // empty' <<<"$jobs" 2>/dev/null)
    [ -n "$found" ] || continue
    # A sibling that skipped or ended without a result is no help; an older
    # one may still be.
    if [ "$(jq -r '.status // ""' <<<"$found")" = completed ]; then
      case "$(jq -r '.conclusion // ""' <<<"$found")" in
      success | neutral | '') ;;
      *) continue ;;
      esac
    fi
    sibling_job="$found"
    sibling_run_url="$html"
    return
  done <<<"$candidates"
}

echo "Wachten op de review van commit ${head_sha} in workflow-run ${RUN_ID} (max ${MAX_WAIT_SECONDS}s)."
deadline=$(($(date +%s) + MAX_WAIT_SECONDS))
job=''
status=''
last_error=''
# Set once it is established that this run's own review will not deliver:
# `absent`, or the conclusion it ended on.
own_gone=''
own_url=''
no_review_since=''
review_run="workflow-run ${RUN_ID}"

# There is no review to wait for: this run's own will not deliver, and no
# manual run reviewed this commit. Block now with what to do, rather than after
# the long wait that is meant for a review that is still running.
no_review() {
  local why
  case "$own_gone" in
  absent) why="\`${JOB_NAME}\` is in workflow-run ${RUN_ID} niet gestart, en \`${TARGET_JOB}\` is al klaar" ;;
  skipped) why="\`${JOB_NAME}\` is overgeslagen in workflow-run ${RUN_ID}; de reden staat in de job \`${TARGET_JOB}\`" ;;
  *) why="\`${JOB_NAME}\` eindigde op \`${own_gone}\` in workflow-run ${RUN_ID}" ;;
  esac
  blocked "${why}. Er is ook geen andere run en geen handmatig gestarte run die commit \`${head_sha}\` reviewt, dus er komt geen review waar de poort op kan wachten.${dispatch_note} ${retrigger} ${own_url}"
}

# `filter=all` is essentieel: het run-id blijft gelijk over attempts heen, dus na
# "Re-run this job" op alleen de poort zit `claude-review` niet in de joblijst van
# de nieuwste attempt. Over meerdere attempts is de volgorde niet gespecificeerd,
# vandaar `max_by(.id)`.
while :; do
  if response=$(gh api "repos/${REPO}/actions/runs/${RUN_ID}/jobs?filter=all&per_page=100" 2>/dev/null); then
    last_error=''
  else
    last_error="de jobs van workflow-run ${RUN_ID} waren niet op te halen"
    echo "API-aanroep mislukt, opnieuw proberen."
    response='{"jobs":[]}'
  fi

  own=$(jq -c --arg name "$JOB_NAME" '[.jobs[] | select(.name == $name)] | max_by(.id) // empty' <<<"$response" 2>/dev/null)
  own_gone=''
  if [ -n "$own" ]; then
    own_url=$(jq -r '.html_url // ""' <<<"$own" 2>/dev/null)
    if [ "$(jq -r '.status // ""' <<<"$own" 2>/dev/null)" = completed ]; then
      case "$(jq -r '.conclusion // ""' <<<"$own" 2>/dev/null)" in
      success | neutral | '') ;;
      *) own_gone=$(jq -r '.conclusion' <<<"$own") ;;
      esac
    fi
  elif [ -z "$last_error" ] &&
    [ "$(jq -r --arg name "$TARGET_JOB" '[.jobs[] | select(.name == $name)] | max_by(.id) | .status // ""' <<<"$response" 2>/dev/null)" = completed ]; then
    own_gone=absent
  fi

  if [ -z "$own_gone" ]; then
    job="$own"
    review_run="workflow-run ${RUN_ID}"
    no_review_since=''
  else
    find_sibling_review
    job="$sibling_job"
    if [ -n "$job" ]; then
      review_run="run ${sibling_run_url} op dezelfde commit"
    else
      find_dispatch_review
      job="$dispatch_job"
      [ -n "$job" ] && review_run="handmatige run ${dispatch_run_url}"
    fi
    if [ -n "$job" ]; then
      no_review_since=''
    elif [ -z "$last_error" ]; then
      # Only on established facts: a failed API call proves nothing either way.
      now=$(date +%s)
      no_review_since="${no_review_since:-$now}"
      if [ "$now" -ge $((no_review_since + NO_REVIEW_GRACE_SECONDS)) ] || [ "$now" -ge "$deadline" ]; then
        no_review
      fi
    fi
  fi

  status=''
  [ -n "$job" ] && status=$(jq -r '.status // ""' <<<"$job" 2>/dev/null)

  # Vlak na het afronden geeft de jobs-API een job soms nog zonder stappen;
  # een paar seconden later staan ze er wel. Zonder stappen is het bewijs nog
  # niet te lezen, dus dat telt als nog niet klaar en niet als een job zonder
  # bewijsstap. A skipped job never gets steps, so waiting for them would last
  # until the deadline (PR 1801).
  if [ "$status" = "completed" ]; then
    [ "$(jq -r '.conclusion // ""' <<<"$job" 2>/dev/null)" = skipped ] && break
    steps_pending=$(jq -r '(has("steps") | not) or (.steps == [])' <<<"$job" 2>/dev/null)
    [ "$steps_pending" != "true" ] && break
    status='completed, nog zonder stappen'
  fi

  if [ "$(date +%s)" -ge "$deadline" ]; then
    if [ -n "$last_error" ]; then
      blocked "Na ${MAX_WAIT_SECONDS}s is nog steeds niet vast te stellen of \`${JOB_NAME}\` klaar is: ${last_error}. Dat is geen uitspraak over de review zelf; draai deze job opnieuw."
    fi
    if [ -z "$job" ]; then
      blocked "Na ${MAX_WAIT_SECONDS}s zit er geen job \`${JOB_NAME}\` in workflow-run ${RUN_ID}. De review is niet gestart; start de hele workflow opnieuw via de Actions-tab (\"Re-run all jobs\")."
    fi
    if [ "$status" = 'completed, nog zonder stappen' ]; then
      blocked "\`${JOB_NAME}\` is afgerond, maar na ${MAX_WAIT_SECONDS}s geeft de jobs-API nog geen stappen, dus het bewijs dat er gereviewd is valt niet te lezen. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. $(jq -r '.html_url // ""' <<<"$job")"
    fi
    blocked "Na ${MAX_WAIT_SECONDS}s is \`${JOB_NAME}\` in ${review_run} nog niet klaar (status \`${status}\`). Draai deze job opnieuw zodra de review af is."
  fi

  echo "Nog niet klaar (status '${status:-geen job gevonden}' in ${review_run}). Volgende poging over ${POLL_SECONDS}s."
  sleep "$POLL_SECONDS"
done

conclusion=$(jq -r '.conclusion // "none"' <<<"$job")
url=$(jq -r '.html_url // ""' <<<"$job")
# De starttijd van de job scheidt wat déze review schreef van wat er van een
# vorige is blijven staan.
job_started=$(jq -r '.started_at // ""' <<<"$job" 2>/dev/null)

# Een groene job is geen bewijs dat er gereviewd is. Dat bewijs komt uit de
# review-job zelf: een stap die alleen draait als de actie `execution_file` heeft
# gezet, en dat doet zij pas nadat de CLI werkelijk gedraaid heeft. Die stap
# staat in het job-object dat hierboven al is opgehaald, dus het bewijs is per
# constructie aan déze run en deze attempt gebonden, en het bestaat ook wanneer
# de review nul bevindingen had.
#
# Precies één stap met die naam, niet de laatste die zo heet: twee stappen met
# dezelfde naam is geen situatie waarin de poort een winnaar hoort te kiezen.
review_proven=no

assert_review_ran() {
  local matches proof
  matches=$(jq -r --arg step "$PROOF_STEP" '(.steps // []) | map(select(.name == $step)) | length' <<<"$job" 2>/dev/null)

  case "${matches:-onbekend}" in
  0)
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar de job kent geen stap \`${PROOF_STEP}\`, terwijl \`${WORKFLOW_FILE}\` gelijk is aan de versie op \`${default_branch}\`. Of er gereviewd is, valt daarmee niet af te lezen. Controleer of die stap nog in \`${WORKFLOW_FILE}\` staat. ${url}"
    ;;
  1) ;;
  onbekend)
    blocked "De stappen van \`${JOB_NAME}\` zijn niet uit het antwoord van de jobs-API te lezen, dus of er gereviewd is, is onbekend. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. ${url}"
    ;;
  *)
    blocked "\`${JOB_NAME}\` kent ${matches} stappen met de naam \`${PROOF_STEP}\`. Welke daarvan het bewijs is, kan de poort niet uitmaken. Laat één zo'n stap staan in \`${WORKFLOW_FILE}\`. ${url}"
    ;;
  esac

  proof=$(jq -r --arg step "$PROOF_STEP" '(.steps // []) | map(select(.name == $step)) | .[0].conclusion // ""' <<<"$job" 2>/dev/null)
  case "$proof" in
  success)
    review_proven=yes
    return
    ;;
  skipped)
    # De workflow-validation-skip is hierboven al uitgesloten: het
    # workflowbestand is gelijk aan dat op de default branch. Blijft over dat de
    # actie geen `execution_file` opleverde, bijvoorbeeld doordat zij vroegtijdig
    # uitstapte of doordat een upgrade die output hernoemde.
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar de stap \`${PROOF_STEP}\` is in die job overgeslagen. Die stap draait alleen als de review-actie een uitvoerbestand oplevert, dus dat is uitgebleven. \`${WORKFLOW_FILE}\` is gelijk aan de versie op \`${default_branch}\`, dus de bekende workflow-validation-skip verklaart dit niet. Lees het log van de review-job, en ga na of een upgrade van de review-actie de output \`execution_file\` heeft hernoemd. ${url}"
    ;;
  '')
    blocked "De stap \`${PROOF_STEP}\` in \`${JOB_NAME}\` heeft geen leesbare conclusie, dus of er gereviewd is, is onbekend. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. ${url}"
    ;;
  *)
    blocked "\`${JOB_NAME}\` meldt \`${conclusion}\`, maar de stap \`${PROOF_STEP}\` eindigde op \`${proof}\`. Of er gereviewd is, is daarmee niet vastgesteld. Draai de review opnieuw: ${url}"
    ;;
  esac
}

# Drie plekken, want de body van de review draagt bevindingen die nergens anders
# staan. Alleen wat na de starttijd van de job is geschreven telt: ouder is van
# een vorige run, en een submitted review is niet te verwijderen.
#
# Het resultaat gaat via een globale en niet via `$(...)`: daarin zou `blocked`
# alleen de subshell beëindigen.
critical_at=''
# How many tagged items `claude[bot]` wrote since the job started, over all
# three places. A review that ran but wrote nothing is not a review anyone can
# read; see assert_no_critical_finding.
review_items=0
# A rerun of this whole run reviews again in this run; a review taken over
# from another run is redone the way it was started.
no_output_remedy() {
  case "$review_run" in
  "workflow-run ${RUN_ID}") echo "Draai de hele run opnieuw met \`gh run rerun ${RUN_ID}\`, of zet de PR op draft en daarna weer op ready for review." ;;
  *) echo "$retrigger" ;;
  esac
}
collect_critical() {
  local what="$1" path="$2" stamp="$3" payload found
  # `--slurp` bundelt de pagina's tot één array van arrays; zonder dat levert
  # `--paginate` meerdere losse documenten en telt `length` per pagina.
  if ! payload=$(gh api "repos/${REPO}/${path}?per_page=100" --paginate --slurp 2>"$gh_stderr"); then
    blocked "De ${what} van pull request ${PR_NUMBER} zijn niet op te halen, dus of de review een kritieke bevinding heeft achtergelaten is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. Foutmelding: $(cat "$gh_stderr")"
  fi
  # Geen leesbare lijst betekent niets te doorzoeken, en stilzwijgend groen is
  # dan precies fout.
  if ! jq -e 'type == "array" and (all(.[]; type == "array"))' <<<"$payload" >/dev/null 2>&1; then
    blocked "Het antwoord op de ${what} van pull request ${PR_NUMBER} is geen leesbare lijst, dus of de review een kritieke bevinding heeft achtergelaten is niet vastgesteld. Dat is geen uitspraak over de review zelf; draai deze job opnieuw."
  fi
  # Een item zonder bruikbaar tijdstempel valt buiten het venster en zou stil
  # worden overgeslagen: dat is dezelfde fail-open als "niet kunnen kijken",
  # alleen onzichtbaar. Een `PENDING` review is de uitzondering; die is nooit
  # ingediend en heeft dus terecht geen `submitted_at`.
  local zonder_tijd
  zonder_tijd=$(jq -r --arg author "$REVIEW_AUTHOR" --arg stamp "$stamp" \
    --arg tag "$REVIEW_TAG" '
    [.[][] | select(.user.login == $author)
           | select((.body // "") | contains($tag))
           | select((.state // "") != "PENDING")
           | select((.[$stamp] // "") == "")] | length' <<<"$payload" 2>/dev/null)
  if [ "${zonder_tijd:-onbekend}" != 0 ]; then
    blocked "In de ${what} van pull request ${PR_NUMBER} staat wat \`${REVIEW_AUTHOR}\` schreef zonder bruikbaar tijdstempel (\`${stamp}\`), dus of het van deze review is valt niet vast te stellen. Dat is geen uitspraak over de review zelf; draai deze job opnieuw."
  fi

  if ! found=$(jq -r --arg author "$REVIEW_AUTHOR" --arg since "$job_started" \
    --arg stamp "$stamp" --arg marker "$CRITICAL_MARKER" --arg tag "$REVIEW_TAG" '
      .[][]
      | select(.user.login == $author)
      | select((.body // "") | contains($tag))
      | select((.[$stamp] // "") >= $since)
      | select((.body // "") | contains($marker))
      | .html_url // "(zonder url)"' <<<"$payload" 2>/dev/null); then
    blocked "De ${what} van pull request ${PR_NUMBER} zijn niet te doorzoeken op \`${CRITICAL_MARKER}\`, dus of de review een kritieke bevinding heeft achtergelaten is niet vastgesteld. Draai deze job opnieuw."
  fi
  [ -n "$found" ] && critical_at="${critical_at}${found}"$'\n'

  local written
  if ! written=$(jq -r --arg author "$REVIEW_AUTHOR" --arg since "$job_started" \
    --arg stamp "$stamp" --arg tag "$REVIEW_TAG" '
      [.[][]
       | select(.user.login == $author)
       | select((.body // "") | contains($tag))
       | select((.[$stamp] // "") >= $since)] | length' <<<"$payload" 2>/dev/null); then
    blocked "De ${what} van pull request ${PR_NUMBER} zijn niet te tellen, dus of de review iets heeft achtergelaten is niet vastgesteld. Draai deze job opnieuw."
  fi
  review_items=$((review_items + written))
  return 0
}

# Pas hier telt een leeg antwoord als "niets gevonden". Daarvoor stond nog open of
# de review überhaupt gedraaid had, en dan is nul comments geen uitkomst maar een
# open venster: de review-job schrijft zijn comments aan het eind van de job, en
# daarvóór staat er niets.
assert_no_critical_finding() {
  local waar

  if [ "$review_proven" != yes ]; then
    blocked "Interne fout in de poort: de bevindingen zijn opgevraagd voordat vaststond dat \`${JOB_NAME}\` werkelijk gereviewd heeft. In die volgorde betekent \"geen kritieke bevinding\" niets. Dit is een fout in \`script/await-claude-review.sh\`, geen uitspraak over deze pull request."
  fi

  if [ -z "$job_started" ]; then
    blocked "De starttijd van \`${JOB_NAME}\` staat niet in het antwoord van de jobs-API, dus welke comments van deze review zijn en welke van een vorige is niet uit elkaar te houden. Dat is geen uitspraak over de review zelf; draai deze job opnieuw. ${url}"
  fi

  collect_critical "samenvattende review-comments" "issues/${PR_NUMBER}/comments" updated_at
  collect_critical "inline review-comments" "pulls/${PR_NUMBER}/comments" updated_at
  collect_critical "ingediende reviews" "pulls/${PR_NUMBER}/reviews" submitted_at

  if [ -z "$critical_at" ]; then
    # The proof step only says the CLI ran. Whether the review was posted is
    # up to the model: the action posts nothing itself in this mode, and on
    # 23 October several runs ended green after 15 seconds without a word on
    # the PR (PR 1779). The prompt demands a summary comment even when there
    # is nothing to report, so its absence means the review is not there.
    if [ "$review_items" -eq 0 ]; then
      blocked "\`${JOB_NAME}\` is afgerond en de review-actie heeft gedraaid, maar \`${REVIEW_AUTHOR}\` heeft sinds de start van die job (\`${job_started}\`) niets op pull request ${PR_NUMBER} gezet met de regel \`${REVIEW_TAG}\`, in samenvatting, inline comments of ingediende reviews; een comment zonder die regel telt niet. De prompt vraagt die samenvatting ook als er niets te melden is, dus er is geen review om op af te gaan. $(no_output_remedy) ${url}"
    fi
    return
  fi
  waar=$(printf '%s' "$critical_at" | paste -sd' ' -)

  blocked "De review heeft op commit \`${head_sha}\` een bevinding met \`${CRITICAL_MARKER}\` achtergelaten: ${waar}. Repareer wat er staat en push opnieuw: de review schrijft zijn comment bij elke run over, dus een bevinding die weg is, is daarna ook uit de poort weg. ${url}"
}

case "$conclusion" in
success | neutral)
  assert_review_ran
  assert_no_critical_finding
  echo "::notice title=Claude review gate::review afgerond (${conclusion})"
  summary "### Claude review gate: groen"
  summary ""
  summary "\`${JOB_NAME}\` is afgerond voor commit \`${head_sha}\` met conclusie \`${conclusion}\`, in ${review_run}."
  summary ""
  summary "Wat deze check bewijst: \`${WORKFLOW_FILE}\` is gelijk aan de versie op \`${default_branch}\`, de job \`${JOB_NAME}\` is in ${review_run} afgerond met conclusie \`${conclusion}\`, de stap \`${PROOF_STEP}\` is in die job gedraaid, dus de review-actie heeft werkelijk gereviewd, \`${REVIEW_AUTHOR}\` heeft sinds de start van die job ${review_items} keer iets met \`${REVIEW_TAG}\` geschreven, en daarin staat geen \`${CRITICAL_MARKER}\`. Wat hij niet bewijst: dat de bevindingen deugen, dat ze zijn verwerkt, of dat een bevinding van lagere ernst er niet toe doet — daar kijkt de poort niet naar. Dat blijft mensenwerk."
  summary ""
  summary "[Bekijk de review-run](${url})"
  exit 0
  ;;
# Not expected: a skipped own job sends the loop looking elsewhere, and a job
# taken over from another run is never a skipped one. Kept as a guard, so an
# unforeseen path blocks instead of reaching the green branch.
skipped)
  blocked "\`${JOB_NAME}\` is overgeslagen in ${review_run}, terwijl deze PR geen fork, geen draft en niet van dependabot is. Er is dus geen review. ${retrigger} ${url}"
  ;;
*)
  blocked "\`${JOB_NAME}\` eindigde op \`${conclusion}\` in ${review_run}. Er is geen bruikbare review. ${retrigger} ${url}"
  ;;
esac
