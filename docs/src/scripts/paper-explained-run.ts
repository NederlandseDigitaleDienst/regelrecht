/*
 * The two decisions on /research/rules-as-executed/uitgelegd, computed in the
 * visitor's browser.
 *
 * Section 4.4 of the paper separates two checks. A digest shows which
 * specification an organization says it ran; only re-running the published
 * rule on the recorded inputs shows that this rule decided the case. The panel
 * makes both checks on two decisions about the same person:
 *
 *   A  decided with the published Wet op de zorgtoeslag, as in force;
 *   B  decided with a local copy that still carries last year's
 *      percentage_toetsingsinkomen, a version nobody published.
 *
 * Both are real runs of the same engine. B is the paper's honest-but-failing
 * administration (Section 4.4: "it can run a version it did not publish"), not
 * a forged document: its record carries the digest of what it actually ran.
 *
 * Nothing here draws anything.
 */
import {
  parseFeature,
  dispatch,
  matchStep,
  typedArgs,
  traceRoot,
  ExecutionContext,
} from '~/lib/gherkin/index.js';
import { prepare, corpusScenario, engineFrom } from './engine';

export { prepare };

const LAW_ID = 'wet_op_de_zorgtoeslag';
const OUTPUT = 'hoogte_zorgtoeslag';
/** Matched by name, as the landing panel does, so a reordering cannot swap it. */
const SCENARIO = 'boven het drempelinkomen';

/**
 * What the local copy changes. Last year's value of the share of income above
 * the threshold that is counted (13,67 % in the 2024 text, 13,7 % in 2025).
 * Kept as the literal YAML lines so that a reformatted law file makes this
 * throw rather than silently run an unchanged copy and report a match.
 */
const PUBLISHED_LINE = 'percentage_toetsingsinkomen:\n          value: 0.137\n';
const LOCAL_LINE = 'percentage_toetsingsinkomen:\n          value: 0.1367\n';

export interface Fact {
  provider: string;
  field: string;
  value: unknown;
  unit: string | null;
}

export interface Decision {
  /** The path of the law version the deciding system ran. */
  version: string;
  digest: string;
  amount: number;
}

export interface Receipts {
  calculationDate: string;
  /** The version in force on the calculation date, as published. */
  publishedVersion: string;
  publishedDigest: string;
  a: Decision;
  b: Decision;
  /** The recorded inputs: every value the run read from a data source. */
  facts: Fact[];
  change: { field: string; published: string; local: string };
}

async function sha256(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const hash = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(hash), (b) => b.toString(16).padStart(2, '0')).join('');
}

/** The newest version of the law that is not later than the calculation date. */
function versionInForce(paths: string[], date: string): string {
  const candidates = paths
    .filter((p) => p.includes(`/${LAW_ID}/`))
    .map((p) => ({ p, from: p.split('/').pop()!.replace(/\.yaml$/, '') }))
    .filter((c) => c.from <= date)
    .sort((x, y) => (x.from < y.from ? 1 : -1));
  if (!candidates.length) throw new Error(`no version of ${LAW_ID} in force on ${date}`);
  return candidates[0].p;
}

/**
 * Runs one at a time. A run clears the engine's data sources and registers
 * the scenario's over several awaited steps, so two runs on the shared engine
 * that overlap (two "check" buttons pressed in quick succession) would wipe
 * each other's inputs halfway and report a mismatch that is not there.
 */
let queue: Promise<unknown> = Promise.resolve();

function run(engine: any, feature: string): Promise<{ amount: number; root: any; date: string }> {
  const next = queue.then(() => runNow(engine, feature));
  queue = next.catch(() => {});
  return next;
}

