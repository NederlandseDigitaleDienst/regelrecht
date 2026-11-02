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
# shows it without anyone having to get into the pod. Diagnostic only: it never
# stops the worker, and it never prints the key.
if [ -n "$VLAM_BASE_URL" ] && [ -n "$VLAM_API_KEY" ]; then
  node -e "
    const base = process.env.VLAM_BASE_URL.replace(/\/+\$/, '');
    const model = JSON.parse(require('fs').readFileSync('/etc/opencode/opencode.json', 'utf8'))
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
  " || true
fi

# Set up opencode config in user-writable location, with the endpoint above in
# place of the demo URL baked into opencode.json.
mkdir -p "$HOME/.config/opencode"
if [ -n "$VLAM_BASE_URL" ]; then
  node -e "
    const fs = require('fs');
    const config = JSON.parse(fs.readFileSync('/etc/opencode/opencode.json', 'utf8'));
    config.provider.vlam.options.baseURL = process.env.VLAM_BASE_URL;
    fs.writeFileSync(process.argv[1], JSON.stringify(config, null, 2));
  " "$HOME/.config/opencode/opencode.json"
else
  echo "WARNING: neither VLAM_BASE_URL nor VLAM_API_URL is set; using the demo API endpoint baked into opencode.json" >&2
  cp /etc/opencode/opencode.json "$HOME/.config/opencode/opencode.json"
fi
ln -sf /opt/opencode-plugins/node_modules "$HOME/.config/opencode/node_modules"
printf '{"dependencies":{"@ai-sdk/openai-compatible":"*"}}' \
  > "$HOME/.config/opencode/package.json"

# --- Claude auth ---
# The claude provider authenticates with a personal Claude subscription via
# CLAUDE_CODE_OAUTH_TOKEN (`claude setup-token`), read directly from env — no file
# is written. ANTHROPIC_API_KEY is intentionally NOT used: it is not forwarded to
# the claude subprocess (see LLM_ENV_ALLOWLIST), so it can never take precedence
# over the OAuth token, even if it is still set here.

exec regelrecht-enrich-worker
