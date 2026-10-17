---
title: "Deployment"
description: "How components are built and deployed to ZAD (RIG/Quattro/rijksapps) through GitHub Actions."
---

All components are deployed to ZAD (RIG/Quattro/rijksapps) via GitHub Actions. Docker images are pushed to GitHub Container Registry (GHCR).

## How deployment works

### On pull request

A PR only builds and deploys when it carries the `deploy:preview` label. Without that label nothing is built, because a preview costs about half of the repository's runner budget and no required check depends on it. Add the label and the build starts, whether the PR was opened a minute or a month ago:

1. Changed components are detected automatically
2. Docker images are built and pushed to `ghcr.io/<owner>/regelrecht-{component}`, where `<owner>` is the repository owner in lowercase (now `minbzk`), tagged `pr-{N}` and `sha-{commit}`
3. A preview deployment named `pr{N}` is created on ZAD
4. The PR gets a comment with preview URLs

Every later commit on a labelled PR repeats this. Remove the label and the preview and its images are cleaned up.

Only changed components are rebuilt. Any component can also be forced to build by adding its `deploy:<component>` label to the PR (for example `deploy:editor`); those labels are additive and do nothing on their own, since `deploy:preview` is what opens the gate.

The gate is the presence of the label on the PR, checked on every event, not the kind of event. Before this was opt-in, building seven images and a preview environment for every PR accounted for about half of the repository's runner time, while nothing tests against the preview (`E2E (mocked)` runs against mocks). The runner budget goes to the merge queue instead, and a preview is something someone chooses.

Fork PRs never build, labels or not. They have no secrets and a read-only `GITHUB_TOKEN`, so the push to GHCR could not succeed.

### On merge to main

When a PR merges to main, production deployment runs:

1. The `ci-green` job (`script/require-ci-green.sh`) waits for the CI run of the merge commit and lets the builds start only when it is green. No run for that commit blocks as well; a missing CI run is not taken as a pass
2. All changed components are rebuilt with the merge commit SHA
3. Components are deployed to the `regelrecht` deployment on ZAD
4. Production URLs update within minutes

`deploy-preview` and `deploy-production` each deploy all components in a single ZAD task. ZAD lets a task give way to a newer task that covers the same deployment, so two tasks for one deployment side by side would push each other aside.

### On PR close, or when the label is removed

The `cleanup-preview` job in `deploy.yml` deletes the `pr{N}` deployment on ZAD with its GitHub environment and deployments, and asks the ZAD cleanup action to remove the PR's `pr-{N}` image tags. No preview keeps running that nobody looks at. Fork PRs are skipped, since they never built anything. All other image cleanup happens nightly, as described below.

## Cleanup

`scheduled-cleanup.yml` runs every night. It removes stale `pr{N}` ZAD deployments and GitHub environments, and then runs all image cleanup through one script, `script/ghcr-cleanup.mjs`, in one step. That script removes three kinds of image versions:

- `pr-` tagged versions of closed pull requests, including any the per-PR cleanup missed
- versions whose tags are all of the `sha-` form, that run nowhere and are older than a week
- untagged manifests that no tagged index points to any more

The three belong together because they share the same protection, and two cleaners side by side would not know each other's exceptions:

- Production runs on a `sha-` tag, not on `latest`. A cleaner that goes by tag shape alone would remove the image under the running deployment. The script therefore checks against what ZAD is running at that moment, and removes nothing at all, of any kind, if it cannot get that list.
- Untagged is not the same as garbage. Buildx enables provenance, so every push produces an OCI index carrying the tag plus two untagged children (the platform image and an attestation manifest). The script first builds the reference graph and deletes only untagged versions that do not occur in it. If one manifest lookup fails, every orphan in that package stays. The children of a running index are in that graph, so they are protected twice.

The ZAD cleanup action inventories GitHub environments, and so misses any ZAD deployment whose environment is already gone. `script/prune-orphaned-deployments.sh` works the other way round. Afterwards `script/check-preview-deployments.sh` and `script/check-preview-environments.sh` establish what is actually left, because the cleanup's own report says nothing about the outcome.

The reasoning behind each rule is in the header of the script concerned.

## Debugging a failed preview deploy

