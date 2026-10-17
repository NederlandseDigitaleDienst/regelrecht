// Bepaal welke deploybare componenten door een wijziging geraakt worden.
//
// Waarom niet met de hand: `deploy.yml` somde per component de paden op die
// hem raken, en die lijst dreef weg. `packages/auth` en `packages/law-model`
// stonden er in geen enkele filter, terwijl editor-api en admin op auth leunen
// en engine, corpus, pipeline en harvester op law-model. Een merge die alleen
// zo'n crate raakte bouwde dus geen image en rolde niets uit, zonder dat er
// iets rood werd. Dit script leidt de Rust-kant af uit de dependency-graaf van
// cargo, zodat een nieuwe crate vanzelf op de goede plek terechtkomt.
//
// De niet-Rust-kant blijft met de hand: frontends, Dockerfiles, het corpus en
// de skills die de enrich-worker meeneemt. Die verzameling is klein en
// verandert zelden.
//
// Faalt hier iets — cargo ontbreekt, de graaf is onleesbaar, een pad valt
// buiten alle regels — dan zet het script alles op `true`. De duurste uitkomst
// is dan een overbodige build; de goedkope uitkomst zou een stille overslag
// zijn, en dat is precies de fout die dit script moet uitbannen.

import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// De nginx-images kopiëren dit bestand alle twee mee. Het valt buiten hun
// eigen map, dus zonder deze regel levert een aangescherpte header een groene
// build op die niets uitrolt.
const NGINX_SHARED = 'deploy/nginx/';

