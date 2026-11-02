#!/bin/sh
set -e

# Set HOME so git config and CLI tools work for the app user.
# RIG runs containers with a read-only root filesystem, so we
# use /tmp for all writable state.
export HOME=/tmp/app-home
mkdir -p "$HOME"

# Create output directory at runtime (read-only root filesystem)
OUTPUT_DIR="${REGULATION_REPO_PATH:-/tmp/regulation-repo}"
mkdir -p "$OUTPUT_DIR"
export REGULATION_REPO_PATH="$OUTPUT_DIR"

# Create corpus repo directory at runtime
CORPUS_DIR="${CORPUS_REPO_PATH:-/tmp/corpus-repo}"
mkdir -p "$CORPUS_DIR"
export CORPUS_REPO_PATH="$CORPUS_DIR"

# --- OpenCode/VLAM auth ---
# Write auth.json from VLAM_API_KEY secret so opencode can authenticate
# with the VLAM API. The provider config (opencode.json) is baked into
# the image; only the key is injected at runtime.
if [ -n "$VLAM_API_KEY" ]; then
  mkdir -p "$HOME/.local/share/opencode"
  # Use Node.js to safely JSON-encode the API key, avoiding shell injection
  # and broken JSON from keys containing quotes, backslashes, or percent signs.
  node -e "
    const fs = require('fs');
    const key = process.env.VLAM_API_KEY;
    const data = {vlam: {type: 'api', key: key}};
    fs.writeFileSync(process.argv[1], JSON.stringify(data));
  " "$HOME/.local/share/opencode/auth.json"
  chmod 600 "$HOME/.local/share/opencode/auth.json"
fi

# --- VLAM endpoint ---
# On ZAD the `vlam` service gives the pod VLAM_API_URL, a proxy to SSC-ICT's
# VLAM API that speaks the OpenAI API under /v1. VLAM_BASE_URL still wins, so a
# local run can point somewhere else.
if [ -z "$VLAM_BASE_URL" ] && [ -n "$VLAM_API_URL" ]; then
  VLAM_BASE_URL="${VLAM_API_URL%/}/v1"
  export VLAM_BASE_URL
fi

