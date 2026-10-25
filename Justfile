# Justfile voor regelrecht
# Gebruik: just <task>

set dotenv-load := true

# CI uses RUSTFLAGS=-Dwarnings; ci_flags mirrors that for quality/test recipes
# but not for dev, where a warning should not stop the stack from starting.
# We also pass the mold link-arg here: an explicit RUSTFLAGS overrides the
# target.rustflags in packages/.cargo/config.toml, so without it these recipes
# would fall back to the slow default linker. (dev has no RUSTFLAGS, so it picks
# up mold straight from .cargo/config.toml.)
#
# The mold link-arg is x86_64-Linux-only, mirroring the [target.x86_64-unknown-linux-gnu]
# scoping in packages/.cargo/config.toml (and dev_needs_mold in script/dev-lib.sh)
# — otherwise quality/test recipes would force `-fuse-ld=mold` on macOS or
# aarch64 Linux, where nothing else asks for mold to be installed.
ci_flags := if os() + "-" + arch() == "linux-x86_64" {
    "RUSTFLAGS='-Dwarnings -C link-arg=-fuse-ld=mold'"
} else {
    "RUSTFLAGS=-Dwarnings"
}

# Default task - toon beschikbare tasks
default:
    @just --list

# --- WASM ---

# The wasm-bindgen CLI that matches packages/Cargo.lock, as a path on stdout
#
# Checked BEFORE the minutes-long cargo build of a recipe that uses it.
[private]
wasm-bindgen-cli:
    #!/usr/bin/env bash
    set -euo pipefail
    # `cargo install` puts it in $CARGO_HOME/bin, which is not always on the
    # PATH a recipe gets (cargo itself may be reachable through a symlink
    # elsewhere). The CLI must match the wasm-bindgen crate in Cargo.lock
    # exactly; the Docker build and CI enforce the same pin.
    locked=$(grep -A1 '^name = "wasm-bindgen"$' packages/Cargo.lock \
      | sed -n '/^version = /{s/^version = "\(.*\)"$/\1/p;q;}' || true)
    if [ -z "$locked" ]; then
      echo "could not read the wasm-bindgen version from packages/Cargo.lock" >&2
      exit 1
    fi
    bindgen=$(command -v wasm-bindgen || true)
    if [ -z "$bindgen" ] && [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen" ]; then
      bindgen="${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen"
    fi
    if [ -z "$bindgen" ]; then
      echo "wasm-bindgen not found. Install the version packages/Cargo.lock uses:" >&2
      echo "  cargo install wasm-bindgen-cli --version $locked --locked" >&2
      exit 1
    fi
    have=$("$bindgen" --version | awk '{print $2}')
    if [ "$have" != "$locked" ]; then
      echo "wasm-bindgen $have found at $bindgen, but packages/Cargo.lock uses $locked. Install the matching version:" >&2
      echo "  cargo install wasm-bindgen-cli --version $locked --locked --force" >&2
      exit 1
    fi
    echo "$bindgen"

# Build WASM module for browser use
wasm-build:
    #!/usr/bin/env bash
    set -euo pipefail
    bindgen=$({{just_executable()}} wasm-bindgen-cli)
    # Pin the target dir explicitly. A CLI --target-dir overrides any shared
    # [build] target-dir from `just dev-setup` (root .cargo/config.toml), so the
    # artifact always lands at packages/target — no metadata lookup (and no jq/
    # python3 dependency) needed, and it works with or without dev-setup.
    cargo build --manifest-path packages/engine/Cargo.toml --target wasm32-unknown-unknown --release --features wasm --target-dir packages/target
    "$bindgen" --target web --out-dir frontend/public/wasm/pkg packages/target/wasm32-unknown-unknown/release/regelrecht_engine.wasm
    # The demo runs the same engine in the browser; keep the two copies identical.
    mkdir -p frontend-demo/public/wasm/pkg && cp frontend/public/wasm/pkg/* frontend-demo/public/wasm/pkg/
    # The landing page runs the zorgtoeslag scenario in the visitor's browser
    # when the panel scrolls into view, so the amount it shows is computed there
    # and then rather than asserted. Same artifact again: one engine, three
    # places.
    mkdir -p docs/public/wasm/pkg && cp frontend/public/wasm/pkg/* docs/public/wasm/pkg/

# Copy the laws, the scenario and the canonical-grammar runner into the docs
# project, so the landing page can run the scenario in the visitor's browser.
landing-laws:
    ./script/landing-laws.sh

# --- Quality checks ---

# Check Rust formatting
format:
    cd packages && cargo fmt --check --all

# The features a deployed artefact or a `just` recipe actually turns on
# (`poc-chronolexografie/wasm` is the browser build of that poc).
# Deliberately not --all-features: that also builds `engine/wasm` and the two
# `test-utils` features. Those still get compiled by `just test`, which does run
# --all-features, so leaving them out here costs clippy coverage on that code
# and nothing else.
check_features := "regelrecht-engine/validate,regelrecht-corpus/annotation-validation,regelrecht-poc-chronolexografie/wasm"

# Run clippy lints
lint:
    cd packages && {{ci_flags}} cargo clippy --workspace --all-targets --features {{check_features}}

# Run cargo check
build-check:
    cd packages && {{ci_flags}} cargo check --workspace --features {{check_features}}

# Validate regulation YAML files
validate *FILES:
    script/validate.sh {{FILES}}

# Validate the demo corpus (schema + type check, RFC-036/RFC-037)
#
# `validate` without arguments walks corpus/regulation only, so the eighty demo
# laws would be seen by the pre-commit hook and by nothing else. This recipe
# hands them to the same binary, so a law that stops type-checking is caught by
# `just demo` and not first by a red scenario run.
[doc("Validate the demo corpus (schema + type check)")]
validate-demo:
    find corpus/demo/regulation -name '*.yaml' ! -name '.*' -print0 \
        | sort -z | xargs -0 script/validate.sh

# Validate note sidecar files (RFC-005, RFC-016)
# Orphaned/ambiguous notes and unknown tags are warnings, not errors.
[doc("Validate note sidecar files (RFC-005, RFC-016)")]
validate-annotations *FILES:
    script/validate-annotations.sh {{FILES}}

# Schema↔model conformance suite — proves the Rust law-model conforms to the
# canonical schema.json. Standalone helper; `just test` already runs it via
# --all-features. Needs the `validate` feature (jsonschema). See
# packages/engine/tests/conformance/README.md.
[doc("Prove the Rust law-model conforms to the canonical schema.json")]
conformance:
    cd packages && {{ci_flags}} cargo test -p regelrecht-engine --features validate --test conformance

# Pins the deploy filters against the real cargo graph. Node's built-in test
# runner, so no dependency for one file. Needs `cargo metadata`, not a build.
[doc("Check the deploy filters against the real cargo dependency graph")]
deploy-filters-test:
    node --test script/deploy-filters.test.mjs

# Pins the GHCR cleaner against its safety rules. It deletes manifests from a
# production registry and there is no undo, so every rule that decides "delete"
# versus "keep" is covered here. Same reasoning as deploy-filters-test: node's
# built-in runner, no dependency.
[doc("Check the GHCR cleanup safety rules")]
ghcr-cleanup-test:
    node --test script/ghcr-cleanup.test.mjs

# Pins the build-time asset precompression the editor image relies on. Same
# reasoning as deploy-filters-test: node's built-in runner, no dependency.
[doc("Check the editor's build-time asset precompression")]
precompress-test:
    node --test frontend/scripts/precompress.test.mjs

# Holds the Rust services, the nginx images and Grafana to the same header
# set. Same reasoning as deploy-filters-test: node's built-in runner, no
# dependency.
[doc("Check that every domain sends the same security headers")]
security-headers-test:
    node --test script/security-headers.test.mjs

# Pins the first-load guard against fixture dists. The guard itself only runs
# inside `npm run build`, so without this a broken chunk match surfaces as a
# failing image build — or worse, as a green one that ships echarts eagerly.
[doc("Check the editor's first-load guard")]
first-load-test:
    node --test frontend/scripts/check-first-load.test.mjs

# Draait het shellfragment van de `Test`-poort uit ci.yml met echte
# resultaatwaarden. Zonder dit is een voorganger die wel in `needs` staat maar
# niet gelezen wordt niet van een werkende poort te onderscheiden.
[doc("Check that the Test gate in ci.yml blocks on a failed predecessor")]
ci-gate-test:
    node --test script/ci-gate.test.mjs

# De merge queue deelt zijn lijst verplichte checks met de pull request. Een
# check die op de queue-branch niet rapporteert laat elke entry vastlopen, en
# dat gebeurt stil: de melding staat daar en niet op de pull request.
[doc("Check that every required check also reports in the merge queue")]
merge-queue-checks-test:
    node --test script/merge-queue-checks.test.mjs

# De guard die de per-component imports bij de gebruikte tags houdt. Zonder
# deze test is het verschil tussen een gerenderde tag en een tag die de
# documentatie alleen noemt niet vastgelegd, en dat verschil is precies waar hij
# eerder op omviel.
[doc("Check the design-system import guard")]
nldd-imports-test:
    node --test script/nldd-imports.test.mjs

# Een `slot="..."` die het component niet kent, is nergens een fout: het
# element blijft in de light-DOM, wordt nooit toegewezen en is 0x0. Zo stonden
# de persona-tags op het portaal en de startknop van de presentatie er wel,
# maar zag niemand ze. Deze guard laat de build erop omvallen.
[doc("Check that every nldd slot assignment exists")]
nldd-slots:
    node script/check-nldd-slots.mjs frontend-demo/src frontend/src frontend-lawmaking/src

[doc("Check the design-system slot guard")]
nldd-slots-test:
    node --test script/nldd-slots.test.mjs

# De Awb staat in twee corpora en moet daar hetzelfde zeggen. Het overzetten
# ging twee keer mis op een weggevallen laatste regel — één keer de termijn in
# artikel 6:8, waardoor de einddatum van de bezwaartermijn gelijk werd aan de
# bekendmakingsdatum. Geldige YAML, dus de schemacontrole zag het niet, en geen
# scenario raakt 6:8.
[doc("Check that the Awb says the same in both corpora")]
awb-parity-test:
    node --test script/awb-parity.test.mjs

# Welke organisatie een wet uitvoert staat in services.yaml en niet in het
# wetsbestand: het stuurt logo's, kleuren en groepering, en waarden als
# GEMEENTE_ROTTERDAM volgen uit geen wet. Een wet die in die kaart ontbreekt
# krijgt stil `service: null`: geen logo, geen kleur, geen foutmelding. Deze
# controle is wat de garantie vervangt die het oude veld gratis gaf.
[doc("Check that services.yaml covers every demo law")]
service-map-check:
    node frontend-demo/scripts/check-service-map.mjs

# Houdt de drie Rust-Dockerfiles bij de workspace: elke member wordt ge-COPYd
# of weggeknipt, de rust-tag volgt rust-toolchain.toml en elke binary-naam
# bestaat. Die drie zijn stringliteralen die verder niets nakijkt.
[doc("Check the Rust Dockerfiles against the cargo workspace")]
dockerfile-consistency-test:
    node --test script/dockerfile-consistency.test.mjs

# The browser jobs in ci.yml run in the Playwright image, whose browsers
# belong to one Playwright release. Its tag has to follow the version the
# lockfiles resolve, and Dependabot only bumps the lockfiles.
[doc("Check that the Playwright CI image matches the lockfiles")]
playwright-image-test:
    node --test script/playwright-image.test.mjs

# De poort die productie op een groene CI laat wachten. Zonder deze test is een
# poort die alles doorlaat niet te onderscheiden van een die werkt; `gh` staat
# hier als stub op PATH, dus er gaat geen verkeer naar GitHub.
[doc("Check the gate that holds production to a green CI")]
deploy-gate-test:
    script/require-ci-green.test.sh

# De controle die na de uitrol nakijkt of productie antwoordt. `curl` staat in
# de test als stub op PATH, dus er gaat geen verkeer naar buiten.
[doc("Check the post-deploy health check")]
deployed-urls-test:
    script/check-deployed-urls.test.sh
# De controle die telt wat de nachtelijke opruiming heeft achtergelaten. `gh`
# staat in de test als stub op PATH, dus er gaat geen verkeer naar GitHub.
[doc("Check the guard on leftover preview environments")]
preview-environments-test:
    script/check-preview-environments.test.sh

# Run all quality checks, exactly what CI runs. Needs Docker for the
# container-backed suites; on a machine without a daemon, swap `test` for
# `test-no-docker`.
[doc("Run all quality checks, exactly what CI runs (needs Docker)")]
check: format lint build-check validate validate-annotations deploy-filters-test ghcr-cleanup-test precompress-test security-headers-test first-load-test ci-gate-test merge-queue-checks-test nldd-imports-test nldd-slots nldd-slots-test dockerfile-consistency-test deploy-gate-test deployed-urls-test preview-environments-test advisories-report-test dev-preflight-test test

# --- Tests ---

# Crates with testcontainers-backed suites. Enumerates the exceptions, not the
# members: a new crate is covered by --workspace. The list is exactly the crates
# that dev-depend on regelrecht-pipeline/test-utils:
#   grep -l 'regelrecht-pipeline.*test-utils' packages/*/Cargo.toml
db_crates_exclude := "--exclude regelrecht-pipeline --exclude regelrecht-editor-api --exclude regelrecht-admin"
db_crates := "-p regelrecht-pipeline -p regelrecht-editor-api -p regelrecht-admin"

# Every recipe below is ONE cargo invocation, and that is load-bearing. Cargo
# unifies features over the selected packages, so two invocations with different
# selections in the same target dir resolve dependencies differently and rebuild
# each other's artefacts. Splitting `just test` into four invocations cost 240
# seconds of pure rebuild in CI, with regelrecht-engine compiled four times.

# Run every Rust test in the workspace. Needs Docker for the testcontainers
# suites. This is what `just check` runs.
[doc("Run every Rust test in the workspace (needs Docker)")]
test:
    cd packages && {{ci_flags}} cargo test --workspace --all-features

# The frontend unit tests and type checks, as the "Frontend tests" job in CI
# runs them. Not part of `check`, which needs no npm dependencies; this one
# installs the npm workspace on first use.
[doc("Run the frontend unit tests and type checks")]
frontend-test:
    #!/usr/bin/env bash
    set -euo pipefail
    source script/dev-lib.sh
    dev_ensure_deps . "frontend tests"
    npm test -w packages/frontend-shared
    npm run typecheck -w packages/frontend-shared
    npm test -w frontend
    npm run typecheck -w frontend
    npm test -w frontend-lawmaking
    npm run typecheck -w frontend-lawmaking
    npm test -w frontend-demo
    npm run typecheck -w frontend-demo
    npm test -w frontend-poc-chronolexografie

# Every test that needs no external services: the workspace minus the
# container-backed crates. For a machine without a Docker daemon, and the
# `unit` leg of the CI matrix.
[doc("Run every Rust test that needs no external services")]
test-no-docker:
    cd packages && {{ci_flags}} cargo test --workspace --all-features {{db_crates_exclude}}

# Only the container-backed crates: the `db` leg of the CI matrix.
test-db:
    cd packages && {{ci_flags}} cargo test {{db_crates}} --all-features

# Run Rust BDD tests
bdd:
    cd packages/engine && {{ci_flags}} cargo test --test bdd -- --nocapture

# BDD met de acties van elk artikel omgekeerd: de volgorde in het bestand mag
# geen uitkomst veranderen. CI draait de conformance- en demobucket zo ook.
bdd-reversed:
    cd packages/engine && {{ci_flags}} BDD_REVERSE_ACTIONS=1 cargo test --test bdd -- --nocapture
    cd packages/engine && {{ci_flags}} BDD_REVERSE_ACTIONS=1 BDD_BUCKET=corpus REGULATION_PATH="$(pwd)/../../corpus/demo/regulation" cargo test --test bdd -- --nocapture

# Bucket A over de democorpus: REGULATION_PATH wijst wetten en scenario's naar corpus/demo.
#
# De Awb-levensloop draait er achteraan, over hetzelfde corpus. Een scenario
# toetst één wet; de levensloop-test toetst wat de Awb aan elk besluit toevoegt
# (RFC-007, RFC-008) en daar kwam een fout in dit corpus aan het licht die geen
# enkel scenario zag: artikel 6:8 rekende met een afgekapte formule en gaf de
# bekendmakingsdatum terug als einddatum van de bezwaartermijn.
bdd-demo:
    cd packages/engine && {{ci_flags}} BDD_BUCKET=corpus REGULATION_PATH="$(pwd)/../../corpus/demo/regulation" cargo test --test bdd -- --nocapture
    cd packages/engine && {{ci_flags}} REGULATION_PATH="$(pwd)/../../corpus/demo/regulation" cargo test --test awb_lifecycle

# Start the demo and open it. One command for anyone who just wants to see it:
# it builds the engine to WASM, starts Vite and opens the browser on the
# presentation. Stop it with ctrl-c.
[doc("Start the demo and open it in a browser")]
demo: wasm-build
    #!/usr/bin/env bash
    set -euo pipefail
    # Vite's own --open races the server on a cold start and lands on an error
    # page, so wait for the port to answer before opening the browser.
    ( until curl -sf -o /dev/null http://127.0.0.1:7400/; do sleep 0.3; done
      case "$(uname -s)" in
        Darwin) open http://127.0.0.1:7400/ ;;
        *) xdg-open http://127.0.0.1:7400/ >/dev/null 2>&1 || true ;;
      esac ) &
    cd frontend-demo && npm run dev -- --port 7400 --strictPort --host 0.0.0.0