// Per component: de crate waar het image aan hangt (of null, of een lijst als
// het image er meer dan één bouwt), plus de paden buiten de graaf die het image
// beïnvloeden.
export const COMPONENTS = {
  editor: {
    crate: 'regelrecht-editor-api',
    paths: [
      'frontend/',
      'packages/frontend-shared/',
      'corpus-registry.yaml',
      'corpus/regulation/',
    ],
  },
  admin: {
    crate: 'regelrecht-admin',
    paths: ['packages/frontend-shared/', 'packages/admin/Dockerfile'],
  },
  'harvester-worker': {
    crate: 'regelrecht-pipeline',
    paths: ['packages/pipeline/Dockerfile'],
  },
  'enrich-worker': {
    crate: 'regelrecht-pipeline',
    paths: ['packages/pipeline/Dockerfile', '.claude/skills/law-'],
  },
  'pipeline-api': {
    crate: 'regelrecht-pipeline',
    paths: ['packages/pipeline/Dockerfile'],
  },
  grafana: { crate: null, paths: ['packages/grafana/'] },
  lawmaking: {
    crate: null,
    paths: ['frontend-lawmaking/', 'packages/frontend-shared/', NGINX_SHARED],
  },
  // De landingspagina bakt het corpus in (de wettekst en de YAML die ze naast
  // elkaar zet, de annotatie eronder) en draait het zorgtoeslag-scenario als
  // WASM in de browser van de bezoeker. Daarmee hangt docs aan dezelfde dingen
  // als de demo: de engine-crate, het gedeelde frontend-pakket waar de
  // Gherkin-runner uit komt, en de wetten die script/landing-laws.sh meeneemt.
  // Zonder die paden bleef productie na een corpuswijziging de oude wettekst
  // tonen, precies de drift die landing-demo.ts zegt te voorkomen.
  docs: {
    crate: 'regelrecht-engine',
    paths: [
      'docs/',
      'packages/frontend-shared/',
      'corpus/regulation/',
      'corpus/annotations/',
      'script/landing-laws.sh',
      NGINX_SHARED,
    ],
  },
  // De demo bouwt de engine als WASM (zie frontend-demo/Dockerfile), dus hij
  // hangt aan de engine-crate; daarnaast aan zijn eigen map, het gedeelde
  // frontend-pakket en het demo-corpus dat hij bundelt. `build-demo` in
  // deploy.yml leest deze uitkomst.
  demo: {
    crate: 'regelrecht-engine',
    paths: ['frontend-demo/', 'packages/frontend-shared/', 'corpus/demo/', NGINX_SHARED],
  },
  // napp draait als eigen, niet-gepubliceerd component; het portaal proxyt
  // ernaartoe. Eigen image, dus eigen filter.
  'poc-napp': {
    crate: 'regelrecht-poc-napp',
    paths: [
      'pocs/',
      'corpus-poc/napp/',
      'frontend-poc-napp/',
      'packages/frontend-shared/',
      'packages/poc-napp/Dockerfile',
    ],
  },
  // vakantieverlof draait net als napp als eigen, niet-gepubliceerd component.
  // De crate-kant (cel, engine) volgt uit de graaf; het image neemt daarnaast
  // de regelwerken mee die de cel draait, en de cel van de school.
  'poc-vakantieverlof': {
    crate: 'regelrecht-poc-vakantieverlof',
    paths: [
      'pocs/',
      'corpus/regulation/nl/wet/leerplichtwet_1969/',
      'corpus/regulation/nl/wet/algemene_wet_bestuursrecht/',
      'corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaar_obs_de_klimop/',
      'corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaargrenzen_obs_de_klimop/',
      'corpus/cells/obs_de_klimop/',
      'frontend-poc-vakantieverlof/',
      'packages/frontend-shared/',
      'packages/poc-vakantieverlof/Dockerfile',
    ],
  },
  // Het poc-portaal bakt de statische pocs in zijn eigen image, dus het raakt
  // ook aan hun frontends en hun casus-corpus. De crate-kant volgt uit de graaf,
  // over twee crates: de portaal-binary, en de engine die het image als WASM
  // voor de pocs bouwt (packages/poc-portal/Dockerfile, de wasm-builder). Het
  // portaal zelf hangt niet aan de engine, dus met alleen de eerste bleef
  // productie na een engine-wijziging stil op de oude WASM draaien.
  //
  // `packages/poc-assistent/` staat er met de hand bij en moet er blijven: het
  // is JavaScript, dus geen crate, en de graaf vindt het nooit. Het image
  // kopieert het wel (poc-portal/Dockerfile). Zonder deze regel bouwt een
  // wijziging aan de assistent geen image en blijft productie stil op de oude
  // staan — precies de fout die dit script hoort uit te bannen.
  poc: {
    crate: ['regelrecht-poc-portal', 'regelrecht-engine'],
    paths: [
      'pocs/',
      'corpus-poc/',
      'frontend-poc-portal/',
      'frontend-poc-terugbetaalregimes/',
      'frontend-poc-nieuwkomersbekostiging/',
      'packages/frontend-shared/',
      'packages/poc-assistent/',
      'packages/poc-portal/Dockerfile',
    ],
  },
};

// Raakt elk component met een Rust-image: de workspace zelf.
const RUST_WIDE = [
  'packages/Cargo.lock',
  'packages/Cargo.toml',
  'packages/.cargo/',
  'rust-toolchain.toml',
  // Elk Rust-image doet `COPY schema/ /schema/`, en corpus bakt er een schema
  // uit in met `include_str!`. Zonder deze regel levert een schemawijziging
  // stille, verouderde images op.
  'schema/',
];

export function workspaceGraph() {
  const raw = execFileSync(
    'cargo',
    ['metadata', '--no-deps', '--format-version', '1'],
    { cwd: 'packages', maxBuffer: 64 * 1024 * 1024, encoding: 'utf8' },
  );
  const meta = JSON.parse(raw);
  const byName = new Map(meta.packages.map((p) => [p.name, p]));

  // Alleen `normal` en `build`: een dev-dependency zit niet in de binary, dus
  // een wijziging daarin hoeft geen image te bouwen.
  const closure = (name, seen = new Set()) => {
    for (const dep of byName.get(name)?.dependencies ?? []) {
      const kind = dep.kind ?? 'normal';
      if (!byName.has(dep.name) || seen.has(dep.name) || kind === 'dev') continue;
      seen.add(dep.name);
      closure(dep.name, seen);
    }
    return seen;
  };

  const dirOf = (name) => {
    const manifest = byName.get(name)?.manifest_path ?? '';
    const idx = manifest.lastIndexOf('/packages/');
    return idx === -1 ? null : manifest.slice(idx + 1).replace(/Cargo\.toml$/, '');
  };

  return { byName, closure, dirOf };
}