async function runNow(engine: any, feature: string): Promise<{ amount: number; root: any; date: string }> {
  const parsed = parseFeature(feature);
  const wanted = parsed.scenarios.find((s: any) => s.name.includes(SCENARIO));
  if (!wanted) throw new Error('scenario not found in the feature file');

  const ctx = new ExecutionContext();
  engine.clearDataSources();
  for (const step of [...(parsed.background ?? []), ...wanted.steps]) {
    const match = matchStep(step.text);
    if (!match) throw new Error(`unknown step: ${step.text}`);
    const { entry, args } = match;
    const typed = typedArgs(entry, args);
    if (entry.action === 'evaluate' || entry.action === 'evaluate_outputs') {
      const result = engine.executeMultipleWithTrace(
        typed[1],
        [OUTPUT],
        ctx.parameters,
        ctx.calculationDate,
      );
      const amount = result.outputs?.[OUTPUT];
      if (typeof amount !== 'number') throw new Error(`${OUTPUT} was not computed`);
      return { amount, root: traceRoot(result.trace), date: ctx.calculationDate };
    }
    await dispatch(ctx, engine, entry.action, [...typed, ...entry.literals], step.dataTable ?? null, {
      loadDependency: async () => {},
    });
  }
  throw new Error('the scenario never evaluated an output');
}

/** Every value the run read from a data source, once each, in trace order. */
function factsOf(root: any): Fact[] {
  const seen = new Set<string>();
  const out: Fact[] = [];
  const walk = (node: any) => {
    const provider = node.source?.provider;
    if (provider && node.node_type === 'resolve') {
      const key = `${provider}:${node.name}`;
      if (!seen.has(key)) {
        seen.add(key);
        out.push({
          provider,
          field: node.name,
          value: node.result,
          unit: node.type_spec?.unit ?? null,
        });
      }
    }
    for (const child of node.children ?? []) walk(child);
  };
  walk(root);
  return out;
}

export async function runReceipts(base = '/'): Promise<Receipts> {
  const [loaded, feature] = await Promise.all([prepare(base), corpusScenario(base)]);

  const published = await run(loaded.engine, feature);
  const publishedVersion = versionInForce([...loaded.laws.keys()], published.date);
  const publishedText = loaded.laws.get(publishedVersion)!;
  if (!publishedText.includes(PUBLISHED_LINE)) {
    throw new Error(`${publishedVersion} no longer reads ${JSON.stringify(PUBLISHED_LINE)}`);
  }
  const localText = publishedText.replace(PUBLISHED_LINE, LOCAL_LINE);

  // B's system: the same engine and laws, except its copy of the law in force.
  const localEngine = engineFrom(loaded, (path, text) => (path === publishedVersion ? localText : text));
  let local: Awaited<ReturnType<typeof run>>;
  try {
    local = await run(localEngine, feature);
  } finally {
    // Used once; the WASM memory behind it is not reclaimed by the JS GC.
    localEngine.free?.();
  }
  if (local.amount === published.amount) {
    throw new Error('the local copy computes the same amount; the panel would show nothing');
  }

  const [publishedDigest, localDigest] = await Promise.all([sha256(publishedText), sha256(localText)]);

  return {
    calculationDate: published.date,
    publishedVersion,
    publishedDigest,
    a: { version: publishedVersion, digest: publishedDigest, amount: published.amount },
    b: { version: publishedVersion, digest: localDigest, amount: local.amount },
    facts: factsOf(published.root),
    change: { field: 'percentage_toetsingsinkomen', published: '0.137', local: '0.1367' },
  };
}

/**
 * The verifier's side: fetch the published version again, take its digest,
 * and re-run it on the recorded inputs. A separate run on purpose, made when
 * the visitor asks for it, rather than the amount A already computed.
 *
 * On this page the verifier uses the same engine that decided. Section 9.1 of
 * the paper is about what changes when it does not.
 */
export async function recompute(base = '/'): Promise<{ amount: number; digest: string }> {
  const [loaded, feature] = await Promise.all([prepare(base), corpusScenario(base)]);
  const { amount, date } = await run(loaded.engine, feature);
  const version = versionInForce([...loaded.laws.keys()], date);
  const text = await fetch(`${base}${version}`).then((r) => {
    if (!r.ok) throw new Error(`${version}: ${r.status}`);
    return r.text();
  });
  return { amount, digest: await sha256(text) };
}