# The backend behind the demo's "why" button, on :7401, next to `just demo`
# (Vite proxies /api/why to it). Runs the Claude Code CLI with your own login, or
# with CLAUDE_CODE_OAUTH_TOKEN / ANTHROPIC_API_KEY when set. Unlock the button in
# the demo menu with the password below.
[doc("Start the demo's 'why' backend (needs the Claude Code CLI)")]
demo-why password="lokaal-demo-wachtwoord":
    DEMO_WHY_PASSWORD="${DEMO_WHY_PASSWORD:-{{password}}}" node frontend-demo/server/why.mjs

# Run the demo frontend locally without opening a browser (same server as `demo`)
dev-demo: wasm-build
    cd frontend-demo && npm run dev -- --port 7400 --strictPort --host 0.0.0.0

# The recorded walkthrough of the demo, one entry point:
#
#   just walkthrough record          the demo with the recorder panel (Chrome)
#   just walkthrough prepare [take]  clean the voice, transcribe, draft cuts
#   just walkthrough check [take]    what a take sounds like, in numbers
#   just walkthrough script [take]   a draft script per slide, for the cloned voice
#   just walkthrough build           walkthrough.yaml -> what the demo plays
#   just walkthrough publish <tag>   the media into a GitHub release (asks first)
#   just walkthrough status          which takes there are, and how far along
#   just walkthrough test            the pipeline's own tests
#
# Takes land in .walkthrough/takes/. The route from recording to release is in
# docs/src/content/docs/components/demo.md#from-recording-to-release. The
# pipeline needs ffmpeg and downloads its models on first use.
[doc("The recorded walkthrough: record, prepare, check, script, build, publish, status, test")]
walkthrough *args:
    #!/usr/bin/env bash
    set -euo pipefail
    set -- {{args}}
    case "${1:-}" in
      record)
        just wasm-build
        # Open Chrome once the server answers; Vite's own --open races a cold
        # start. The endpoint that writes takes exists only with this variable.
        ( until curl -sf -o /dev/null http://127.0.0.1:7400/; do sleep 0.3; done
          case "$(uname -s)" in
            Darwin) open -a "Google Chrome" "http://127.0.0.1:7400/presentatie?record" ;;
            *) xdg-open "http://127.0.0.1:7400/presentatie?record" >/dev/null 2>&1 || true ;;
          esac ) &
        cd frontend-demo && WALKTHROUGH_RECORD=1 npm run dev -- --port 7400 --strictPort --host 127.0.0.1
        ;;
      test)
        uv run --quiet --project script/walkthrough pytest -q script/walkthrough/tests
        ;;
      verify)
        # The same check CI runs (job "Rondleiding speelt terug"): every
        # chapter replayed headless, every action that finds nothing named.
        # Starts its own dev server unless --url points at one; needs the
        # WASM engine (just wasm-build).
        shift
        node frontend-demo/scripts/verify-walkthrough.mjs "$@"
        ;;
      *)
        uv run --quiet --project script/walkthrough walkthrough "$@"
        ;;
    esac

