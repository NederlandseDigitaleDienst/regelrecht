/*
 * Running the scenario for real, in the visitor's browser.
 *
 * This is the only execution the panel has. There was a recorded one beside it
 * for a while, rendered server-side so the panel was never empty, and it turned
 * out to be one execution too many: two traces of the same law, with different
 * step counts because they asked the engine different questions, two sets of
 * wording that both had to stay true, and two buttons a visitor could not tell
 * apart. On a page arguing that one law should have one execution, that was the
 * wrong shape. So: the engine runs here or the panel says nothing ran.
 *
 * The engine and the laws are about 600 KB gzipped, fetched when the panel
 * before this one scrolls into view and run when this one does. Not on page
 * load: a visitor who never reaches the panel never pays for it.
 *
 * Nothing here draws anything. It produces rows, and the player animates them.
 */
import {
  parseFeature,
  dispatch,
  matchStep,
  typedArgs,
  traceRoot,
  ExecutionContext,
  // Copied into this project rather than imported from the workspace; see
  // script/landing-laws.sh for why. The copy is remade on every build (npm
  // `prebuild`), so what ships is always current with the shared package.
} from '~/lib/gherkin';
// The engine and the laws are loaded once per page and shared with the
// scenario runner on /concepts/scenarios; see ~/scripts/engine.ts.
import { prepare, corpusScenario } from './engine';
// Re-exported: the panel warms the engine through this module before it asks
// for a run, and the two belong to the same import for it.
export { prepare };

/** The output the panel puts on screen; the scenario asserts it too. */
const AMOUNT_OUTPUT = 'hoogte_zorgtoeslag';

/** One row of the trace, in the shape the server-rendered rows already use. */
export interface Beat {
  identifier: string;
  law: string | null;
  type: string;
  result: unknown;
  unit: string | null;
  provider: string | null;
  depth: number;
  tree: string;
}

export interface RunResult {
  beats: Beat[];
  amount: unknown;
  steps: number;
  durationMs: number;
}


/**
 * The box-drawing prefixes, one per row.
 *
 * The prefixes are not in the trace itself, and this is the only place that
 * draws them. Putting them in the trace would mean every consumer of an RFC-039
 * trace carries one renderer's drawing choices around; the engine's own
 * terminal view draws the same shapes from the same structure, separately.
 *
 * The distinction the shapes carry: double rules for a scope a cross-law call
 * opens, single rules for the arithmetic inside it. Plain indentation flattens
 * that away.
 */
function treePrefixes(root: any): string[] {
  const out: string[] = [];

  const walk = (node: any, cols: string[], isLast: boolean, isDouble: boolean, isRoot: boolean) => {
    if (!isRoot) {
      const connector = isDouble ? (isLast ? '╙──' : '╟──') : isLast ? '└──' : '├──';
      out.push(cols.join('') + connector);
    }

    // A cross-law call opens a scope of its own, drawn with the double rules;
    // everything else continues the scope it is in.
    //
    // The names are the serialised ones, snake_case, not the Rust variants:
    // testing for `CrossLawReference` here silently never matched, and the
    // whole tree came out in single lines.
    const opensScope = node.node_type === 'cross_law_reference' || node.node_type === 'article';

    const nextCols = isRoot ? cols : [...cols, isLast ? '    ' : isDouble ? '║   ' : '│   '];

    const children = node.children ?? [];
    children.forEach((child: any, i: number) => {
      walk(child, nextCols, i === children.length - 1, opensScope, false);
    });
  };

  // The root itself is not drawn; its children are the top-level rows, and the
  // recorder walks them from the root with `is_root` set, so that the root
  // contributes no column of its own.
  walk(root, [], false, false, true);
  return out;
}

/** Flatten the trace into rows, in the order the engine took them. */
function flatten(node: any, depth: number, out: Beat[]): void {
  const raw: string = node.name ?? '';
  const [before, output] = raw.includes('#') ? raw.split('#') : [null, raw];
  out.push({
    identifier: output,
    law: before,
    type: node.node_type,
    result: node.result,
    unit: node.type_spec?.unit ?? null,
    provider: node.source?.provider ?? null,
    depth,
    tree: '',
  });
  for (const child of node.children ?? []) flatten(child, depth + 1, out);
}

/**
 * Run the scenario and return its trace as rows.
 *
 * The scenario is the file that sits beside the law in the repository, not a
 * copy written for this page. The steps are dispatched through the shared
 * canonical-grammar runner, the same one the editor and the demo use, so "the
 * engine ran it" means the same thing here as it does there.
 */
export async function runScenario(base = '/'): Promise<RunResult> {
  // Together rather than one after the other: the scenario file is a separate
  // fetch now (only this panel needs it), and it should still land beside the
  // laws instead of after them.
  const [{ engine }, feature] = await Promise.all([prepare(base), corpusScenario(base)]);

  const parsed = parseFeature(feature);
  // The scenario the panel above shows: an income above the threshold, where
  // the allowance tapers. Matched by name so a reordering of the file cannot
  // silently run a different one.
  const wanted = parsed.scenarios.find((s: any) => s.name.includes('boven het drempelinkomen'))
    ?? parsed.scenarios[0];
  if (!wanted) throw new Error('scenario not found in the feature file');

  const ctx = new ExecutionContext();
  engine.clearDataSources();

  const t0 = performance.now();
  let trace: any = null;
  let amount: unknown = null;

  // `background` is the step list itself, not an object holding one.
  const all = [...(parsed.background ?? []), ...wanted.steps];
  for (const step of all) {
    const match = matchStep(step.text);
    if (!match) throw new Error(`unknown step: ${step.text}`);
    const { entry, args } = match;
    const typed = typedArgs(entry, args);
    const table = step.dataTable ?? null;

    if (entry.action === 'evaluate' || entry.action === 'evaluate_outputs') {
      const outputs =
        entry.action === 'evaluate_outputs'
          ? String(typed[0]).split(',').map((s) => s.trim())
          : [String(typed[0])];
      const result = engine.executeMultipleWithTrace(
        typed[1],
        outputs,
        ctx.parameters,
        ctx.calculationDate,
      );
      ctx.result = result;
      ctx.executed = true;
      trace = traceRoot(result.trace);
      // The scenario evaluates `heeft_recht_op_zorgtoeslag`, so the root of the
      // trace is a boolean. The panel shows the amount, which the same run
      // computes on the way: take it from the outputs, not from the root.
      amount = result.outputs?.[AMOUNT_OUTPUT] ?? trace?.result ?? null;
    } else {
      await dispatch(ctx, engine, entry.action, [...typed, ...entry.literals], table, {
        loadDependency: async () => {},
      });
    }
  }
  const durationMs = performance.now() - t0;

  if (!trace) throw new Error('the scenario never evaluated an output');

  const beats: Beat[] = [];
  for (const child of trace.children ?? []) flatten(child, 0, beats);
  const prefixes = treePrefixes(trace);
  beats.forEach((b, i) => {
    b.tree = prefixes[i] ?? '';
  });

  return { beats, amount, steps: beats.length, durationMs };
}
