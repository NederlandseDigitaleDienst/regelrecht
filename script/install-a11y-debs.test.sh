#!/usr/bin/env bash
# Covers the paths in script/install-a11y-debs.sh that decide where a .deb
# comes from: the fallback over the sources, skipping a host that does not
# answer, the SHA256 check, and the cache. `curl` and `dpkg` are stubs, so
# nothing goes over the network and nothing is installed. The .debs are two
# small fake files with their own checksums.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="$here/install-a11y-debs.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp:?}"' EXIT

pass=0
fail=0

mkdir -p "$tmp/bin" "$tmp/fixtures"
printf 'first package\n' >"$tmp/fixtures/a_1_all.deb"
printf 'second package\n' >"$tmp/fixtures/b_1_all.deb"
sha_a="$(sha256sum "$tmp/fixtures/a_1_all.deb" | cut -d' ' -f1)"
sha_b="$(sha256sum "$tmp/fixtures/b_1_all.deb" | cut -d' ' -f1)"
DEBS="
$sha_a  a_1_all.deb  main/a/a
$sha_b  b_1_all.deb  main/b/b
"
# Same shape as the real list: Launchpad first, then pool mirrors.
SOURCES="
flat https://launchpad.example/files/
pool https://one.example/ubuntu/pool/
pool https://two.example/ubuntu/pool/
"

# The curl stub. Per host, the file $tmp/hosts/<host> says what it does:
#   dead     no answer (curl exit 6, as for a name that does not resolve)
#   serve    serves every fixture
#   wrong    serves other bytes than the fixture
#   only:<f> serves only fixture <f>, an HTTP error (exit 22) for the rest
# A host without a file answers with an HTTP error. Every URL goes to
# $tmp/curl.log.
cat >"$tmp/bin/curl" <<'STUB'
#!/usr/bin/env bash
out="" url=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    --connect-timeout|--max-time|--retry) shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done
echo "$url" >>"$STUB_DIR/curl.log"
host="${url#*://}"; host="${host%%/*}"
file="${url##*/}"
mode="$(cat "$STUB_DIR/hosts/$host" 2>/dev/null || echo http-error)"
case "$mode" in
  dead) echo "curl: (6) Could not resolve host: $host" >&2; exit 6 ;;
  serve) cp "$STUB_DIR/fixtures/$file" "$out" ;;
  wrong) echo tampered >"$out" ;;
  only:"$file") cp "$STUB_DIR/fixtures/$file" "$out" ;;
  *) echo "curl: (22) The requested URL returned error: 404" >&2; exit 22 ;;
esac
STUB
cat >"$tmp/bin/dpkg" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$STUB_DIR/dpkg.log"
STUB
chmod +x "$tmp/bin/curl" "$tmp/bin/dpkg"

# Resets the stubs. Arguments are host=mode pairs.
hosts() {
  rm -rf "${tmp:?}/hosts" "${tmp:?}/curl.log" "${tmp:?}/dpkg.log"
  mkdir -p "$tmp/hosts"
  : >"$tmp/curl.log"
  for pair in "$@"; do
    echo "${pair#*=}" >"$tmp/hosts/${pair%%=*}"
  done
}

# Runs the script with the stubs on PATH. Extra arguments go to the script.
# Sets $status and $out.
run() {
  out="$(PATH="$tmp/bin:$PATH" STUB_DIR="$tmp" A11Y_DEBS="$DEBS" \
    A11Y_SOURCES="${SOURCES_OVERRIDE-$SOURCES}" DPKG=dpkg \
    bash "$script" "$@" 2>&1)"
  status=$?
}

ok() { echo "ok: $1"; pass=$((pass + 1)); }
bad() {
  echo "FAIL: $1"
  echo "$out" | sed 's/^/    /'
  [ -f "$tmp/curl.log" ] && sed 's/^/    curl: /' "$tmp/curl.log"
  fail=$((fail + 1))
}

# $1 = name, $2 = expected exit status.
expect_status() {
  if [ "$status" -eq "$2" ]; then ok "$1"; else bad "$1 (exit $status, expected $2)"; fi
}

# $1 = name, $2 = pattern that must occur as a line in curl.log this many
# times ($3).
expect_calls() {
  local n
  n="$(grep -cxF "$2" "$tmp/curl.log")"
  if [ "$n" -eq "$3" ]; then ok "$1"; else bad "$1 ($2 called $n times, expected $3)"; fi
}

# $1 = name. dpkg got both packages, in list order.
expect_installed() {
  if [ -f "$tmp/dpkg.log" ] &&
    [ "$(sed 's#.*/##' "$tmp/dpkg.log" | tr '\n' ' ')" = "-i a_1_all.deb b_1_all.deb " ]; then
    ok "$1"
  else
    bad "$1 (dpkg was not called with both packages)"
  fi
}

# --- Sources --------------------------------------------------------------

hosts launchpad.example=serve
run
expect_status "Launchpad answering installs from Launchpad" 0
expect_calls "Launchpad URL ends in the file name" "https://launchpad.example/files/a_1_all.deb" 1
expect_calls "a mirror is not asked when Launchpad answers" "https://one.example/ubuntu/pool/main/a/a/a_1_all.deb" 0
expect_installed "Launchpad answering hands both files to dpkg"