# Everything the demo consists of, in the order a failure is cheapest to read:
# the laws themselves, then what they compute, then the app around them.
#
# The demo is a corpus plus a frontend plus the engine compiled to WASM, and
# each of the three used to be checked by a different command. Running them
# separately meant a law could be schema-valid, its scenarios green, and the
# app still broken on a stale WASM build. This is the one command to run before
# pushing anything demo-related; CI runs the same four steps in its own jobs.
[doc("Check the whole demo: laws, scenarios, frontend tests, WASM and build")]
demo-check: validate-demo awb-parity-test service-map-check bdd-demo
    cd frontend-demo && npx vitest run
    just wasm-build
    cd frontend-demo && npm run build

# Regenerate all BDD step bindings from bdd/grammar.yaml
bdd-codegen:
    node bdd/codegen/gen-js.mjs
    @echo "Rust bindings regenerate automatically via build.rs on next cargo build"

# Run BDD tests with execution trace output (writes box-drawing traces to trace_output/)
bdd-trace:
    rm -rf trace_output
    cd packages/engine && {{ci_flags}} TRACE=1 cargo test --test bdd -- --nocapture

# Run regelrecht-github crate tests (shared GitHub REST client)
github-test:
    cd packages && {{ci_flags}} cargo test -p regelrecht-github

