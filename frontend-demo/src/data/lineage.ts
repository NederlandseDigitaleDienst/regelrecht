/**
 * Turn an execution trace into the "Gebruikte gegevens" tree: which values the
 * law used, where each came from (an organisation's register, or another law
 * that computed it), and which law owns the input so a correction can be
 * registered against the right law.
 *
 * Trace shape: `schema/trace/v1/trace-schema.json` (RFC-039). A step carries
 *   { node_type, name, result?, resolve_type?, source?, anchor?, children[] }
 * A `cross_law_reference` is named `<law>#<output>`; a `resolve` with
 * resolve_type DATA_SOURCE names its register in `source.provider` (the
 * organisation, or `correcties` for an approved claim).
 *
 * That provider used to be recovered by matching a regular expression against
 * the step's `message`, an English sentence written for a terminal. RFC-039
 * put it in a field; `message` is presentation and parsing it is a defect.
 */
import { CLAIMS_SOURCE } from '../engine/useDemoEngine';

/**
 * A trace step, as far as this module reads it. The engine's `TraceNode` is
 * assignable to it; `source` is the RFC-039 field that names the register.
 */
interface LineageTraceStep {
  node_type?: string;
  name?: string;
  result?: unknown;
  resolve_type?: string | null;
  source?: { provider?: string | null } | null;
  children?: LineageTraceStep[] | null;
}

/** The parameter a law was keyed on, and its value. */
interface LineageKey {
  keyField?: string;
  keyValue?: string;
}

export interface LineageNode extends LineageKey {
  kind: 'value' | 'law';
  /** owning law id */
  law: string;
  /** input name (value) or output name (law) */
  name: string;
  value: unknown;
  /** organisation for a value from a register */
  service?: string | null;
  /** value came from an approved claim */
  corrected?: boolean;
  children?: LineageNode[];
}

// The key parameters (bsn, kvk_nummer, ...) are strings in the law format, so
// their values are taken as strings here, as before.
function keyOf(children: LineageTraceStep[] | null | undefined): LineageKey {
  for (const key of ['bsn', 'kvk_nummer', 'organisatie_id', 'project_id']) {
    const hit = (children ?? []).find((c) => c.node_type === 'resolve' && c.name === key && c.resolve_type === 'PARAMETER');
    if (hit) return { keyField: key, keyValue: hit.result as string };
  }
  return {};
}

/**
 * @param trace       root trace node
 * @param rootLawId
 * @param rootParams  parameters of the top-level call
 */
export function lineageFromTrace(
  trace: LineageTraceStep | null | undefined,
  rootLawId: string,
  rootParams: Record<string, unknown>,
): LineageNode[] {
  const rootKey: LineageKey = rootParams.bsn ? { keyField: 'bsn', keyValue: rootParams.bsn as string } : rootParams.kvk_nummer ? { keyField: 'kvk_nummer', keyValue: rootParams.kvk_nummer as string } : {};
  return collect(trace, rootLawId, rootKey, new Set());
}

function collect(node: LineageTraceStep | null | undefined, lawId: string, key: LineageKey, seen: Set<string>): LineageNode[] {
  const out: LineageNode[] = [];
  for (const child of node?.children ?? []) {
    if (child.node_type === 'resolve' && child.resolve_type === 'DATA_SOURCE') {
      const dedupe = `${lawId}|${child.name}`;
      if (seen.has(dedupe)) continue;
      seen.add(dedupe);
      const source = child.source?.provider ?? null;
      out.push({
        kind: 'value',
        law: lawId,
        // A step without a name would be a malformed trace; kept as it comes.
        name: child.name as string,
        value: child.result ?? null,
        service: source === CLAIMS_SOURCE ? null : source,
        corrected: source === CLAIMS_SOURCE,
        ...key,
      });
    } else if (child.node_type === 'cross_law_reference') {
      // The engine names a cross-law step `<law>#<output>`. `split` always
      // yields a first element; a name without `#` would leave `output`
      // undefined, as before.
      const [refLaw, output] = String(child.name).split('#') as [string, string];
      const dedupe = `ref|${refLaw}|${output}`;
      if (seen.has(dedupe)) continue;
      seen.add(dedupe);
      const childKey = { ...key, ...keyOf(child.children) };
      out.push({
        kind: 'law',
        law: refLaw,
        name: output,
        value: child.result ?? null,
        children: collect(child, refLaw, childKey, seen),
        ...childKey,
      });
    } else if (
      child.node_type === 'article' ||
      child.node_type === 'action' ||
      child.node_type === 'operation' ||
      child.node_type === 'resolve'
    ) {
      // Descend: nested resolutions inside an action tree still count. An
      // input is resolved where an operation first reads it (RFC-043), so its
      // register value or cross-law call sits under that `resolve` step.
      out.push(...collect(child, lawId, key, seen));
    }
  }
  return out;
}

/** Flatten to the register values only (for counting and for a summary). */
export function leafValues(nodes: LineageNode[]): LineageNode[] {
  const out: LineageNode[] = [];
  for (const n of nodes) {
    if (n.kind === 'value') out.push(n);
    else out.push(...leafValues(n.children ?? []));
  }
  return out;
}
