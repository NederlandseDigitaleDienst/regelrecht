#!/usr/bin/env bash
# Install the packages the docs accessibility gate needs inside the Playwright
# image, without apt. Used by the docs-a11y job in .github/workflows/ci.yml.
#
# The image lacks two things the gate needs. Puppeteer unpacks its Chrome
# download with `unzip`. Without DejaVu, fontconfig resolves `monospace` to a
# CJK font (WenQuanYi Zen Hei Mono) whose wider glyphs make code blocks
# overflow, and axe then reports them as scrollable regions without keyboard
# access (it did on /components/demo).
#
# Why not `apt-get install`: the image's apt sources point at the Azure Ubuntu
# mirror, the same one the runners use, and on a bad day that mirror serves
# packages at a few kB/s; a C compiler once took 22 minutes there. `apt-get
# update` also downloads some 34 MB of indexes for 1.5 MB of packages. These
# are the exact .debs `apt-get install --no-install-recommends unzip
# fonts-dejavu-core` picks in the image (fonts-dejavu-mono is a dependency of
# fonts-dejavu-core), so dpkg installs the same files, fontconfig
# configuration included.
#
# Sources, tried in the order of SOURCES below. Launchpad's copy of the Ubuntu
# archive comes first: the librarian keeps every version that was ever
# published, so a URL keeps working after noble-updates moves on. The other
# four are mirrors of the archive pool, which drops superseded versions.
# archive.ubuntu.com and azure.archive.ubuntu.com (the mirror in the image's
# apt sources) are names under ubuntu.com, and launchpad.net is Canonical's
# too. On 2026-11-11 public resolvers returned SERVFAIL for both domains,
# none of the three names resolved in the container, and every PR that
# touched the docs was blocked.
# The last two sources do not depend on Canonical's DNS: the Azure mirror
# under its own Azure name, and the kernel.org mirror. A source that does not
# answer at all is skipped for the remaining files, so a dead host costs its
# timeout only once; one that answers with an HTTP error or the wrong bytes
# still gets the next file.
#
# Every file is checked against the SHA256 below, wherever it came from: a
# mirror can only yield the bytes the signed package index names.
# The plain-http Azure name is what the image's apt uses; the checksum, not
# the transport, is what makes a file trustworthy.
#
# The list is only right for the image ci.yml pins, named on the line below;
# script/playwright-image.test.mjs fails when the two differ. To refresh it after
# moving to a newer Playwright image, run in that image:
#   apt-get update
#   apt-get install --no-install-recommends --print-uris -y unzip fonts-dejavu-core
# and put each listed file name below, with the SHA256 that
#   apt-cache show <package>=<version>
# prints for it (from the signed package index) and the pool directory of its
# `Filename:` line. Check it against the file at
#   https://launchpad.net/ubuntu/+archive/primary/+files/<file>
#
# script/install-a11y-debs.test.sh covers the fallback and the checksum, with
# `curl` and `dpkg` as stubs. It sets A11Y_DEBS, A11Y_SOURCES and DPKG; CI
# sets none of them.
set -euo pipefail

# Made for: mcr.microsoft.com/playwright:v1.64.1-noble

DEBS="${A11Y_DEBS:-
a505b9d491386167bd8e14e3383315a4a7d6539e4406745901ccf009a7988271  unzip_6.0-28ubuntu4.1_amd64.deb  main/u/unzip
40049660c194f3b8a2541fc7369efebb10e9f94bdac836a2f38fafedd10fa73a  fonts-dejavu-core_2.37-8_all.deb  main/f/fonts-dejavu
8a599d6553307db7ecb795d2f0e5a301e03234afc75c7358b0ba43466454c89a  fonts-dejavu-mono_2.37-8_all.deb  main/f/fonts-dejavu
}"

# One source per line: its layout and a URL prefix. A `flat` source holds
# every file in one directory (Launchpad); for a `pool` mirror the script
# appends the pool directory, `<prefix><pool dir>/<file>`.
SOURCES="${A11Y_SOURCES:-
flat https://launchpad.net/ubuntu/+archive/primary/+files/
pool https://archive.ubuntu.com/ubuntu/pool/
pool http://azure.archive.ubuntu.com/ubuntu/pool/
pool http://ubuntu-archive-asm.trafficmanager.net/ubuntu/pool/
pool https://mirrors.edge.kernel.org/ubuntu/pool/
}"

DPKG="${DPKG:-dpkg}"

dir="$(mktemp -d)"
trap 'rm -rf "${dir:?}"' EXIT

# No --retry: the next source is the retry, and a curl retry would double
# what a dead host costs.
fetch() {
  curl -fsSL --connect-timeout 10 --max-time 120 -o "$1" "$2"
}

verify() {
  echo "$sha  $dir/$file" | sha256sum -c --status -
}

dead=""
files=()
while read -r sha file pool; do
  [ -n "$file" ] || continue
  files+=("$dir/$file")
  got=no
  while read -r layout source; do
    [ -n "$source" ] || continue
    case " $dead " in *" $source "*) continue ;; esac
    case "$layout" in
      flat) url="$source$file" ;;
      pool) url="$source$pool/$file" ;;
      *) echo "unknown source layout '$layout' for $source" >&2; exit 1 ;;
    esac
    status=0
    fetch "$dir/$file" "$url" || status=$?
    if [ "$status" -eq 0 ] && verify; then
      echo "$file: from $url" >&2
      got=yes
      break
    fi
    rm -f "${dir:?}/$file"
    if [ "$status" -eq 0 ]; then
      echo "$file: $url has other bytes than SHA256 $sha" >&2
    elif [ "$status" -eq 22 ]; then
      # The host answered with an HTTP error: a mirror that dropped this
      # version may still hold the next file.
      echo "$file: $url gave an HTTP error" >&2
    else
      # No answer at all (DNS, connect, timeout): skip the host from now on.
      echo "$file: $source did not answer (curl exit $status); skipping it from now on" >&2
      dead="$dead $source"
    fi
  done <<< "$SOURCES"
  if [ "$got" = no ]; then
    echo "$file: no source delivered a file with SHA256 $sha" >&2
    exit 1
  fi
done <<< "$DEBS"

"$DPKG" -i "${files[@]}"
