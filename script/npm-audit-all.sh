#!/usr/bin/env bash
# Zoekt elke package-lock.json in de boom en auditeert hem.
#
# Een handmatige opsomming dreef twee keer weg: docs/ viel jarenlang buiten de
# scan, en packages/arch-extract/ui kwam pas boven toen Dependabot er zelf een
# advisory op vond.
#
# Een lockfile met een eigen naam (`<naam>-package-lock.json`) heeft geen
# package.json naast zich. `npm audit` in die map auditeert dan een lege boom en
# meldt nul kwetsbaarheden, wat niet van een echte schone uitslag te
# onderscheiden is. Zo'n paar gaat daarom onder standaardnamen naar een tijdelijke
# map.
set -euo pipefail

cd "$(dirname "$0")/.."
root=$PWD

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

status=0
found=0

audit_in() {
    echo "== npm audit: $1 =="
    (cd "$2" && npm audit) || status=1
}

while IFS= read -r lock; do
    found=$((found + 1))
    dir=$(dirname "$lock")
    base=$(basename "$lock")

    if [ "$base" = "package-lock.json" ]; then
        audit_in "$dir" "$root/$dir"
        continue
    fi

    manifest="${dir}/${base%-lock.json}.json"
    if [ ! -f "$manifest" ]; then
        echo "FOUT: $lock heeft geen bijbehorende $manifest, dus er valt niets te auditeren" >&2
        status=1
        continue
    fi

    work="$tmp/$(echo "$lock" | tr / _)"
    mkdir -p "$work"
    cp "$root/$manifest" "$work/package.json"
    cp "$root/$lock" "$work/package-lock.json"
    audit_in "$lock" "$work"
done < <(git ls-files '*package-lock.json')

if [ "$found" -eq 0 ]; then
    echo "FOUT: geen enkele package-lock.json gevonden; de scan zegt zo niets" >&2
    exit 1
fi

echo "${found} lockfile(s) geauditeerd"
exit "$status"
