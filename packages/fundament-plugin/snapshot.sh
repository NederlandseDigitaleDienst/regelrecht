# The snapshot Job of the wetsproces plugin (snapshot.go): it fetches one
# commit of the case repository and writes to stdout a tar.gz of the manifest
# and the files it names, which the plugin reads from the log and stores in
# ConfigMaps. It runs in alpine/git, with yq copied in by an init container.
#
# In: CASE_REPO, CASE_REF and CASE_PATH (the manifest), MAX_BYTES, and for a
# private repository GIT_USERNAME and GIT_TOKEN. Out, on stdout:
#
#   wetsproces-snapshot commit=<40 hex> bytes=<n>
#   <the tar.gz in base64, in lines>
#   wetsproces-snapshot end
#
# On failure it writes why to the termination message, one line, which the
# plugin shows in the status of the Wetsproces.
set -eu
# Paths are file names, never patterns.
export GIT_LITERAL_PATHSPECS=1

YQ="${YQ:-/tools/yq}"
WORK="${WORK:-/work}"
TERMINATION_LOG="${TERMINATION_LOG:-/dev/termination-log}"

fail() {
  echo "$*" >&2
  printf '%s' "$*" > "$TERMINATION_LOG" 2>/dev/null || true
  exit 1
}

# norm prints path $1 without . and empty parts and with .. resolved, or
# fails when .. climbs above the root of the repository.
norm() {
  printf '%s\n' "$1" | awk -F/ '{
    n = 0
    for (i = 1; i <= NF; i++) {
      if ($i == "" || $i == ".") continue
      if ($i == "..") { if (n == 0) { print "\001"; exit } n--; continue }
      p[++n] = $i
    }
    s = ""
    for (i = 1; i <= n; i++) s = s (i > 1 ? "/" : "") p[i]
    print s
  }'
}

# under prints repository path $2 under the root $1.
under() {
  r="$(norm "$1/$2")"
  [ "$r" != "$(printf '\001')" ] || fail "the manifest $CASE_PATH names '$2', which lies outside the repository"
  printf '%s' "$r"
}

# A name in the manifest lies below its root: no .., no . and no empty part,
# as the server's own check (packages/corpus/src/source_manifest.rs).
below() {
  case "/$(printf '%s' "$1" | sed 's#^/*##; s#/*$##')/" in
    *//* | */./* | */../* | //) fail "the manifest $CASE_PATH: '$1' is no directory below the root" ;;
  esac
}

git_says() {
  # The last line git wrote, with the token never in it: git does not print
  # it, and the helper takes it from the environment.
  tail -n 1 "$WORK/git.err" | sed 's/^fatal: //; s/^error: //'
}

git init -q "$WORK/case"
cd "$WORK/case"
# Names as they are, also with a letter outside ASCII (beëindiging.yaml):
# quoted, the filter on .yaml below would miss them.
git config core.quotePath false
if [ -n "${GIT_TOKEN:-}" ]; then
  # git asks this helper for the credentials; it reads them from the
  # environment, so they never appear in the arguments of a process.
  git config credential.helper '!f() { echo "username=${GIT_USERNAME}"; echo "password=${GIT_TOKEN}"; }; f'
fi

# Fetch rather than clone: `git fetch` takes a branch, a tag or a commit.
# Without the contents of the files (blob:none): git fetches those it checks
# out below, in one go. A server without partial clone sends them all, which
# only costs time.
if ! git fetch -q --depth=1 --filter=blob:none "$CASE_REPO" "$CASE_REF" 2>"$WORK/git.err"; then
  said="$(git_says)"
  case "$said" in
    *"Authentication failed"* | *"could not read Username"* | *"terminal prompts disabled"* | *" 401"* | *" 403"*)
      fail "the git server refused access to $CASE_REPO: check the token in the Secret of credentialsSecretRef, which needs read access ($said)" ;;
    *"couldn't find remote ref"* | *"not our ref"*)
      fail "$CASE_REPO has no branch, tag or commit $CASE_REF ($said)" ;;
    *"not found"* | *"Not Found"*)
      fail "$CASE_REPO is not there, or is private and needs a token in credentialsSecretRef ($said)" ;;
    *)
      fail "git could not fetch $CASE_REF of $CASE_REPO: $said" ;;
  esac
fi
commit="$(git rev-parse FETCH_HEAD)"

if ! git show "FETCH_HEAD:$CASE_PATH" > "$WORK/manifest.yaml" 2>"$WORK/git.err"; then
  fail "the manifest $CASE_PATH is not in commit $commit of $CASE_REF"
fi
"$YQ" -e 'type == "!!map" and has("directories")' "$WORK/manifest.yaml" >/dev/null 2>&1 ||
  fail "$CASE_PATH is no source manifest: it has no directories"

# repo_root is relative to the directory of the manifest.
base="$(dirname "$CASE_PATH")"
root="$(under "$base" "$("$YQ" '.repo_root // ""' "$WORK/manifest.yaml")")"

: > "$WORK/files"
printf '%s\n' "$CASE_PATH" >> "$WORK/files"

"$YQ" '.directories | to_entries | .[] | [.key, .value] | @tsv' "$WORK/manifest.yaml" > "$WORK/directories"
while IFS="$(printf '\t')" read -r target source; do
  below "$target"
  below "$source"
  dir="$(under "$root" "$source")"
  # The server reads only the YAML files of a directory, and refuses one
  # that has none.
  git ls-tree -r --name-only FETCH_HEAD -- "$dir/" | grep -E '\.ya?ml$' > "$WORK/found" || true
  [ -s "$WORK/found" ] || fail "the manifest $CASE_PATH names '$source' for '$target', which holds no YAML file in commit $commit"
  cat "$WORK/found" >> "$WORK/files"
done < "$WORK/directories"

ui="$("$YQ" '.ui_spec // ""' "$WORK/manifest.yaml")"
if [ -n "$ui" ]; then
  below "$ui"
  spec="$(under "$root" "$ui")"
  git cat-file -e "FETCH_HEAD:$spec" 2>/dev/null ||
    fail "the manifest $CASE_PATH names '$ui' for its UI spec, which is not in commit $commit"
  printf '%s\n' "$spec" >> "$WORK/files"
fi

sort -u -o "$WORK/files" "$WORK/files"
git checkout -q FETCH_HEAD --pathspec-from-file="$WORK/files" 2>"$WORK/git.err" ||
  fail "git could not check out the files of the manifest: $(git_says)"
tar -czf "$WORK/case.tar.gz" -T "$WORK/files"

bytes="$(wc -c < "$WORK/case.tar.gz" | tr -d ' ')"
[ "$bytes" -le "$MAX_BYTES" ] ||
  fail "the case is $bytes bytes packed, more than the $MAX_BYTES a snapshot holds; let the manifest name fewer directories"

echo "wetsproces-snapshot commit=$commit bytes=$bytes"
base64 "$WORK/case.tar.gz"
echo "wetsproces-snapshot end"