# Run harvester tests
harvester-test:
    cd packages/harvester && {{ci_flags}} cargo test

# Run pipeline unit tests. The suites in tests/ run in
# `pipeline-integration-test`; `--lib` does not reach them, and most of them
# need Docker. The CLI test suites run the built binaries against temp
# directories and need no Docker, so they belong in this recipe as well.
[doc("Run pipeline unit tests (no Docker; the container-backed suites are in pipeline-integration-test)")]
pipeline-test:
    cd packages/pipeline && {{ci_flags}} cargo test --lib
    cd packages/pipeline && {{ci_flags}} cargo test --test law_source_cli --test law_check_cli --test enrich_once_cli --test law_migrate_cli

# Run pipeline integration tests (requires Docker for testcontainers)
pipeline-integration-test:
    cd packages/pipeline && {{ci_flags}} cargo test --test '*'

# Run the frontend Playwright e2e specs (mocked backend, no Postgres/token needed).
# Depends on wasm-build: the scenario-execution specs run the real WASM engine
# in-browser, so the compiled artifact under frontend/public/wasm/pkg must exist.
# cargo caches, so the build is a near-no-op once warm.
[doc("Run the frontend Playwright e2e specs against a mocked backend")]
test-e2e: wasm-build
    npm run test:e2e -w frontend


# Optionele variabelen-overrides (moeten VOOR de recipe-naam staan in just):
#   just PR=886 smoke-preview
#   just URL=https://editor.regelrecht.rijks.app smoke-preview
PR := ""
URL := ""

# Token-vrije browser-smoketest tegen een DEPLOYED editor-preview (of productie):
# echte Keycloak-login + editor-api + DB + WASM. Draai met een PR-nummer of een
# volledige URL. De test-user-creds komen uit /workspace/.cred (override met
# SMOKE_CRED_FILE) - nooit in de repo.
#
# Drie manieren om het doel op te geven (positioneel is het handigst):
#   just smoke-preview 886                                    # PR-preview #886
#   just smoke-preview https://editor.regelrecht.rijks.app    # exacte URL
#   just PR=886 smoke-preview                                 # via variabele
[doc("Browser smoketest against a deployed editor preview or production")]
smoke-preview TARGET="":
    #!/usr/bin/env bash
    set -euo pipefail
    target="{{TARGET}}"
    if [ -n "{{URL}}" ]; then
      base="{{URL}}"
    elif [ -n "{{PR}}" ]; then
      base="https://editor-pr{{PR}}-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl"
    elif [ -n "$target" ]; then
      case "$target" in
        http://*|https://*) base="$target" ;;
        *) base="https://editor-pr${target}-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl" ;;
      esac
    else
      echo "Geef een doel mee, bijv. 'just smoke-preview 886' of 'just URL=<url> smoke-preview'." >&2
      exit 2
    fi
    cred="${SMOKE_CRED_FILE:-/workspace/.cred}"
    if [ ! -f "$cred" ]; then
      echo "Cred-bestand niet gevonden: $cred (verwacht regels 'Username:'/'Password:')." >&2
      exit 2
    fi
    export SMOKE_BASE_URL="$base"
    export SMOKE_USER="$(sed -n 's/^Username:[[:space:]]*//p' "$cred" | head -n1)"
    export SMOKE_PASS="$(sed -n 's/^Password:[[:space:]]*//p' "$cred" | head -n1)"
    if [ -z "$SMOKE_USER" ] || [ -z "$SMOKE_PASS" ]; then
      echo "Kon Username/Password niet uit $cred lezen." >&2
      exit 2
    fi
    echo "Smoke-test tegen $SMOKE_BASE_URL"
    npx playwright test -c frontend/playwright.smoke.config.js

# --- Mutation testing ---

# Run mutation testing on engine (in-place because tests use relative paths to corpus/)
mutants *ARGS:
    cd packages/engine && cargo mutants --in-place --timeout-multiplier 3 {{ARGS}}

# Mutation testing on your own changes only — the local tegenhanger of the
# mutation-diff CI-poort. De volledige set kost uren, dit een paar minuten.
#
# Het pad-detail dat dit recept wegneemt: cargo-mutants noemt bestanden
# relatief aan de workspace-root (packages/), dus de diff moet `b/engine/src/…`
# bevatten en niet `b/packages/engine/src/…`. Voer je een diff met de verkeerde
# prefix in, dan vindt hij nul mutanten en lijkt alles in orde.
#
# Driepunts (`BASE...HEAD`), niet tweepunts: tweepunts vergelijkt twee bomen,
# dus alles wat main na jouw aftakking veranderde komt in de diff terecht als
# jouw wijziging. Op een branch die achterloopt muteer je dan andermans regels.
#
# Pakketten, werkmap en timeout volgen `.github/workflows/mutation-diff.yml`.
# Dit recept keek eerst alleen naar engine, terwijl de poort ook pipeline
# muteert; een pipeline-PR was hier dan groen zonder één mutant te zien en
# viel pas in CI om (#1549, 14 overlevers). Verander je het één, verander dan
# het ander mee.
[doc("Mutation testing on your own changed lines only")]
mutants-diff BASE="origin/main":
    #!/usr/bin/env bash
    set -euo pipefail
    diff_file="$(mktemp -t mutants-diff-XXXXXX.diff)"
    git -C packages diff --relative "{{BASE}}...HEAD" -- engine pipeline > "$diff_file"
    if [ ! -s "$diff_file" ]; then
        echo "Geen gewijzigde regels in packages/engine of packages/pipeline ten opzichte van {{BASE}}."
        exit 0
    fi
    cd packages
    cargo mutants --in-place --timeout 120 --in-diff "$diff_file"

# --- Benchmarks ---

_bench_flags := "--bench uri_parsing --bench variable_resolution --bench operations --bench article_evaluation --bench law_loading --bench priority --bench service_e2e"

# Run criterion benchmarks (skips unit test harness, runs only criterion benches)
bench *ARGS:
    cd packages/engine && cargo bench {{_bench_flags}} {{ARGS}}

# Run benchmarks and save baseline
bench-save NAME:
    cd packages/engine && cargo bench {{_bench_flags}} -- --save-baseline {{NAME}}

# Compare against saved baseline (run `just bench-save <name>` first to create one)
bench-compare BASE:
    cd packages/engine && cargo bench {{_bench_flags}} -- --baseline {{BASE}}

# --- Security ---

# De deterministische helft van de security-audit, en de enige die een pull
# request blokkeert. Bans, licenses en sources hangen alleen aan wat er in de
# lockfiles staat: dezelfde commit geeft vandaag en over een maand dezelfde
# uitslag. Kwetsbaarheden horen daar niet bij, want die komen van buiten en
# maken een PR rood zonder dat er iets aan die PR veranderd is; die staan in
# `just audit-advisories`.
audit:
    script/cargo-deny.sh bans licenses sources
    # license-checker draait alleen over de workspace in de root; die drops
    # --production omdat de root geen eigen productie-deps heeft, en de
    # hele-boom-scan is strikt ruimer.
    npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-1.0;AGPL-3.0;SSPL-1.0;BUSL-1.1"

