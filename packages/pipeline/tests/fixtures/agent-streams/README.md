# Agent stream fixtures

What a provider CLI writes on stdout during one enrichment call, one JSON event
per line. `src/agent_stream_fixtures.rs` plays each file through the worker's
supervisor with a fake CLI (`sed '/^#/d' <file>`) and pins what the run
reports: usage, model, error text and the activity summary.

Lines starting with `#` are a header and never reach the code under test.

| File | Source | What it pins |
| --- | --- | --- |
| `claude-success.jsonl` | Captured | Model from `init`, usage from the closing `result`, turns and tool calls |
| `claude-error.jsonl` | Captured | Exit 1 with an empty stderr: the reason comes from the `result` event (`920e1821a`) |
| `opencode-success.jsonl` | **Assembled** | Usage as the sum of `step_finish` events, model from configuration (#1894) |
| `opencode-cut-off.jsonl` | **Assembled** | A step ended by the output limit shows in the summary (#1890) |
| `opencode-error.jsonl` | **Assembled** | An `error` event and exit 1, whose reason does *not* reach the job's error message (unlike claude's) |

## Assembled files are a stopgap

No recorded opencode/VLAM transcript existed when these were written. The
assembled files reuse the event shapes from the hand-written samples in
`agent_activity.rs` and `enrich.rs`, so they are only as faithful as those
samples. Replacing them with real transcripts is tracked in
[#1895](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/1895).
A real transcript changes the expected numbers in the tests, not their shape.

## Capturing a claude transcript

Run the CLI with the flags the worker passes, in an empty directory holding a
fictional `law.yaml`:

```sh
claude -p "Read law.yaml with the Read tool, then reply with only its first word." \
  --allowedTools Read --disallowedTools Bash \
  --output-format stream-json --verbose --include-partial-messages \
  --model claude-haiku-5-5 > claude-success.raw.jsonl
```

Then scrub it before it goes anywhere near the repository. This is a public
repository, and a raw transcript carries the machine that made it.

- Replace paths with `/work`. Streaming splits a tool's input over many
  `input_json_delta` events, so join each block's `partial_json` before
  replacing, or a path survives in pieces that no per-line search finds.
- Replace `session_id`, `uuid`, message, request and tool-use ids with
  placeholders, in keys as well as values (`wire_tool_inputs` is keyed by
  tool-use id). Keep distinct ids distinct: turns are counted per message id.
- Empty the account's lists in the `init` event (`skills`, `plugins`,
  `mcp_servers`, `slash_commands`, `agents`), keep `tools` to the file tools,
  and drop `memory_paths`, `messaging_socket_path` and `original_cwd`.
- Reduce `rate_limit_event` to its status, replace thinking signatures and
  `credential_expires_at`.
- Keep every event, its key order and the compact formatting. The worker
  matches substrings such as `"type":"result"`, so reformatting the JSON
  changes what is tested.

Finish with a search of every string and key, and of the joined stream
deltas, for `/home/`, `/tmp/`, `@`, token prefixes, your username and the
capture directory's name.