// Eén implementatie van "hoe rapporteren we een component-uitkomst", zodat de
// fallback hieronder hem kan hergebruiken in plaats van hem na te bouwen.
function emit(name, value) {
  console.log(`${name}=${value}`);
  const out = process.env.GITHUB_OUTPUT;
  if (out) appendFileSync(out, `${name}=${value}\n`);
}

// De padprefixen die dit component raken: de handmatige lijst, en voor een
// component met een image de crate zelf plus zijn hele normal/build-closure.
export function prefixesFor(spec, graph) {
  const prefixes = [...spec.paths];
  if (!spec.crate) return prefixes;

  prefixes.push(...RUST_WIDE);
  for (const crate of [spec.crate].flat()) {
    if (!graph.byName.has(crate)) {
      throw new Error(`crate ${crate} niet gevonden in de workspace`);
    }
    for (const dep of [crate, ...graph.closure(crate)]) {
      const dir = graph.dirOf(dep);
      if (!dir) throw new Error(`geen pad voor crate ${dep}`);
      prefixes.push(dir);
    }
  }
  return prefixes;
}

// Welke componenten raakt deze lijst gewijzigde bestanden? Een lege lijst
// betekent "we weten het niet" en dus alles bouwen, dezelfde kant op als de
// fallback: een overbodige build is goedkoper dan een stille overslag.
// Een Dockerfile in een cratemap hoort niet bij de crate maar bij het image dat
// hem gebruikt. Zonder dit onderscheid zet een wijziging in
// packages/pipeline/Dockerfile ook editor en admin op true, want die hangen via
// de graaf aan pipeline terwijl ze die Dockerfile niet gebruiken: twee builds
// van een kwartier voor niets. Welke component welke Dockerfile gebruikt staat
// in `paths`, en dat blijft handwerk omdat het niet uit de graaf volgt.
const isDockerfile = (file) => /(^|\/)Dockerfile[^/]*$/.test(file);

export function componentsFor(changed, graph) {
  const result = {};
  const unknown = changed.length === 0;
  for (const [name, spec] of Object.entries(COMPONENTS)) {
    if (unknown) {
      result[name] = true;
      continue;
    }
    const explicit = spec.paths;
    const derived = prefixesFor(spec, graph).filter((p) => !explicit.includes(p));
    result[name] = changed.some(
      (file) =>
        explicit.some((p) => file.startsWith(p)) ||
        (!isDockerfile(file) && derived.some((p) => file.startsWith(p))),
    );
  }
  return result;
}

function main() {
  // Uit een bestand als het er is: een grote PR heeft meer bestandsnamen dan
  // er in een commandoregel passen.
  const args = process.argv.slice(2);
  const changed =
    args[0] === '--from-file'
      ? readFileSync(args[1], 'utf8').split('\n').filter(Boolean)
      : args.filter(Boolean);

  if (changed.length === 0) {
    console.log('Geen lijst met gewijzigde bestanden: alles bouwen.');
    for (const [name, hit] of Object.entries(componentsFor(changed, null))) {
      emit(name, hit);
    }
    return;
  }

  const graph = workspaceGraph();
  for (const [name, hit] of Object.entries(componentsFor(changed, graph))) {
    emit(name, hit);
  }
}

// Alleen draaien als het script zelf is aangeroepen; bij `import` vanuit de
// test moet er niets gebeuren.
const invokedDirectly =
  process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));

if (invokedDirectly) {
  try {
    main();
  } catch (error) {
    console.log(`::warning::deploy-filters faalde (${error.message}); alles bouwen.`);
    for (const name of Object.keys(COMPONENTS)) emit(name, true);
  }
}