Tell a timeout apart from an error first. A message that the wait ran out ("Timed out after 900s waiting for the task; it may still be running") says only that ZAD took longer than the window. The deployment carries on, and the preview usually comes up a little later. Before the window went from 300 to 900 seconds, failed and successful runs overlapped completely in duration ([#1144](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/1144)), so a timeout on its own tells you nothing about the application.

An error with a status or an exception in it is the diagnostic case. Then:

1. Check the container logs: `zad logs <deployment>` (for example `zad logs pr429`)
2. Look for `ERROR` lines. Common causes are migration conflicts, missing environment variables and panics at startup
3. If the database is in a bad state (for example a migration checksum mismatch after renumbering), delete the preview deployment (`zad deployment delete <deployment>`) and re-trigger CI to get a fresh database
4. Do not retry blindly; find the cause first

One failure is harmless: `Could not extract URL from result` with `"status": "superseded"` in the JSON below it. ZAD let the task give way to a newer task covering the same deployment (a new push, or the cleanup of a closed PR). The newer task did the work, and re-running only this job is enough.

## Deployed components

| Component | Image | Production URL |
|-----------|-------|----------------|
| Editor | `regelrecht-editor` | `editor.regelrecht.rijks.app` |
| Admin | `regelrecht-admin` | `harvester-admin.regelrecht.rijks.app` |
| Harvester Worker | `regelrecht-harvester-worker` | (no web UI) |
| Enrich Worker | `regelrecht-enrich-worker` | (no web UI) |
| Pipeline API | `regelrecht-pipeline-api` | (no public URL; reached in-cluster) |
| Lawmaking | `regelrecht-lawmaking` | `lawmaking.regelrecht.rijks.app` |
| Demo | `regelrecht-demo` | `demo.regelrecht.rijks.app` |
| Docs | `regelrecht-docs` | `docs.regelrecht.rijks.app` + `regelrecht.rijks.app` (landing) |
| PoC portal | `regelrecht-poc` | `poc.regelrecht.rijks.app` |
| PoC napp | `regelrecht-poc-napp` | (internal; reached through the portal at `/napp/`) |
| PoC vakantieverlof | `regelrecht-poc-vakantieverlof` | (internal; reached through the portal at `/vakantieverlof/`) |
| Grafana | `regelrecht-grafana` | `grafana.regelrecht.rijks.app` (built on `main` only; not part of a preview) |

The docs image also serves `/roadmap`, a read-only rendering of the werkpakketten in `docs/src/content/roadmap/` and the JSON file in `docs/src/data/`. It is not a component of its own and has no write path: changing the roadmap means editing those files through a pull request, and every werkpakket page links to its own source on GitHub. The landing page links to it from the footer, next to the documentation and research links; it stays out of the main navigation, which covers the landing page's own sections.

## The demo

The demo (`frontend-demo/`) runs at `demo.regelrecht.rijks.app` as the ZAD component `demo` in the `regelrecht` deployment, with only `publish-on-web` on port 8000. It rolls out with `deploy-preview` and `deploy-production` when `script/deploy-filters.mjs` marks the `demo` component as changed. That happens for changes to the engine crate and the workspace crates it depends on, the workspace-wide Rust files (`packages/Cargo.toml`, `packages/Cargo.lock`, `rust-toolchain.toml`, `schema/`), `frontend-demo/`, `packages/frontend-shared/`, `corpus/demo/`, or `deploy/nginx/`.

The build is the `build-demo` job in `deploy.yml` (`image-name: regelrecht-demo`, `dockerfile: frontend-demo/Dockerfile`, `cache-scope: demo`). The image is covered by `scheduled-cleanup.yml`, which checks `sha-` tags against the running deployment before it deletes anything.

What to check after a change is easiest on a preview (label the PR `deploy:preview`): that the WASM engine loads (network tab: `wasm/pkg/*.wasm` served as `application/wasm`), the slides, the portals of Merijn and Claudia, and one application followed all the way into the case system. The two things in `frontend-demo/nginx.conf` that can break are the SPA fallback to `index.html` and the MIME type for `.wasm`.

The demo needs no secrets to run. With `DEMO_WHY_PASSWORD` and `CLAUDE_CODE_OAUTH_TOKEN` set on the `demo` component, the container also starts the backend of the "why" explanation next to nginx; see [Demo](/components/demo). Without them the button does not appear. Its build takes longer than the other frontends because of the Rust-to-WASM step. The `wasm-builder` stage pins the Rust image to the version in `rust-toolchain.toml` (a pre-commit test, `script/dockerfile-consistency.test.mjs`, fails when the two drift) and pins `wasm-bindgen-cli` to the version in `packages/Cargo.lock`; the build itself fails loudly when that second pair diverges.

## ZAD CLI

Use [`zad-cli`](https://github.com/RijksICTGilde/zad-cli) to manage deployments directly:

```bash
# Install / upgrade
uv tool install git+https://github.com/RijksICTGilde/zad-cli.git
uv tool upgrade zad-cli

# List deployments
zad deployment list

# Get logs
zad logs --deployment regelrecht --lines 50

# Add a new component
zad component add docs \
    --image ghcr.io/nederlandsedigitaledienst/regelrecht-docs:latest \
    --deployment regelrecht \
    --port 8000 \
    --service publish-on-web
```

Configure `ZAD_API_KEY` and `ZAD_PROJECT_ID` in `.env`.

## Required secrets

- `RIG_API_KEY` - API key for ZAD Operations Manager (configured in GitHub repository secrets)
- `GITHUB_TOKEN` - used for GHCR image pushes (provided automatically by GitHub Actions)
- `ADMIN_TOKEN` - token with admin rights on the repository, passed to the ZAD cleanup action in `cleanup-preview` and in `scheduled-cleanup.yml`, so it can delete preview GitHub environments

## Further reading

- [CI/CD Pipeline](./ci-cd) - the continuous integration checks that run before deployment
- [Contributing](./contributing) - the PR workflow that triggers deployment
