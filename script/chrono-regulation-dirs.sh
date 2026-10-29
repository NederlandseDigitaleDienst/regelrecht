#!/usr/bin/env bash
# The regulation directories of the corpus of the chronolexografie PoC, one
# per line, relative to the repository root: the shared regulation, and the
# own regulation of each case (casussen/<id>/regulation). Each holds `nl/`.
# The reductions are in `cells/` beside them, which this list leaves out.
#
# `just bdd-chrono` and CI join them with `:` into REGULATION_PATH; `just
# validate` walks them.
set -euo pipefail

CORPUS=packages/poc-chronolexografie/corpus
cd "$(dirname "$0")/.."
echo "$CORPUS/regulation"
for dir in "$CORPUS"/casussen/*/regulation; do
    [ -d "$dir" ] && echo "$dir"
done