# De tijdsafhankelijke helft: advisories uit RustSec en npm. Draait periodiek
# in .github/workflows/security-advisories.yml en meldt zich daar als issue.
audit-advisories:
    script/audit-advisories.sh

# De meldlogica van de advisory-controle
advisories-report-test:
    script/report-advisories.test.sh

# De preflight van `just dev`: mold alleen eisen waar cargo ermee linkt
[doc("Check that the dev preflight only requires mold on x86_64 Linux")]
dev-preflight-test:
    script/dev-lib.test.sh

# --- Admin ---

# Run admin API locally (requires DATABASE_SERVER_FULL env var)
admin:
    cd packages && cargo run --package regelrecht-admin

# Check admin Rust code
admin-check:
    cd packages && {{ci_flags}} cargo check --package regelrecht-admin

# Lint admin Rust code
admin-lint:
    cd packages && {{ci_flags}} cargo clippy --package regelrecht-admin

# Format check admin Rust code
admin-fmt:
    cd packages && cargo fmt --check --package regelrecht-admin

# Run admin tests
admin-test:
    cd packages && {{ci_flags}} cargo test --package regelrecht-admin

# --- Editor API ---

# Run editor API locally
editor-api:
    cd packages && cargo run --package regelrecht-editor-api

# Run editor API with real SSO Rijk login (loads .env.sso-local; see auth-and-roles.md)
editor-sso:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f .env.sso-local ]; then
        echo "Missing .env.sso-local — copy .env.sso-local.example and fill in the Keycloak values." >&2
        exit 1
    fi
    set -a; . ./.env.sso-local; set +a
    cd packages && cargo run --package regelrecht-editor-api

# Check editor API Rust code
editor-api-check:
    cd packages && {{ci_flags}} cargo check --package regelrecht-editor-api

# Lint editor API Rust code
editor-api-lint:
    cd packages && {{ci_flags}} cargo clippy --package regelrecht-editor-api

# Format check editor API Rust code
editor-api-fmt:
    cd packages && cargo fmt --check --package regelrecht-editor-api

# Run ALL editor API tests: src unit tests + tests/*.rs integration tests
# (the latter require Docker for testcontainers). A shortcut for running this
# crate alone; `just check` covers it through the workspace-wide `test`.
[doc("Run ALL editor API tests, unit and integration (needs Docker)")]
editor-api-test:
    cd packages && {{ci_flags}} cargo test --package regelrecht-editor-api

# Run editor API integration tests only (requires Docker for testcontainers).
editor-api-integration-test:
    cd packages && {{ci_flags}} cargo test --package regelrecht-editor-api --test '*'

# --- Development (native with hot reload) ---

compose := "docker compose -f docker-compose.dev.yml"
compose-local := compose + " -f dev/compose.local.yaml"
compose-native := compose + " -f dev/compose.native.yaml"
pidfile := ".dev-pids"

# Geef deze worktree een eigen cargo-target-dir. Kost een koude build (~40 s voor
# `just validate`, ~170 s voor `just build-check`) en levert op dat een lange build
# hier geen andere worktree meer laat wachten op de build-lock. De meting waarop
# die afweging rust staat bovenin script/target-dir.sh.
[doc("Geef deze worktree een eigen cargo-target-dir (geen gedeelde build-lock)")]
target-isolated:
    script/target-dir.sh isolated

# Terug naar de gedeelde target-dir van de hoofdcheckout.
[doc("Zet deze worktree terug op de gedeelde cargo-target-dir")]
target-shared:
    script/target-dir.sh shared

# One-time build-speed setup: install mold (x86_64 Linux) + sccache, share one target dir across worktrees
dev-setup:
    #!/usr/bin/env bash
    set -euo pipefail
    bold="\033[1m"  dim="\033[2m"  reset="\033[0m"
    green="\033[32m"  yellow="\033[33m"  red="\033[31m"

    sudo_if_needed() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

    install_one() {
        local bin="$1"; shift
        command -v "$bin" >/dev/null 2>&1 && { printf "${green}%s already installed${reset}\n" "$bin"; return 0; }
        printf "${bold}=> Installing %s…${reset} " "$bin"
        if "$@" >/dev/null 2>&1; then printf "${green}done${reset}\n"; return 0; else printf "${red}failed${reset} (install %s manually)\n" "$bin"; return 1; fi
    }

    # mold is only the linker on x86_64 Linux (dev_needs_mold, shared with the
    # dev recipes' preflight). Elsewhere it is neither installed nor asked for.
    source script/dev-lib.sh
    needs_mold=false
    if dev_needs_mold; then needs_mold=true; fi
    # install_mold <installer…>: install mold where cargo links with it, no-op elsewhere.
    install_mold() { [ "$needs_mold" = true ] || return 0; install_one mold "$@" || true; }

    if command -v apt-get >/dev/null 2>&1; then
        sudo_if_needed apt-get update -qq || true
        install_mold sudo_if_needed apt-get install -y mold
        install_one sccache sudo_if_needed apt-get install -y sccache || true
    elif command -v dnf >/dev/null 2>&1; then
        install_mold sudo_if_needed dnf install -y mold
        install_one sccache sudo_if_needed dnf install -y sccache || true
    elif command -v brew >/dev/null 2>&1; then
        install_mold brew install mold
        install_one sccache brew install sccache || true
    else
        printf "${yellow}No supported package manager found.${reset}\n"
        if [ "$needs_mold" = true ]; then printf "  Install mold:    https://github.com/rui314/mold\n"; fi
        printf "  Install sccache: cargo install sccache --locked\n"
    fi
    # Decided by what is on PATH afterwards, so a pre-existing mold counts too.
    mold_ok=true
    if [ "$needs_mold" = true ] && ! command -v mold >/dev/null 2>&1; then mold_ok=false; fi
    # sccache may not be packaged everywhere — fall back to cargo install.
    command -v sccache >/dev/null 2>&1 || install_one sccache cargo install sccache --locked

    # Shared target dir across all worktrees. .worktrees/ lives inside the main
    # checkout, so cargo's upward config search finds this root .cargo/config.toml
    # from every worktree. It is gitignored (machine-specific absolute path), so
    # CI keeps its own packages/target.
    #
    # The sharing is worth it — a first `just build-check` in a fresh worktree
    # takes 1 second instead of 170 — but cargo locks a target dir exclusively
    # for the length of a build, so concurrent worktrees serialize: a `just
    # validate` measured 2 s alone and 38 s next to a 45 s `just lint`. A worktree
    # that runs long builds can opt out with `just target-isolated`; the numbers
    # behind the trade-off are in script/target-dir.sh.
    root="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"

    # A Rust build writes tens of thousands of small files. On a slow/remote
    # mount (9p in WSL2/Docker-Desktop dev containers, NFS, SMB, …) that I/O
    # dominates build time — far more than the linker does. Detect that and put
    # the shared target on fast local storage instead of inside the repo.
    fstype="$(stat -f -c %T "$root" 2>/dev/null || echo unknown)"
    case "$fstype" in
        9p|v9fs|nfs|nfs4|cifs|smb*|smb2|fuseblk|fuse.*|vboxsf|virtiofs|prl_fs)
            cache="${XDG_CACHE_HOME:-$HOME/.cache}/regelrecht"
            shared="$cache/target-$(printf '%s' "$root" | cksum | cut -d' ' -f1)"
            printf "${yellow}=> Repo is on a slow filesystem (%s); using fast local target dir.${reset}\n" "$fstype"
            ;;
        *)
            shared="$root/.cargo/target"
            ;;
    esac
    mkdir -p "$root/.cargo" "$shared"
    {
        echo "# Generated by 'just dev-setup'. Gitignored — machine-local, do not commit."
        echo "[build]"
        echo "target-dir = \"$shared\""
    } > "$root/.cargo/config.toml"
    printf "${green}=> Shared target dir:${reset} %s\n" "$shared"

    printf "\n"
    if [ "$needs_mold" = false ]; then
        printf "${bold}${green}Done.${reset} Shared target is active for all worktrees (mold only links on x86_64 Linux, not needed here).\n"
    elif [ "$mold_ok" = true ]; then
        printf "${bold}${green}Done.${reset} Mold linking + shared target are active for all worktrees.\n"
    else
        printf "${bold}${yellow}Partly done.${reset} Shared target is set up, but ${red}mold is missing${reset} — dev builds will fail to link.\n"
        printf "${dim}Install mold manually (https://github.com/rui314/mold), then re-run 'just dev-setup'.${reset}\n"
    fi
    printf "${dim}Old per-worktree packages/target dirs can now be removed to reclaim disk.${reset}\n"
    printf "${dim}Optional: enable sccache locally (disables incremental, best for cold/flag-varying builds):${reset}\n"
    printf "  export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0\n"

