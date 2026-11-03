#!/usr/bin/env bash
# Dekt de toestandskeuze van script/cross-law-integriteit.py.
#
# Waarom juist dit: een wet met meerdere `valid_from`-toestanden deelt één `$id`.
# De loader hield eerder wat glob toevallig als laatste opleverde, dus de controle
# las een willekeurige tekst. Een binding op een term die pas in een nieuwere
# toestand bestaat werd dan als dangling gemeld, terwijl een echte fout in de
# oudere tekst juist onzichtbaar bleef. Beide richtingen worden hier vastgepind.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
gate="$here/cross-law-integriteit.py"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# $1 = map, $2 = valid_from, $3 = declareert de open term? (ja/nee), [$4 = valid_to]
doelwet() {
    mkdir -p "$1"
    cat >"$1/$2.yaml" <<YAML
\$id: doelwet
valid_from: '$2'
YAML
    if [ -n "${4:-}" ]; then
        printf "valid_to: '%s'\n" "$4" >>"$1/$2.yaml"
    fi
    cat >>"$1/$2.yaml" <<'YAML'
articles:
  - number: '1'
    machine_readable:
      execution:
        output:
          - name: uitkomst
YAML
    if [ "$3" = "ja" ]; then
        cat >>"$1/$2.yaml" <<'YAML'
      open_terms:
        - id: open_term_nieuw
          type: amount
YAML
    fi
}

# Een lagere regeling die zich op die open term aanhaakt.
invuller() {
    mkdir -p "$1"
    cat >"$1/2026-01-01.yaml" <<'YAML'
$id: invuller
valid_from: '2026-01-01'
articles:
  - number: '1'
    machine_readable:
      implements:
        - law: doelwet
          article: '1'
          open_term: open_term_nieuw
      execution:
        output:
          - name: invulling
YAML
}

check() { # naam, verwachte exitcode, verwacht patroon in de uitvoer, corpus, [extra args]
    local naam="$1" want_code="$2" want_uit="$3" corpus="$4"
    shift 4
    local uit code
    uit="$(python3 "$gate" "$corpus" "$@" 2>&1)"
    code=$?
    if [ "$code" -eq "$want_code" ] && printf '%s' "$uit" | grep -q "$want_uit"; then
        pass=$((pass + 1))
        printf '  ok   %s\n' "$naam"
    else
        fail=$((fail + 1))
        printf '  FOUT %s — exit %s (verwacht %s), zocht "%s"\n' "$naam" "$code" "$want_code" "$want_uit"
        printf '%s\n' "$uit" | sed 's/^/       /'
    fi
}

echo "cross-law toestandskeuze:"

# 1. Twee toestanden; alleen de nieuwe declareert de open term. Op een peildatum
#    ná die toestand hoort de binding te kloppen — dit is het geval dat vroeger
#    zes valse meldingen opleverde.
c="$tmp/twee"
doelwet "$c/doelwet" 2020-01-01 nee
doelwet "$c/doelwet" 2026-01-01 ja
invuller "$c/invuller"
check "nieuwe toestand geldt → binding klopt" 0 "impl-dangling=0" "$c" --peildatum 2026-06-01

# 2. Dezelfde boom, maar op een peildatum vóór de nieuwe toestand. Dan bestond de
#    open term nog niet en is de melding terecht — de fix mag niet alles groen maken.
check "oude toestand geldt → binding is terecht dangling" 1 "impl-dangling=1" "$c" --peildatum 2021-01-01

# 3. De gekozen toestand hoort zichtbaar te zijn. Een stille keuze is wat de
#    oude loader gevaarlijk maakte.
check "keuze wordt gerapporteerd" 0 "2026-01-01.yaml (uit 2 toestanden)" "$c" --peildatum 2026-06-01

# 4. Alles in de toekomst → de vroegste toestand, zodat een volledig
#    vooruitgedateerd corpus niet ongecontroleerd doorglipt.
check "alles toekomstig → vroegste toestand" 1 "impl-dangling=1" "$c" --peildatum 2010-01-01