hosts launchpad.example=dead one.example=dead two.example=serve
run
expect_status "two dead sources fall through to the third" 0
expect_calls "a dead Launchpad is asked once, not per file" "https://launchpad.example/files/a_1_all.deb" 1
expect_calls "a dead Launchpad is skipped for the next file" "https://launchpad.example/files/b_1_all.deb" 0
expect_calls "a dead mirror is skipped for the next file" "https://one.example/ubuntu/pool/main/b/b/b_1_all.deb" 0
expect_calls "a pool mirror gets the pool directory in its URL" "https://two.example/ubuntu/pool/main/b/b/b_1_all.deb" 1
expect_installed "the third source's files reach dpkg"

hosts launchpad.example=dead one.example=dead two.example=dead
run
expect_status "no source answering fails" 1
if [ -f "$tmp/dpkg.log" ]; then bad "dpkg runs although nothing was downloaded"; else ok "dpkg does not run when a download fails"; fi
if grep -qF "no source delivered" <<<"$out"; then ok "the failure names the file"; else bad "the failure names the file"; fi

hosts one.example=only:b_1_all.deb two.example=serve
run
expect_status "an HTTP error on one file falls through" 0
expect_calls "a mirror that gave an HTTP error still gets the next file" "https://one.example/ubuntu/pool/main/b/b/b_1_all.deb" 1
expect_calls "the file the mirror lacked came from the next one" "https://two.example/ubuntu/pool/main/a/a/a_1_all.deb" 1
expect_calls "the file the mirror had was not fetched twice" "https://two.example/ubuntu/pool/main/b/b/b_1_all.deb" 0

hosts launchpad.example=wrong one.example=serve
run
expect_status "a file with the wrong SHA256 is not accepted" 0
expect_calls "after wrong bytes the next source is asked" "https://one.example/ubuntu/pool/main/a/a/a_1_all.deb" 1
expect_calls "a source with wrong bytes still gets the next file" "https://launchpad.example/files/b_1_all.deb" 1
expect_installed "the right bytes reach dpkg"

hosts launchpad.example=wrong one.example=wrong two.example=wrong
run
expect_status "only wrong bytes everywhere fails" 1

# --- Cache ----------------------------------------------------------------

cache="$tmp/cache"
hosts launchpad.example=dead one.example=serve
run "$cache"
expect_status "a cold cache downloads" 0
if cmp -s "$cache/a_1_all.deb" "$tmp/fixtures/a_1_all.deb" && cmp -s "$cache/b_1_all.deb" "$tmp/fixtures/b_1_all.deb"; then
  ok "a download lands in the cache directory"
else
  bad "a download lands in the cache directory"
fi

hosts launchpad.example=dead one.example=dead two.example=dead
run "$cache"
expect_status "a warm cache needs no source" 0
if [ -s "$tmp/curl.log" ]; then bad "a warm cache still calls curl"; else ok "a warm cache calls no curl"; fi
expect_installed "the cached files reach dpkg"

echo tampered >"$cache/b_1_all.deb"
hosts two.example=serve
run "$cache"
expect_status "a damaged cached file is fetched again" 0
expect_calls "only the damaged file is fetched" "https://two.example/ubuntu/pool/main/a/a/a_1_all.deb" 0
if cmp -s "$cache/b_1_all.deb" "$tmp/fixtures/b_1_all.deb"; then
  ok "the damaged file is replaced in the cache"
else
  bad "the damaged file is replaced in the cache"
fi

echo tampered >"$cache/a_1_all.deb"
hosts launchpad.example=dead one.example=dead two.example=dead
run "$cache"
expect_status "a damaged cached file without a source fails" 1
if [ -f "$tmp/dpkg.log" ]; then bad "dpkg runs on a damaged cache"; else ok "dpkg does not run on a damaged cache"; fi

# --- The real source list -------------------------------------------------

# The default SOURCES, with every Canonical name and the Azure mirror's
# ubuntu.com name dead, as on 2026-11-11: the files must still arrive.
hosts launchpad.net=dead archive.ubuntu.com=dead azure.archive.ubuntu.com=dead \
  ubuntu-archive-asm.trafficmanager.net=dead mirrors.edge.kernel.org=serve
SOURCES_OVERRIDE="" run
expect_status "the default sources survive Canonical's DNS being down" 0
expect_calls "the default list falls back to the kernel.org pool" \
  "https://mirrors.edge.kernel.org/ubuntu/pool/main/a/a/a_1_all.deb" 1
order="$(sed 's#^[a-z]*://\([^/]*\)/.*#\1#' "$tmp/curl.log" | head -5 | tr '\n' ' ')"
want="launchpad.net archive.ubuntu.com azure.archive.ubuntu.com ubuntu-archive-asm.trafficmanager.net mirrors.edge.kernel.org "
if [ "$order" = "$want" ]; then ok "the default sources are tried in order"; else bad "the default order is: $order"; fi

hosts azure.archive.ubuntu.com=serve
SOURCES_OVERRIDE="" run
expect_calls "the Azure mirror URL is the one in the image's apt sources" \
  "http://azure.archive.ubuntu.com/ubuntu/pool/main/a/a/a_1_all.deb" 1

echo
echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