# --- VLAM check ---
# Log whether the key works and what the configured model reports, so a deploy
# shows it without anyone having to get into the pod. Diagnostic only: it runs
# in the background so it never delays or stops the worker, and it never
# prints the key.
if [ -n "$VLAM_BASE_URL" ] && [ -n "$VLAM_API_KEY" ]; then
  node -e "
    const base = process.env.VLAM_BASE_URL.replace(/\/+\$/, '');
    const model = JSON.parse(require('fs').readFileSync('/opt/opencode/opencode.json', 'utf8'))
      .model.replace(/^vlam\//, '');
    const headers = {
      Authorization: 'Bearer ' + process.env.VLAM_API_KEY,
      'Content-Type': 'application/json',
    };
    const flat = (s) => s.replace(/\s+/g, ' ').slice(0, 400);
    (async () => {
      try {
        const res = await fetch(base + '/models', { headers, signal: AbortSignal.timeout(10000) });
        const body = await res.text();
        let entry = null;
        try { entry = (JSON.parse(body).data || []).find((m) => m.id === model) || 'not listed'; } catch {}
        console.error('VLAM check models: HTTP ' + res.status + ', ' + model + ': ' +
          (entry ? JSON.stringify(entry) : flat(body)));
      } catch (e) {
        console.error('VLAM check models: ' + e.name + ': ' + e.message);
      }
      try {
        const res = await fetch(base + '/chat/completions', {
          method: 'POST',
          headers,
          body: JSON.stringify({ model, max_tokens: 20, messages: [{ role: 'user', content: 'Antwoord met alleen: OK' }] }),
          signal: AbortSignal.timeout(60000),
        });
        const body = await res.text();
        let answer = null;
        try { answer = JSON.parse(body).choices[0].message.content; } catch {}
        console.error('VLAM check chat: HTTP ' + res.status + ', ' +
          (answer !== null ? 'answer: ' + flat(answer) : 'body: ' + flat(body)));
      } catch (e) {
        console.error('VLAM check chat: ' + e.name + ': ' + e.message);
      }
    })();
  " &
fi

# Set up opencode config in user-writable location, with the endpoint above in
# place of the demo URL baked into opencode.json.
mkdir -p "$HOME/.config/opencode"
if [ -n "$VLAM_BASE_URL" ]; then
  node -e "
    const fs = require('fs');
    const config = JSON.parse(fs.readFileSync('/opt/opencode/opencode.json', 'utf8'));
    config.provider.vlam.options.baseURL = process.env.VLAM_BASE_URL;
    fs.writeFileSync(process.argv[1], JSON.stringify(config, null, 2));
  " "$HOME/.config/opencode/opencode.json"
else
  echo "WARNING: neither VLAM_BASE_URL nor VLAM_API_URL is set; using the demo API endpoint baked into opencode.json" >&2
  cp /opt/opencode/opencode.json "$HOME/.config/opencode/opencode.json"
fi
ln -sf /opt/opencode-plugins/node_modules "$HOME/.config/opencode/node_modules"

# --- opencode binary ---
# The image carries two opencode builds (see the Dockerfile): one that needs
# AVX2 and a baseline build without. Use the AVX2 build where the CPU has it,
# as opencode's own installer would. The worker spawns this binary directly, so
# its memory watchdog measures opencode itself.
if [ -z "$OPENCODE_PATH" ]; then
  oc_root="$(npm root -g 2>/dev/null)"
  if grep -qw avx2 /proc/cpuinfo 2>/dev/null; then
    oc_variants="opencode-linux-x64-musl opencode-linux-x64-baseline-musl"
  else
    oc_variants="opencode-linux-x64-baseline-musl"
  fi
  for v in $oc_variants; do
    if [ -x "$oc_root/$v/bin/opencode" ]; then
      OPENCODE_PATH="$oc_root/$v/bin/opencode"
      break
    fi
  done
  if [ -n "$OPENCODE_PATH" ]; then
    export OPENCODE_PATH
    echo "opencode binary: $OPENCODE_PATH" >&2
  else
    echo "WARNING: no opencode build found under $oc_root; falling back to opencode on the PATH" >&2
  fi
fi

# --- Memory sampler (temporary diagnostic) ---
# A VLAM enrichment kills the pod about 20s after opencode starts, without a
# log line. Log the container's memory and the largest processes whenever the
# usage moves by 64 MiB, so the log shows what grows before a kill.
# Remove once the cause is known.
(
  if [ -r /sys/fs/cgroup/memory.current ]; then
    cur=/sys/fs/cgroup/memory.current; max=/sys/fs/cgroup/memory.max
  else
    cur=/sys/fs/cgroup/memory/memory.usage_in_bytes; max=/sys/fs/cgroup/memory/memory.limit_in_bytes
  fi
  echo "MEM limit: $(cat "$max" 2>/dev/null); /tmp: $(grep ' /tmp ' /proc/mounts | cut -d' ' -f1,3,4)" >&2
  last=0
  while sleep 2; do
    used=$(cat "$cur" 2>/dev/null) || continue
    used_mb=$((used / 1048576))
    diff=$((used_mb - last)); [ "$diff" -lt 0 ] && diff=$((-diff))
    if [ "$diff" -ge 64 ]; then
      last=$used_mb
      top=$(for s in /proc/[0-9]*/status; do
              awk '/^Name:/{n=$2} /^Pid:/{p=$2} /^VmRSS:/{print $2, p, n}' "$s" 2>/dev/null
            done | sort -rn | head -3 | awk '{printf "%s(%s) %dMB; ", $3, $2, $1/1024}')
      echo "MEM ${used_mb}MB tmp=$(df -m /tmp 2>/dev/null | awk 'NR==2{print $3}')MB top: $top" >&2
    fi
  done
) &
printf '{"dependencies":{"@ai-sdk/openai-compatible":"*"}}' \
  > "$HOME/.config/opencode/package.json"

# --- Claude auth ---
# The claude provider authenticates with a personal Claude subscription via
# CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), read directly from env — no file
# is written. ANTHROPIC_API_KEY is intentionally NOT used: it is not forwarded to
# the claude subprocess (see LLM_ENV_ALLOWLIST), so it can never take precedence
# over the OAuth token, even if it is still set here.

exec regelrecht-enrich-worker
