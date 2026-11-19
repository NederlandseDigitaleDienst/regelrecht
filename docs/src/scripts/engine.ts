/*
 * Fetching the engine and the laws, once per page.
 *
 * Three panels on this site execute laws in the visitor's browser: the landing
 * page's run panel (~/scripts/landing-run.ts), the scenario runner on
 * /concepts/scenarios (~/scripts/scenario-playground.ts), and the receipts on
 * the plain-language paper page (~/scripts/paper-explained-run.ts). They ask the engine
 * different questions and draw different things, but they load exactly the same
 * WASM module and the same set of laws, so that lives here and the promise is
 * shared. A visitor who opens the runner after the landing panel has already
 * fetched the engine waits for nothing.
 *
 * What is loaded comes out of the corpus, assembled by script/landing-laws.sh
 * into public/laws at build time. All versions of each law are loaded, not one
 * picked here: the engine holds them side by side and selects on the
 * calculation date, which is what makes the result the same as the one CI
 * produces.
 */

export interface Manifest {
  calculationDate: string;
  scenario: string;
  laws: string[];
}

export interface Loaded {
  engine: any;
  manifest: Manifest;
  /**
   * Every law file as fetched, by its manifest path. Kept so a page can build
   * a second engine without a second download; see `engineFrom`.
   */
  laws: Map<string, string>;
  /** The engine constructor of the loaded WASM module. */
  WasmEngine: new () => any;
}

let enginePromise: Promise<Loaded> | null = null;
let featurePromise: Promise<string> | null = null;

/**
 * Load the engine and the laws, at most once.
 *
 * Separate from running so a page can start the download while the visitor is
 * still reading: by the time they press a button, the fetch has usually
 * finished.
 */
export function prepare(base = '/'): Promise<Loaded> {
  if (!enginePromise) {
    enginePromise = load(base).catch((err) => {
      // Never cache a failure: a flaky network should not leave a panel
      // permanently unable to run.
      enginePromise = null;
      throw err;
    });
  }
  return enginePromise;
}

async function load(base: string): Promise<Loaded> {
  const manifest: Manifest = await fetch(`${base}laws/manifest.json`).then((r) => {
    if (!r.ok) throw new Error(`manifest: ${r.status}`);
    return r.json();
  });

  const wasm = await import(/* @vite-ignore */ `${base}wasm/pkg/regelrecht_engine.js`);
  await wasm.default(`${base}wasm/pkg/regelrecht_engine_bg.wasm`);

  const engine = new wasm.WasmEngine();

  const texts = await Promise.all(
    manifest.laws.map((p) =>
      fetch(`${base}${p}`).then((r) => {
        if (!r.ok) throw new Error(`${p}: ${r.status}`);
        return r.text();
      }),
    ),
  );

  for (const yaml of texts) engine.loadLaw(yaml);

  const laws = new Map(manifest.laws.map((p, i) => [p, texts[i]]));
  return { engine, manifest, laws, WasmEngine: wasm.WasmEngine };
}

/**
 * A separate engine over the same laws, each passed through `edit` first.
 *
 * For a page that has to show what a different version of a law would have
 * computed, next to the shared engine that holds the corpus as published. The
 * shared engine is never touched: whatever `edit` returns lives only in the
 * engine this returns.
 */
export function engineFrom(
  loaded: Loaded,
  edit: (path: string, text: string) => string = (_p, t) => t,
): any {
  const engine = new loaded.WasmEngine();
  for (const [path, text] of loaded.laws) engine.loadLaw(edit(path, text));
  return engine;
}

/**
 * The scenario file from the corpus, for the caller that runs it as it stands.
 *
 * Separate from `prepare`, and fetched only on demand, because only one of the
 * two panels wants it. The landing page executes the corpus scenario itself; the
 * runner on /concepts/scenarios executes whatever is in its editor and ships its
 * own copy of the text in the page. Loading it inside `prepare` cost every
 * visitor to that page a 26 KB fetch whose result was dropped on the floor.
 *
 * Callers that want both should ask for them together, so the fetch still runs
 * beside the engine's rather than after it.
 */
export function corpusScenario(base = '/'): Promise<string> {
  if (!featurePromise) {
    featurePromise = prepare(base)
      .then(({ manifest }) =>
        fetch(`${base}laws/${manifest.scenario}`).then((r) => {
          if (!r.ok) throw new Error(`scenario: ${r.status}`);
          return r.text();
        }),
      )
      .catch((err) => {
        featurePromise = null;
        throw err;
      });
  }
  return featurePromise;
}