# Vite ports default to 7300/7400/7500 (the redirect URIs already registered on
# the regelrecht-local Keycloak client); override via EDITOR_PORT /
# LAWMAKING_PORT. No grafana / prometheus / workers are started.
#
# Start the dev stack: only what the chosen app needs (backend, DB, WASM, vite). No arg = all apps; APP = editor | admin | lawmaking | all
dev APP="all":
    #!/usr/bin/env bash
    set -euo pipefail
    export COMPOSE="{{ compose-native }}"  PIDFILE="{{ pidfile }}"
    source script/dev-lib.sh

    app="{{ APP }}"
    case "$app" in
        editor|admin|lawmaking|all) ;;
        *) printf "${red}Unknown app '%s'${reset} — use: editor | admin | lawmaking | all\n" "$app"; exit 1 ;;
    esac

    run_editor=false; run_admin=false; run_lawmaking=false
    case "$app" in
        editor)    run_editor=true ;;
        admin)     run_admin=true ;;
        lawmaking) run_lawmaking=true ;;
        all)       run_editor=true; run_admin=true; run_lawmaking=true ;;
    esac

    # Vite (browser-facing) ports — overridable. Editor stays 7300 unless you
    # also update BASE_URL in .env.sso-local and the Keycloak redirect URI.
    editor_port="${EDITOR_PORT:-7300}"
    lawmaking_port="${LAWMAKING_PORT:-7500}"
    # In 'all', editor-api keeps :8000 and admin moves to :8001 to avoid the clash.
    admin_api_port=8000
    if [ "$app" = all ]; then admin_api_port=8001; fi
    # When both editor and admin run (i.e. 'all'), point editor-api's
    # harvester-admin proxy at the local admin API so the editor's Corpusinwinning
    # section reaches it. In editor-only mode the proxy is unset and the Corpusinwinning
    # screens 503 (run `just dev all` for the full harvester flow).
    harvest_admin_env=""
    if [ "$run_admin" = true ]; then
        harvest_admin_env="HARVEST_ADMIN_URL=http://localhost:${admin_api_port} "
    fi

    if [ "$run_editor" = true ] || [ "$run_admin" = true ]; then
        dev_preflight --rust --node
    else
        dev_preflight --node
    fi

    # editor / all run editor-api with OIDC against the central Keycloak.
    if [ "$run_editor" = true ] && [ ! -f .env.sso-local ]; then
        printf "${red}Missing .env.sso-local${reset} — copy .env.sso-local.example and fill in the Keycloak values (see auth-and-roles.md).\n"
        exit 1
    fi

    # DB is needed by editor (session store) and admin; lawmaking has no backend.
    if [ "$run_editor" = true ] || [ "$run_admin" = true ]; then
        dev_compose_up postgres
        dev_wait_postgres
    fi

    rm -f "$PIDFILE"

    if [ "$run_editor" = true ]; then
        dev_ensure_wasm
        if dev_ensure_deps frontend "editor frontend"; then
            # Plain `cargo run` (not cargo-watch): a frontend-dev loop doesn't
            # rebuild the backend, and cargo-watch's recursive watch hangs on a
            # 9p-mounted worktree. Matches the `editor-sso` recipe. Source
            # .env.sso-local inside the backgrounded shell so the OIDC /
            # DATABASE_URL / BASE_URL it sets scope to editor-api only.
            dev_start "editor-api (:8000, OIDC)" .dev-editor-api.log \
              "set -a; . ./.env.sso-local; set +a; cd packages && ${harvest_admin_env}cargo run --package regelrecht-editor-api"
            dev_start "editor vite (:${editor_port})" .dev-editor.log \
              "cd frontend && API_PORT=8000 npx vite --port ${editor_port} --strictPort --host 0.0.0.0"
        fi
    fi

    # Admin is now an API-only service (its dashboard UI moved into the editor's
    # Corpusinwinning section). In 'all' mode the editor-api proxies to it via
    # HARVEST_ADMIN_URL (set on the editor-api above), so the harvester screens
    # work end-to-end through the editor at :${editor_port}.
    if [ "$run_admin" = true ]; then
        db_url="postgres://regelrecht:regelrecht_dev@${DB_HOST:-localhost}:${POSTGRES_PORT:-5433}/regelrecht_pipeline"
        dev_start "admin API (:${admin_api_port})" .dev-admin.log \
          "cd packages && DATABASE_URL='$db_url' RUST_LOG='${RUST_LOG:-info}' ADMIN_PORT=${admin_api_port} cargo run --package regelrecht-admin"
    fi

    if [ "$run_lawmaking" = true ]; then
        if dev_ensure_deps frontend-lawmaking "lawmaking frontend"; then
            dev_start "lawmaking vite (:${lawmaking_port})" .dev-lawmaking.log \
              "cd frontend-lawmaking && npx vite --port ${lawmaking_port} --strictPort --host 0.0.0.0"
        fi
    fi

    printf "${bold}=> Waiting for services…${reset} "
    sleep 4
    printf "${green}done${reset}\n\n"

    printf "${bold}${green}  Dev stack (%s) is running (vite HMR; backends run-once)${reset}\n\n" "$app"
    if [ "$run_editor" = true ]; then
        echo "  Editor:     http://localhost:${editor_port}     (vite HMR → editor-api :8000, SSO)"
    fi
    if [ "$run_admin" = true ]; then
        echo "  Admin API:  http://localhost:${admin_api_port}     (harvester API; UI is the editor's Corpusinwinning section)"
    fi
    if [ "$run_lawmaking" = true ]; then
        echo "  Lawmaking:  http://localhost:${lawmaking_port}     (vite HMR, no backend)"
    fi
    if [ "$run_editor" = true ] || [ "$run_admin" = true ]; then
        echo "  PostgreSQL: localhost:${POSTGRES_PORT:-5433}"
    fi
    printf "\n"
    printf "  ${dim}Logs:${reset}            tail -f .dev-*.log\n"
    printf "  ${dim}Stop everything:${reset} just dev-down\n"

