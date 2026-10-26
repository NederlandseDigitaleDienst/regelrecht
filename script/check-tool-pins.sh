#!/usr/bin/env bash
# De cargo-chef-pin (versie + beide checksums) staat in elke Dockerfile die
# cargo-chef gebruikt. Een die er niet in staat, drijft stil weg: de twee
# poc-images stonden er niet in en hadden daardoor een andere rust-digest dan
# frontend, onder dezelfde tag.
# Uiteenlopen is stil: de build die de verkeerde pin heeft faalt pas als iemand
# dat image toevallig koud bouwt, en dan met een checksum-fout zonder oorzaak.
set -euo pipefail

cd "$(dirname "$0")/.."

files=(frontend/Dockerfile packages/admin/Dockerfile packages/pipeline/Dockerfile
       packages/poc-portal/Dockerfile packages/poc-napp/Dockerfile)
reference=""
status=0

for f in "${files[@]}"; do
    pins=$(grep -E '^ARG CARGO_CHEF_(VERSION|SHA256_[A-Z0-9]+)=' "$f" | sort)
    if [ -z "$pins" ]; then
        echo "FOUT: $f heeft geen ARG CARGO_CHEF_*-regels" >&2
        status=1
        continue
    fi
    if [ -z "$reference" ]; then
        reference="$pins"
        reference_file="$f"
    elif [ "$pins" != "$reference" ]; then
        echo "FOUT: de cargo-chef-pin in $f wijkt af van die in $reference_file:" >&2
        diff <(echo "$reference") <(echo "$pins") >&2 || true
        status=1
    fi
done

if [ "$status" -eq 0 ]; then
    echo "cargo-chef-pin is gelijk in ${#files[@]} Dockerfiles"
fi
exit "$status"
