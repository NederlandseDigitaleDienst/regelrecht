#!/usr/bin/env bash
# Legt de gegevens van een testprofiel vast zoals de behandelaar dat doet, via
# de API van een instantie in test mode. Alleen voor het voorbereiden van een
# demo: in de demo zelf legt de behandelaar ze met de hand vast.
#
#   packages/wetsproces/iit-groningen/leg-testprofiel-vast.sh 999990044
#   WETSPROCES_URL=http://localhost:7181 ... (standaard)
set -euo pipefail
bsn="${1:?geef het BSN van een profiel uit testprofielen.json}"
url="${WETSPROCES_URL:-http://localhost:7181}"
hier="$(cd "$(dirname "$0")" && pwd)"
jar="$(mktemp)"
trap 'rm -f "$jar"' EXIT

curl -sf -c "$jar" -X POST -H 'content-type: application/json' \
  -d '{"sub":"behandelaar-gemeente_groningen","rollen":["wetsproces-behandelaar"],"claims":{},"naam":"Behandelaar"}' \
  "$url/auth/testlogin" -o /dev/null

body="$(python3 -I - "$hier/testprofielen.json" "$bsn" <<'PY'
import json, sys
profielen = json.load(open(sys.argv[1]))["profielen"]
p = next((p for p in profielen if p["bsn"] == sys.argv[2]), None)
if p is None:
    sys.exit(f"geen profiel met BSN {sys.argv[2]}")
velden = dict(p["gegevens"], bsn_aanvrager=p["bsn"])
print(json.dumps({"cel": "gemeente_groningen",
                  "feit": "beleid_iit_gemeente_groningen#1#gegevens_voor_iit_vastgesteld",
                  "velden": velden}))
PY
)"

curl -sf -b "$jar" -X POST -H 'content-type: application/json' \
  -H "Idempotency-Key: profiel-$bsn-$(date +%s)" -d "$body" "$url/api/backoffice/vastleggingen" -o /dev/null
echo "Gegevens van $bsn vastgelegd op $url."