# The recipe was called dev-frontend until the backend-only `dev` was dropped.
alias dev-frontend := dev

# Stop dev: kill native processes and stop infra
dev-down:
    #!/usr/bin/env bash
    set -euo pipefail
    export COMPOSE="{{ compose-native }}"  PIDFILE="{{ pidfile }}"
    source script/dev-lib.sh
    dev_stop

# Follow infra logs in dev mode
dev-logs *ARGS:
    {{ compose-native }} logs -f {{ARGS}}

# Connect to the dev database via psql
dev-psql:
    {{ compose-native }} exec postgres psql -U regelrecht regelrecht_pipeline

# --- Full local stack (everything in Docker) ---

# Run the complete stack in Docker (no hot reload)
local:
    #!/usr/bin/env bash
    set -euo pipefail
    logfile=$(mktemp)
    printf "\033[1m=> Building and starting full stack…\033[0m "
    if {{ compose-local }} up --build -d > "$logfile" 2>&1; then
        printf "\033[32mdone\033[0m\n\n"
        echo "  Editor:     http://localhost:${FRONTEND_PORT:-3000}"
        echo "  Admin:      http://localhost:${ADMIN_PORT:-8000}"
        echo "  Grafana:    http://localhost:${GRAFANA_PORT:-3001}"
        echo "  Prometheus: http://localhost:${PROMETHEUS_PORT:-9090}"
        echo "  PostgreSQL: internal (use 'just local-psql' to connect)"
        echo ""
        printf "  \033[2mLogs:\033[0m just local-logs\n"
        printf "  \033[2mStop:\033[0m just local-down\n"
    else
        printf "\033[31mfailed\033[0m\n\n"
        cat "$logfile"
        rm -f "$logfile"
        exit 1
    fi
    rm -f "$logfile"

# Stop the full local stack
local-down:
    {{ compose-local }} down

# Follow logs from local services (optionally filter: just local-logs admin)
local-logs *ARGS:
    {{ compose-local }} logs -f {{ARGS}}

# Rebuild and restart a specific local service (e.g., just local-restart admin)
local-restart SERVICE:
    {{ compose-local }} up --build -d {{SERVICE}}

# Show status of local services
local-ps:
    {{ compose-local }} ps

# Connect to the local database via psql
local-psql:
    {{ compose-local }} exec postgres psql -U regelrecht regelrecht_pipeline

# Remove all local data (volumes)
local-clean:
    {{ compose-local }} down -v

# --- Documentation ---

# Install docs dependencies
docs-install:
    cd docs && npm ci

# Start docs dev server (Astro, with HMR) at http://localhost:4321
docs:
    cd docs && npm run dev

# Build docs for production
docs-build:
    cd docs && npm run build

# Preview production docs build (Pagefind search works here, not in dev)
docs-preview:
    cd docs && npm run preview

# Run the accessibility gate (build + mermaid-alt + heading-order + pa11y-ci htmlcs+axe, WCAG 2.1 AA)
docs-a11y:
    cd docs && npm run a11y

# --- PoC-portaal ---

# Bouw de assets van het portaal (het ontwerpsysteem voor zijn eigen twee pagina's)
poc-assets:
    npm run build -w poc-portal-assets

# Bouw elke statische poc met zijn eigen basis, en zet alles klaar in .poc-static/
#
# De WASM-engine komt uit `just wasm-build`, chronolexografie uit `just
# poc-chronolexografie-wasm`; copy-assets.js van elke poc stopt met een
# duidelijke melding als die er niet is.
poc-build: wasm-build poc-chronolexografie-wasm poc-assets
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf .poc-static
    mkdir -p .poc-static/_assets
    cp -R frontend-poc-portal/dist/. .poc-static/_assets/
    POC_BASE=/terugbetaalregimes/ npm run build -w poc-terugbetaalregimes
    mkdir -p .poc-static/terugbetaalregimes
    cp -R frontend-poc-terugbetaalregimes/dist/. .poc-static/terugbetaalregimes/
    POC_BASE=/nieuwkomersbekostiging/ npm run build -w poc-nieuwkomersbekostiging
    mkdir -p .poc-static/nieuwkomersbekostiging
    cp -R frontend-poc-nieuwkomersbekostiging/dist/. .poc-static/nieuwkomersbekostiging/
    POC_BASE=/chronolexografie/ npm run build -w poc-chronolexografie
    mkdir -p .poc-static/chronolexografie
    cp -R packages/poc-chronolexografie/frontend/dist/. .poc-static/chronolexografie/

# Start het poc-portaal op http://localhost:8611
#
# De wachtwoorden zijn hier bewust hardcoded en flauw: dit recept draait alleen
# lokaal, en een ontwikkelaar die ze moet opzoeken gebruikt het niet. In ZAD
# komen ze uit `zad env`; het portaal weigert te starten als er één ontbreekt.
#
# De statische pocs worden verwacht in .poc-static/<slug>/. Zolang die er niet
# zijn toont het overzicht ze wel en geeft de poc zelf een 404 achter de poort —
# de poort werkt dus los van de vraag of er al een poc gebouwd is.
poc: poc-build
    #!/usr/bin/env bash
    set -euo pipefail
    echo "poc-portaal → http://localhost:8611"
    echo "wachtwoorden: terugbetaalregimes/nieuwkomersbekostiging/napp/chronolexografie/wetsgeschiedenis = 'demo'"
    POC_COOKIE_SECRET=lokale-ontwikkelsleutel-niet-geheim-0123 \
    POC_PW_TERUGBETAALREGIMES=demo \
    POC_PW_NIEUWKOMERSBEKOSTIGING=demo \
    POC_PW_NAPP=demo \
    POC_PW_CHRONOLEXOGRAFIE=demo \
    POC_PW_WETSGESCHIEDENIS=demo \
    POC_STATIC_DIR="$(pwd)/.poc-static" \
    POC_PORT=8611 \
    cargo run --manifest-path packages/Cargo.toml --package regelrecht-poc-portal

# Start de wetsgeschiedenis los, op http://localhost:8420/wetsgeschiedenis/
#
# Bouwt de frontend met hetzelfde voorvoegsel als achter het portaal, zodat
# `POC_UPSTREAM_WETSGESCHIEDENIS=http://127.0.0.1:8420 just poc` haar ook via de
# poort laat zien. Wat je ophaalt landt in packages/poc-wetsgeschiedenis/data/.
# Nodig: python3 met pyyaml, en curl.
poc-wetsgeschiedenis port="8420":
    #!/usr/bin/env bash
    set -euo pipefail
    POC_BASE=/wetsgeschiedenis/ npm run build -w poc-wetsgeschiedenis
    echo "wetsgeschiedenis → http://localhost:{{port}}/wetsgeschiedenis/"
    WG_PORT={{port}} WG_BASE_PATH=/wetsgeschiedenis/ \
    cargo run --manifest-path packages/Cargo.toml -p regelrecht-poc-wetsgeschiedenis

