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

# --- VLAM reachability check ---
# The ZAD `vlam` service gives the pod VLAM_API_URL, a proxy to SSC-ICT's VLAM
# API. Log which models the key can see there, so a deploy shows whether the
# key and the route work. Diagnostic only: it never stops the worker, and it
# never prints the key.
if [ -n "$VLAM_API_URL" ] && [ -n "$VLAM_API_KEY" ]; then
  node -e "
    const base = process.env.VLAM_API_URL.replace(/\/+\$/, '');
    const paths = ['/v1/models', '/models', '/v2.1/projects/poc/openai-compatible/v1/models'];
    (async () => {
      for (const p of paths) {
        try {
          const res = await fetch(base + p, {
            headers: { Authorization: 'Bearer ' + process.env.VLAM_API_KEY },
            signal: AbortSignal.timeout(10000),
          });
          const body = await res.text();
          let ids = null;
          try { ids = (JSON.parse(body).data || []).map((m) => m.id); } catch {}
          console.error('VLAM check ' + base + p + ': HTTP ' + res.status +
            (ids ? ', models: ' + JSON.stringify(ids) : ', body: ' + body.replace(/\s+/g, ' ').slice(0, 300)));
        } catch (e) {
          console.error('VLAM check ' + base + p + ': ' + e.name + ': ' + e.message);
        }
      }
    })();
  " || true
else
  echo "VLAM check skipped: VLAM_API_URL or VLAM_API_KEY not set" >&2
fi

# Set up opencode config in user-writable location.
# If VLAM_BASE_URL is set, override the demo URL baked into opencode.json.
mkdir -p "$HOME/.config/opencode"
if [ -n "$VLAM_BASE_URL" ]; then
  node -e "
    const fs = require('fs');
    const config = JSON.parse(fs.readFileSync('/etc/opencode/opencode.json', 'utf8'));
    config.provider.vlam.options.baseURL = process.env.VLAM_BASE_URL;
    fs.writeFileSync(process.argv[1], JSON.stringify(config, null, 2));
  " "$HOME/.config/opencode/opencode.json"
else
  echo "WARNING: VLAM_BASE_URL is not set — using demo API endpoint baked into opencode.json" >&2
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
