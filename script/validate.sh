#!/usr/bin/env bash
# Validate regulation YAML files against the law schema.
#
# Usage:
#   script/validate.sh corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml
#   script/validate.sh corpus/regulation/nl/**/*.yaml
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# If no files given, validate all regulation YAML files, and the corpus of the
# chronolexografie PoC: its regulation (script/chrono-regulation-dirs.sh) and
# its reductions, which sit with their cell or with none under cells/
# (RFC-048).
if [ $# -eq 0 ]; then
    # A plain assignment, so a failing script stops this one under `set -e`.
    CHRONO_DIRS=$("$REPO_ROOT/script/chrono-regulation-dirs.sh")
    FILES=()
    while IFS= read -r -d '' f; do
        FILES+=("$f")
    # Exclude dot-prefixed sidecars (e.g. .enrichment.yaml, .enrichment-result.yaml)
    # — enrichment metadata/result envelopes, not law files. `find -name '*.yaml'`
    # matches leading-dot names, so filter them out explicitly.
    done < <({
        find "$REPO_ROOT/corpus/regulation" -name '*.yaml' ! -name '.*' -print0
        while IFS= read -r dir; do
            find "$REPO_ROOT/$dir" -name '*.yaml' ! -name '.*' -print0
        done <<< "$CHRONO_DIRS"
        find "$REPO_ROOT/packages/poc-chronolexografie/corpus/cells" -path '*/reductions/*' -name '*.yaml' -print0
    } | sort -z)
    set -- "${FILES[@]}"
fi

exec cargo run --manifest-path "$REPO_ROOT/packages/engine/Cargo.toml" --features validate --bin validate -- "$@"
