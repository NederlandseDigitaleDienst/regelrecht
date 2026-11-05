---
name: zad-wake
description: "Wake a sleeping PR preview (prN) on ZAD and wait until it is up. Use before a smoketest, Playwright check or check-deployment against a preview, when a preview URL returns a wake page or a 503, or when check-deployment sees no running pods."
user-invocable: true
allowed-tools: Bash, Read
---

# ZAD wake

A sleeping preview answers with a wake page or a 503, so a smoketest against
it fails for reasons unrelated to the change under test. When previews sleep,
for how long, and what the status fields mean is in
`docs/src/content/docs/operations/deployment.md`, under
"Sleeping previews".

## When to use it

- Before any smoketest, Playwright run or `check-deployment` on a `prN`
  preview. Wake it first, every time: waking an awake preview is a no-op.
- When a preview URL shows a wake page or returns a 503.
- When `check-deployment` finds empty logs for every component of a preview
  that did deploy.

## Wake the preview

Pass the deployment name (`pr` plus the PR number):

```bash
script/zad-wake.sh pr1704
```

The script reads `ZAD_API_KEY` (the project key, the variable zad-cli uses),
and falls back to `RIG_API_KEY`. Load it from `.env` first if it is not in the
environment. It posts a wake request, then polls the status until ZAD reports
the preview awake, for at most 10 minutes (`WAIT_SECONDS` changes that).

Exit 0 means the preview is awake, or that sleep-mode does not apply to that
deployment. Exit 1 means the preview is not awake yet, the API refused or
could not be reached, or the status response made no sense. The message says
which, with the HTTP code and the response body. Exit 2 is a usage error, such
as a missing key or a missing `jq`.

After exit 0, give the first request of the smoketest a retry or two. ZAD
reports a preview awake once its pods run, and an application can still be
busy starting behind them.

## Without the script

zad-cli has no sleep-mode command, so the script calls the Operations Manager
API with curl. Done by hand:

```bash
BASE=https://operations-manager.rig.prd1.gn2.quattro.rijksapps.nl
curl -sS -X POST -H "X-API-Key: $ZAD_API_KEY" "$BASE/api/sleep-mode/regel-k4c/pr1704/wake"
curl -sS -H "X-API-Key: $ZAD_API_KEY" "$BASE/api/sleep-mode/regel-k4c/pr1704/status"
```

In the status response, read `sleep_state`, not `state`; the docs page explains
why. The script stops waiting on `sleep_state: awake` or on
`state: ready`.

A 404 means the deployment does not exist; `zad deployment list` shows the
previews there are.

## What not to do

- Do not put an API key in a command you show, a commit or a PR comment.
- Do not redeploy a preview to wake it. A push to the PR does redeploy it and
  wakes it too, but it costs a full build for something one API call does.