# Bouw de chronolexografie-poc als WASM, naar packages/poc-chronolexografie/frontend/public/wasm/pkg
#
# De cel, de engine en het spel draaien in de browser van de bezoeker
# (packages/poc-chronolexografie, feature `wasm`). copy-assets.js van de app
# stopt met een duidelijke melding als dit er niet is.
poc-chronolexografie-wasm:
    #!/usr/bin/env bash
    set -euo pipefail
    bindgen=$({{just_executable()}} wasm-bindgen-cli)
    # Zelfde vaste target-map als wasm-build, om dezelfde reden.
    cargo build --manifest-path packages/Cargo.toml --package regelrecht-poc-chronolexografie --features wasm --target wasm32-unknown-unknown --release --target-dir packages/target
    "$bindgen" --target web --out-dir packages/poc-chronolexografie/frontend/public/wasm/pkg packages/target/wasm32-unknown-unknown/release/regelrecht_poc_chronolexografie.wasm

# Start de chronolexografie-poc los, op de vite-dev-server (http://localhost:5410)
#
# Alles draait in de browser: elk tabblad speelt zijn eigen casus. Achter het
# portaal komt hij uit `just poc`. De NAPP-casus komt uit een bron; lokaal
# zonder token met POC_BRON_NAPP=<map met casussen/, regulation/ en cells/>.
poc-chronolexografie: poc-chronolexografie-wasm
    npm run dev -w poc-chronolexografie

# Start de beleidsassistent van één casus, naast `just poc` in een tweede terminal
#
# In het image doet start.sh dit met een poort per casus; lokaal is één casus
# tegelijk genoeg. Het portaal proxyt /<casus>/api hiernaartoe zodra
# POC_ASSISTENT_<CASUS> gezet is, dus `just poc` moet die variabele kennen:
#
#     POC_ASSISTENT_TERUGBETAALREGIMES=http://127.0.0.1:3600 just poc
#     POC_ASSISTENT_NIEUWKOMERSBEKOSTIGING=http://127.0.0.1:3700 just poc
#
# Wie de vite-dev-server gebruikt (`npm run dev -w poc-<casus>`) heeft dat niet
# nodig: die proxyt /api zelf naar dezelfde poort.
#
# Vereist een ingelogde Claude CLI (`claude setup-token`) of ANTHROPIC_API_KEY;
# zonder allebei weigert de assistent te starten.

# Start de beleidsassistent van één casus (naast `just poc`)
#
# De poort leidt standaard uit de casus, want elke app proxyt /api naar een
# eigen poort (vite.config.js): terugbetaalregimes naar 3600,
# nieuwkomersbekostiging naar 3700. Een vaste standaard van 3600 startte de
# assistent van nieuwkomers op een poort waar zijn app niet keek, en dat
# leest als "de backend draait niet" zonder dat er iets faalt.
poc-assistent casus="terugbetaalregimes" poort="":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${CLAUDE_CODE_OAUTH_TOKEN:-}${ANTHROPIC_API_KEY:-}" ]; then
      echo "geen CLAUDE_CODE_OAUTH_TOKEN of ANTHROPIC_API_KEY; draai eerst \`claude setup-token\`" >&2
      exit 1
    fi
    poort="{{poort}}"
    if [ -z "$poort" ]; then
      case "{{casus}}" in
        terugbetaalregimes) poort=3600 ;;
        nieuwkomersbekostiging) poort=3700 ;;
        *) echo "onbekende casus {{casus}}: geef de poort mee" >&2; exit 1 ;;
      esac
    fi
    echo "beleidsassistent {{casus}} → http://127.0.0.1:$poort"
    POC_CASUS={{casus}} \
    POC_CASUS_DIR="$(pwd)/corpus-poc/{{casus}}" \
    POC_WASM_DIR="$(pwd)/.poc-static/{{casus}}/wasm/pkg" \
    POC_VARIANT_OPSLAG=0 \
    PORT="$poort" \
    node packages/poc-assistent/index.js

# --- Architecture model ---

# Generate the code-derived architecture model
# (packages/arch-extract → docs/src/content/architecture/model.json, a gitignored
# path). The model is generated on-demand, never committed: the local architecture
# explorer regenerates it from the working tree. Run this to inspect the model
# without starting the explorer.
[doc("Generate the code-derived architecture model")]
arch-generate:
    cd packages && {{ci_flags}} cargo run --quiet -p regelrecht-arch-extract -- generate

# Start the local architecture explorer: build the Vue Flow UI, then run the
# arch-extract server, which regenerates the model on-demand from the working
# tree (GET /api/model, cached on source mtime) and serves the UI on
# 0.0.0.0:7180. Open http://localhost:7180. Override the port with
# ARCH_EXPLORE_PORT (stay in 7100–7300 for the dev container). See
# packages/arch-extract/README.md.
[doc("Start the local architecture explorer on http://localhost:7180")]
arch-explore:
    cd packages/arch-extract/ui && npm install && npm run build
    cd packages && cargo run --release --quiet -p regelrecht-arch-extract -- serve

# Run the arch-extract tests: the Rust side (schema validation of the generated
# model + the prose-sidecar drift logic) and the explorer UI's vitest suite
# (edge-lifting / aggregation logic). Part of `just check`.
[doc("Run the arch-extract tests, Rust and explorer UI")]
arch-test:
    cd packages && {{ci_flags}} cargo test -p regelrecht-arch-extract
    npm --prefix packages/arch-extract/ui install
    npm --prefix packages/arch-extract/ui test

# --- Architecture prose sidecar ---
# Per-node "wat/waarom" narrative lives beside the model in
# packages/arch-extract/prose/, keyed by node id, at container + component level.

# Report drift between the on-demand model and the prose sidecar.
arch-prose-status:
    cd packages && cargo run --quiet -p regelrecht-arch-extract -- prose status

# Like arch-prose-status, but exit non-zero on any drift (for the scheduled flow).
arch-prose-check:
    cd packages && cargo run --quiet -p regelrecht-arch-extract -- prose check

# Scaffold empty prose stubs for undocumented in-scope nodes (seeded with each
# node's doc-comment as a starting point).
[doc("Scaffold empty prose stubs for undocumented in-scope nodes")]
arch-prose-sync:
    cd packages && cargo run --quiet -p regelrecht-arch-extract -- prose sync

# Refresh the fingerprint of prose entries after rewriting their text (clears a
# stale flag). Pass one or more node ids, or --all.
[doc("Refresh the fingerprint of prose entries after rewriting them")]
arch-prose-bless *ARGS:
    cd packages && cargo run --quiet -p regelrecht-arch-extract -- prose bless {{ARGS}}

# Run the scheduled prose-drift flow: diff model vs. sidecar and, on drift, open
# a draft PR with proposals. Meant to run on a schedule; DRY_RUN=true to preview.
[doc("Run the scheduled prose-drift flow and open a draft PR on drift")]
arch-prose-drift-pr:
    packages/arch-extract/scripts/prose-drift-pr.sh
