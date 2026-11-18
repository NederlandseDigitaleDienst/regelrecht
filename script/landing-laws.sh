#!/usr/bin/env bash
# Copy the laws the landing-page scenario needs into the docs project, so the
# engine can load them in the visitor's browser.
#
# The list is derived from the scenario rather than written out here. The
# Background names every law it loads, and a law added there without being
# copied would fail at run time, in the browser, on the deployed page -- the
# one place where nobody is watching a test. Reading the feature file keeps the
# two in step by construction.
#
# Everything this writes is generated and gitignored; `just landing-laws`
# re-creates it from the corpus.
set -euo pipefail

# The repository root, or whatever stands in for it. The Docker build lays the
# pieces out differently from a checkout (the docs project is /app, the corpus
# is /corpus), so it passes the roots in rather than having them derived from
# where this script happens to sit.
repo="${LANDING_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
docs="${LANDING_DOCS_ROOT:-$repo/docs}"

scenario="$repo/corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature"
out="$docs/public/laws"

[ -f "$scenario" ] || { echo "landing-laws: scenario not found: $scenario" >&2; exit 1; }

# The calculation date the scenario fixes; it decides which version of each law
# applies. Read from the feature file for the same reason as the law list.
DATE=$(grep -oE 'calculation date is "[0-9]{4}-[0-9]{2}-[0-9]{2}"' "$scenario" \
    | head -1 | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}')
[ -n "$DATE" ] || { echo "landing-laws: no calculation date in $scenario" >&2; exit 1; }

# `Given law "x" is loaded`, plus the law under test itself, which the scenario
# evaluates rather than loads.
laws=$(grep -oE 'law "[a-z0-9_]+" is loaded' "$scenario" | sed -E 's/law "(.*)" is loaded/\1/')
laws=$(printf '%s\nwet_op_de_zorgtoeslag\n' "$laws" | sort -u)

rm -rf "$out"
mkdir -p "$out"

manifest="$out/manifest.json"
files=()

count=0
while IFS= read -r law; do
    [ -n "$law" ] || continue
    src=$(find "$repo/corpus/regulation" -type d -name "$law" -print -quit)
    [ -n "$src" ] || { echo "landing-laws: no such law in the corpus: $law" >&2; exit 1; }

    # Every version of the law, not one picked here. The engine holds multiple
    # versions of the same law side by side and selects on the calculation date
    # at execution time, which is exactly how the BDD runner feeds it. Choosing
    # a version in this script would reimplement that selection, and get it
    # wrong: wet_basisregistratie_personen's only text is dated 2025-02-12,
    # after the scenario's own date, so a strict "not later than" rule drops a
    # law the scenario needs while the engine is content.
    versions=$(find "$src" -maxdepth 1 -name '*.yaml' | sort)
    [ -n "$versions" ] || { echo "landing-laws: no versions of $law in the corpus" >&2; exit 1; }

    mkdir -p "$out/$law"
    while IFS= read -r file; do
        base=$(basename "$file")
        cp "$file" "$out/$law/$base"
        files+=("laws/$law/$base")
    done <<< "$versions"

    count=$((count + 1))
done <<< "$laws"

# The page fetches what this manifest lists. Without it the browser would have
# to guess filenames, and a version added to the corpus would go unnoticed.
{
    printf '{\n  "calculationDate": "%s",\n  "scenario": "scenario.feature",\n  "laws": [\n' "$DATE"
    for i in "${!files[@]}"; do
        printf '    "%s"%s\n' "${files[$i]}" "$([ $((i + 1)) -lt ${#files[@]} ] && echo ,)"
    done
    printf '  ]\n}\n'
} > "$manifest"

# The scenario itself: the page runs it, so it has to ship with the laws.
cp "$scenario" "$out/scenario.feature"

# The canonical-grammar runner, copied in rather than imported.
#
# docs/ sits outside the npm workspace on purpose: it has its own lockfile and
# the image builds from docs/ alone. Making it a workspace member to reach
# @regelrecht/frontend-shared would put the docs build behind a monorepo-wide
# install. Copying would normally cost a staleness risk in exchange, but the
# image runs this script on every build, so what ships is always a fresh copy of
# the shared package; only a working directory can hold an old one.
shared="$repo/packages/frontend-shared/src"
gherkin_out="$docs/src/lib/gherkin"

# The directory holds nothing but this copy (it is gitignored), so it starts
# empty. The imports carry no extension and Vite tries .js before .ts, so a JS
# file left by a run from before the runner became TypeScript would otherwise
# win over the fresh copy.
rm -rf "$gherkin_out"
mkdir -p "$gherkin_out"
for f in "$shared"/gherkin/*.js "$shared"/gherkin/*.ts "$shared/values.ts"; do
    base=$(basename "$f")
    # Tests belong to the package, not to the page.
    case "$base" in *.test.*) continue ;; esac
    cp "$f" "$gherkin_out/$base"
done

# values.ts sits a directory up in the package, so the imports that reach for it
# have to be flattened to match the copy's own flat layout.
for f in "$gherkin_out"/*.ts; do
    sed -i.bak "s#'\.\./values'#'./values'#g" "$f"
    rm -f "$f.bak"
done

echo "landing-laws: $count laws (${#files[@]} versions) + scenario + gherkin runner copied"
