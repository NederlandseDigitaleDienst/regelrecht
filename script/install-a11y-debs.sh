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
# Source: Launchpad's copy of the Ubuntu archive. The librarian keeps every
# version that was ever published, so a URL keeps working after noble-updates
# moves on; the archive pool drops superseded versions.
#
# The list is only right for the image ci.yml pins, named on the line below;
# script/playwright-image.test.mjs fails when the two differ. To refresh it after
# moving to a newer Playwright image, run in that image:
#   apt-get update
#   apt-get install --no-install-recommends --print-uris -y unzip fonts-dejavu-core
# and put each listed file name below, with the SHA256 that
#   apt-cache show <package>=<version>
# prints for it (from the signed package index). Check it against the file at
#   https://launchpad.net/ubuntu/+archive/primary/+files/<file>
set -euo pipefail

# Made for: mcr.microsoft.com/playwright:v1.64.1-noble

DEBS="
a505b9d491386167bd8e14e3383315a4a7d6539e4406745901ccf009a7988271  unzip_6.0-28ubuntu4.1_amd64.deb
40049660c194f3b8a2541fc7369efebb10e9f94bdac836a2f38fafedd10fa73a  fonts-dejavu-core_2.37-8_all.deb
8a599d6553307db7ecb795d2f0e5a301e03234afc75c7358b0ba43466454c89a  fonts-dejavu-mono_2.37-8_all.deb
"

dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT

while read -r sha file; do
  [ -n "$file" ] || continue
  curl -fsSL --connect-timeout 10 --max-time 120 --retry 3 --retry-connrefused -o "$dir/$file" "https://launchpad.net/ubuntu/+archive/primary/+files/$file"
  echo "$sha  $dir/$file" | sha256sum -c -
done <<< "$DEBS"

dpkg -i "$dir"/*.deb
