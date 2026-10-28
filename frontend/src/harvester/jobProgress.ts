/**
 * Rows for the "Progress" section of the job detail sheet, from a job's
 * free-form `progress` JSON. The enrich worker writes it every 10 seconds:
 * the agent's own note (`phase`, `step`, `total_steps`) at the top level and
 * what the worker observed under `worker` (see `progress_value` in
 * packages/pipeline/src/worker.rs).
 */

type Row = [label: string, value: string];

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function asText(value: unknown): string | null {
  return typeof value === 'string' && value !== '' ? value : null;
}

/** `45 s`, `12 min 5 s`, `1 h 3 min`. */
export function formatSeconds(total: number): string {
  const secs = Math.max(0, Math.round(total));
  if (secs < 60) return `${secs} s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} min ${secs % 60} s`;
  return `${Math.floor(mins / 60)} h ${mins % 60} min`;
}

/**
 * `running` is whether the job is still processing. The two time rows are
 * left out otherwise: they were true at the last write, and on a stopped job
 * "3 s ago" would read as now.
 */
export function progressRows(progress: unknown, running = true): Row[] {
  const p = asRecord(progress);
  if (!p) return [];
  const w = asRecord(p.worker) ?? {};
  const rows: Row[] = [];

  const phase = asText(p.phase);
  const step = asNumber(p.step);
  const totalSteps = asNumber(p.total_steps);
  if (phase) {
    const of = step !== null && totalSteps !== null ? ` (step ${step} of ${totalSteps})` : '';
    rows.push(['Phase', `${phase}${of}`]);
  }

  const call = asText(w.call);
  const calls = asNumber(w.calls);
  if (call) rows.push(['Agent call', calls && calls > 1 ? `${call} (call ${calls})` : call]);

  if (running) {
    const elapsed = asNumber(w.call_elapsed_secs);
    if (elapsed !== null) rows.push(['Call running for', formatSeconds(elapsed)]);
    const idle = asNumber(w.idle_secs);
    if (idle !== null) rows.push(['Last output', `${formatSeconds(idle)} ago`]);
  }

  const articles = asNumber(w.articles);
  const done = asNumber(w.articles_with_machine_readable);
  if (articles !== null && done !== null) {
    rows.push(['Articles with machine_readable', `${done} / ${articles}`]);
  }

  const turns = asNumber(w.turns);
  if (turns !== null) rows.push(['Turns', String(turns)]);
  const toolCalls = asNumber(w.tool_calls);
  if (toolCalls !== null) rows.push(['Tool calls', String(toolCalls)]);

  const lastTool = asText(w.last_tool);
  if (lastTool) rows.push(['Last tool', lastTool]);

  return rows;
}

/** The latest tool calls, oldest first, for the code block under the rows. */
export function recentTools(progress: unknown): string[] {
  const w = asRecord(asRecord(progress)?.worker);
  const recent = w?.recent_tools;
  return Array.isArray(recent) ? recent.filter((t): t is string => typeof t === 'string') : [];
}
