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
# The only route to VLAM is the ZAD `vlam` service: it gives the pod
# VLAM_API_URL, a proxy to SSC-ICT's VLAM API that speaks the OpenAI API under
# /v1. Outside ZAD there is no VLAM, and the opencode provider is left without
# an endpoint rather than pointed at a public one.
vlam_base=""
if [ -n "$VLAM_API_URL" ]; then
  vlam_base="${VLAM_API_URL%/}/v1"
fi

# --- VLAM check ---
# Log whether the key works and what the configured model reports, so a deploy
# shows it without anyone having to get into the pod. Diagnostic only: it runs
# in the background so it never delays or stops the worker, and it never
# prints the key.
if [ -n "$vlam_base" ] && [ -n "$VLAM_API_KEY" ]; then
  # In a subshell, so the process is not left a zombie child of the worker
  # that this script execs into.
  (VLAM_BASE="$vlam_base" node -e "
    const base = process.env.VLAM_BASE;
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
      // An enrichment only works if the model calls tools: it edits the law
      // through them. Offer one and see whether it is used.
      try {
        const res = await fetch(base + '/chat/completions', {
          method: 'POST',
          headers,
          body: JSON.stringify({
            model,
            max_tokens: 400,
            messages: [{ role: 'user', content: 'Schrijf de tekst hallo naar het bestand a.txt. Gebruik de tool.' }],
            tools: [{
              type: 'function',
              function: {
                name: 'write_file',
                description: 'Write text to a file',
                parameters: {
                  type: 'object',
                  properties: { path: { type: 'string' }, text: { type: 'string' } },
                  required: ['path', 'text'],
                },
              },
            }],
          }),
          signal: AbortSignal.timeout(60000),
        });
        const body = await res.text();
        let msg = null;
        try { msg = JSON.parse(body).choices[0]; } catch {}
        const calls = msg && msg.message && msg.message.tool_calls;
        console.error('VLAM check tools: HTTP ' + res.status + ', ' +
          (calls && calls.length
            ? 'tool call: ' + flat(JSON.stringify(calls.map((c) => c.function)))
            : 'no tool call, finish_reason=' + (msg && msg.finish_reason) + ', ' +
              (msg ? 'content: ' + flat(String(msg.message && msg.message.content)) : 'body: ' + flat(body))));
      } catch (e) {
        console.error('VLAM check tools: ' + e.name + ': ' + e.message);
      }
    })();
  " &)
fi

# Set up opencode config in user-writable location, with the endpoint above.
mkdir -p "$HOME/.config/opencode"
VLAM_BASE="$vlam_base" node -e "
  const fs = require('fs');
  const config = JSON.parse(fs.readFileSync('/opt/opencode/opencode.json', 'utf8'));
  if (process.env.VLAM_BASE) config.provider.vlam.options = { baseURL: process.env.VLAM_BASE };
  fs.writeFileSync(process.argv[1], JSON.stringify(config, null, 2));
" "$HOME/.config/opencode/opencode.json"
if [ -z "$vlam_base" ]; then
  echo "WARNING: VLAM_API_URL is not set (no ZAD vlam service); the opencode provider has no endpoint" >&2
fi

# --- opencode binary ---
# The image carries two opencode builds (see the Dockerfile): one that needs
# AVX2 and a baseline build without. Use the AVX2 build where the CPU has it,
# as opencode's own installer would. The worker spawns this binary directly, so
# its memory watchdog measures opencode itself.
if [ -z "$OPENCODE_PATH" ] && [ -z "$LLM_PATH" ]; then
  oc_root="$(npm root -g 2>/dev/null || true)"
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

# --- Claude auth ---
# The claude provider authenticates with a personal Claude subscription via
# CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), read directly from env — no file
# is written. ANTHROPIC_API_KEY is intentionally NOT used: it is not forwarded to
# the claude subprocess (see LLM_ENV_ALLOWLIST), so it can never take precedence
# over the OAuth token, even if it is still set here.

exec regelrecht-enrich-worker