# 5. Een wet zonder valid_from blijft laadbaar (geldt altijd).
c2="$tmp/geendatum"
mkdir -p "$c2/doelwet"
cat >"$c2/doelwet/wet.yaml" <<'YAML'
$id: doelwet
articles:
  - number: '1'
    machine_readable:
      open_terms:
        - id: open_term_nieuw
          type: amount
      execution:
        output:
          - name: uitkomst
YAML
invuller "$c2/invuller"
check "wet zonder valid_from blijft laadbaar" 0 "impl-dangling=0" "$c2"

# 6. Zonder --peildatum draait hij op vandaag en blijft hij bruikbaar.
check "zonder --peildatum: vandaag" 0 "impl-dangling=0" "$c"

# 7. De nieuwste toestand is vervallen. De engine valt dan niet terug op een
#    oudere toestand maar meldt de wet als niet meer in werking; de controle leest
#    dus dezelfde tekst, hier die zonder de open term, en zegt dat hij vervallen is.
#    Viel hij terug op 2020, dan stond de binding onterecht groen.
c3="$tmp/vervallen"
doelwet "$c3/doelwet" 2020-01-01 ja
doelwet "$c3/doelwet" 2026-01-01 nee 2026-03-01
invuller "$c3/invuller"
check "vervallen toestand → geen terugval op een oudere" 1 "impl-dangling=1" "$c3" --peildatum 2026-06-01
check "vervallen toestand wordt gemeld" 1 "2026-01-01.yaml vervallen op 2026-03-01" "$c3" --peildatum 2026-06-01

# 8. valid_to is inclusief: op de einddatum zelf geldt de wet nog.
uit="$(python3 "$gate" "$c3" --peildatum 2026-03-01 2>&1)"
if printf '%s' "$uit" | grep -q "vervallen op"; then
    fail=$((fail + 1))
    printf '  FOUT op valid_to zelf nog in werking — toch als vervallen gemeld\n'
    printf '%s\n' "$uit" | sed 's/^/       /'
else
    pass=$((pass + 1))
    printf '  ok   op valid_to zelf nog in werking\n'
fi

# 9. Een reductie die een invoer beantwoordt (RFC-056): de invoer moet bestaan
#    en een competent_authority noemen. Ontbreekt een van beide, dan beantwoordt
#    de cel niets.
afnemer() { # $1 = map, $2 = bron van de invoer
    mkdir -p "$1"
    cat >"$1/2026-01-01.yaml" <<YAML
\$id: afnemer
valid_from: '2026-01-01'
articles:
  - number: '1'
    machine_readable:
      execution:
        input:
          - name: vergunning
            source: $2
YAML
}
reductie() { # $1 = map, $2 = invoer
    mkdir -p "$1"
    cat >"$1/2026-01-01.yaml" <<YAML
\$id: reductie
regulatory_layer: REDUCTIE
valid_from: '2026-01-01'
articles:
  - number: '1'
    machine_readable:
      implements:
        - law: afnemer
          article: '1'
          input: $2
YAML
}
c4="$tmp/invoer"
afnemer "$c4/afnemer" "{competent_authority: {name: inspecteur}}"
reductie "$c4/reductie" vergunning
check "invoer met competent_authority → binding klopt" 0 "impl-dangling=0" "$c4" --peildatum 2026-06-01
c5="$tmp/invoer-onbekend"
afnemer "$c5/afnemer" "{competent_authority: {name: inspecteur}}"
reductie "$c5/reductie" andere_invoer
check "onbekende invoer → dangling" 1 'heeft geen invoer "andere_invoer"' "$c5" --peildatum 2026-06-01
c6="$tmp/invoer-zonder-bevoegde"
afnemer "$c6/afnemer" "{}"
reductie "$c6/reductie" vergunning
check "invoer zonder competent_authority → dangling" 1 'noemt geen competent_authority' "$c6" --peildatum 2026-06-01

echo ""
echo "$pass geslaagd, $fail mislukt"
[ "$fail" -eq 0 ]
