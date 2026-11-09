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

# --- opencode setup ---
# The worker writes opencode's auth.json (from VLAM_API_KEY), its user config
# with the endpoint (from VLAM_API_URL) and picks the model to record, at
# startup: see `prepare_worker` in packages/pipeline/src/agent/opencode.rs.
# What stays here is the CPU-dependent binary choice and the VLAM check below.

# --- VLAM endpoint, for the check below ---
# The only route to VLAM is the ZAD `vlam` service: it gives the pod
# VLAM_API_URL, a proxy to SSC-ICT's VLAM API that speaks the OpenAI API under
# /v1. The worker points opencode at the same address.
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
    const config = JSON.parse(require('fs').readFileSync('/opt/opencode/opencode.json', 'utf8'));
    const model = config.model.replace(/^vlam\//, '');
    // The output limit opencode sends as max_tokens on every call. Asking for
    // it here shows on each deploy whether VLAM accepts it; the answer itself
    // stays short.
    // Falls back to a small value, with a note, when the config does not name
    // the model's limit, so the other checks still run.
    const configured = config.provider?.vlam?.models?.[model]?.limit?.output;
    const maxOutput = configured ?? 20;
    if (configured === undefined) console.error('VLAM check: no limit.output for ' + model + ' in opencode.json');
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
      // One short question, asked as plain as can be and then with each
      // reasoning setting that might apply. VLAM reports no reasoning tokens,
      // yet answering OK takes some sixty completion tokens: the reasoning is
      // in there, and it counts against max_tokens. These lines show whether
      // it comes back as a field of its own and whether a setting shrinks it.
      // Of a separate reasoning field only its length is printed. A server
      // that ignores a setting answers as if it were absent, so equal tokens
      // on two lines do not prove the setting was rejected.
      const ask = async (label, extra) => {
        try {
          const res = await fetch(base + '/chat/completions', {
            method: 'POST',
            headers,
            body: JSON.stringify({ model, max_tokens: maxOutput, messages: [{ role: 'user', content: 'Antwoord met alleen: OK' }], ...extra }),
            signal: AbortSignal.timeout(60000),
          });
          const body = await res.text();
          let answer = null;
          let usage = '';
          try {
            const parsed = JSON.parse(body);
            const message = parsed.choices[0].message;
            answer = message.content;
            const u = parsed.usage || {};
            const reasoning = u.completion_tokens_details && u.completion_tokens_details.reasoning_tokens;
            const separate = message.reasoning_content ?? message.reasoning;
            usage = ', finish_reason=' + parsed.choices[0].finish_reason +
              ', completion_tokens=' + u.completion_tokens +
              (reasoning !== undefined ? ', reasoning_tokens=' + reasoning : '') +
              (typeof separate === 'string' ? ', separate_reasoning_chars=' + separate.length : ', no separate reasoning field');
          } catch {}
          console.error('VLAM check ' + label + ' (max_tokens ' + maxOutput + '): HTTP ' + res.status + usage + ', ' +
            (answer !== null ? 'answer: ' + flat(String(answer)) : 'body: ' + flat(body)));
        } catch (e) {
          console.error('VLAM check ' + label + ': ' + e.name + ': ' + e.message);
        }
      };
      await ask('chat', {});
      await ask('chat reasoning_effort=low', { reasoning_effort: 'low' });
      await ask('chat thinking=false', { chat_template_kwargs: { thinking: false } });
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
