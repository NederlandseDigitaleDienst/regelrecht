// Pint het gedrag van deploy-filters vast tegen de echte cargo-graaf van deze
// workspace.
//
// Waarom tegen de echte graaf en niet tegen een verzonnen fixture: de fout die
// dit script uitbant was een lijst die wegdreef van de werkelijkheid. Een
// fixture zou meedrijven. Deze test rekent dezelfde graaf na die de CI gebruikt,
// dus een crate die van eigenaar wisselt komt hier boven water.
//
// De `try`/`catch` in het script vangt alleen gegooide fouten. De gevallen
// hieronder dekken juist de stille kant: een verkeerde `kind === 'dev'`-check of
// een verschoven `dirOf` gooit niets, maar levert `false` op voor een component
// die had moeten uitrollen. Dat is precies de klasse fouten waarvoor het script
// bestaat.

import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const SCRIPT = fileURLToPath(new URL('./deploy-filters.mjs', import.meta.url));

import { COMPONENTS, componentsFor, prefixesFor, workspaceGraph } from './deploy-filters.mjs';

// Eén keer afleiden; `cargo metadata` is de duurste stap in dit bestand.
const graph = workspaceGraph();

const namesOf = (changed) => componentsFor(changed, graph);
const allComponents = Object.keys(COMPONENTS);

test('dirOf leidt het cratepad af, relatief en met slash', () => {
  assert.equal(graph.dirOf('regelrecht-auth'), 'packages/auth/');
  assert.equal(graph.dirOf('regelrecht-law-model'), 'packages/law-model/');
  assert.equal(graph.dirOf('regelrecht-editor-api'), 'packages/editor-api/');

  // Geen absoluut pad en geen achtergebleven Cargo.toml: daar zou `startsWith`
  // stilzwijgend nooit meer matchen.
  for (const name of graph.byName.keys()) {
    const dir = graph.dirOf(name);
    assert.ok(dir.startsWith('packages/'), `${name} gaf ${dir}`);
    assert.ok(dir.endsWith('/'), `${name} gaf ${dir}`);
    assert.ok(!dir.includes('Cargo.toml'), `${name} gaf ${dir}`);
  }
});

test('de closure van editor-api bevat de twee crates die eerder ontbraken', () => {
  const closure = graph.closure('regelrecht-editor-api');
  assert.ok(closure.has('regelrecht-auth'));
  assert.ok(closure.has('regelrecht-law-model'));

  // law-model zit er indirect in, via pipeline en corpus; shared hangt er weer
  // onder. Valt de recursie weg, dan blijft alleen de directe laag over.
  assert.ok(closure.has('regelrecht-shared'));
  assert.ok(closure.has('regelrecht-engine'));
});

test('een dev-dependency telt niet mee voor de closure', () => {
  // harvester heeft law-model uitsluitend als dev-dependency: die code zit niet
  // in de binary, dus een wijziging erin hoeft dit image niet te bouwen.
  const harvester = graph.closure('regelrecht-harvester');
  assert.ok(harvester.has('regelrecht-shared'));
  assert.ok(
    !harvester.has('regelrecht-law-model'),
    'law-model is een dev-dependency van harvester en hoort niet in de closure',
  );

  // corpus heeft engine alleen als dev-dependency.
  const corpus = graph.closure('regelrecht-corpus');
  assert.ok(corpus.has('regelrecht-law-model'));
  assert.ok(corpus.has('regelrecht-github'));
  assert.ok(
    !corpus.has('regelrecht-engine'),
    'engine is een dev-dependency van corpus en hoort niet in de closure',
  );
});

test('een wijziging in packages/auth laat de editor uitrollen', () => {
  // Het gat waarvoor dit script is geschreven. Zonder de graaf stond auth in
  // geen enkele filter en rolde er niets uit.
  const hit = namesOf(['packages/auth/src/oidc.rs']);
  assert.equal(hit.editor, true);
  assert.equal(hit.admin, true);

  // pipeline leunt niet op auth, dus de drie pipeline-images blijven staan.
  assert.equal(hit['pipeline-api'], false);
  assert.equal(hit['harvester-worker'], false);
  assert.equal(hit['enrich-worker'], false);
  assert.equal(hit.docs, false);
});

test('een wijziging in packages/law-model raakt alle crate-componenten', () => {
  const hit = namesOf(['packages/law-model/src/lib.rs']);
  assert.equal(hit.editor, true);
  assert.equal(hit.admin, true);
  assert.equal(hit['pipeline-api'], true);
  assert.equal(hit['harvester-worker'], true);
  assert.equal(hit['enrich-worker'], true);

  // docs bouwt sinds de landingspagina de engine als WASM in een eigen stage
  // (docs/Dockerfile), dus het hangt net als de demo aan de crate-graaf.
  assert.equal(hit.docs, true);

  // De componenten zonder Rust-image blijven er buiten.
  assert.equal(hit.grafana, false);
  assert.equal(hit.lawmaking, false);
});

test('een pad dat bij geen enkel component hoort levert niets op', () => {
  // tui en arch-extract zitten in geen enkele closure: het zijn crates zonder
  // image. Zou hier iets `true` worden, dan is de closure te ruim en zegt een
  // groene filter niets meer.
  for (const path of [
    'packages/tui/src/main.rs',
    'packages/arch-extract/src/lib.rs',
    'README.md',
    'REVIEW.md',
  ]) {
    const hit = namesOf([path]);
    const geraakt = allComponents.filter((name) => hit[name]);
    assert.deepEqual(geraakt, [], `${path} raakte ${geraakt.join(', ')}`);
  }
});

test('de handmatige paden buiten de graaf blijven werken', () => {
  assert.equal(namesOf(['docs/src/content/docs/index.md']).docs, true);
  assert.equal(namesOf(['docs/src/content/docs/index.md']).editor, false);

  assert.equal(namesOf(['packages/grafana/dashboards/pipeline.json']).grafana, true);
  assert.equal(namesOf(['packages/grafana/dashboards/pipeline.json']).editor, false);

  assert.equal(namesOf(['frontend-lawmaking/src/App.vue']).lawmaking, true);
  assert.equal(namesOf(['frontend-lawmaking/src/App.vue']).editor, false);

  assert.equal(namesOf(['frontend/src/main.ts']).editor, true);
  assert.equal(namesOf(['frontend/src/main.ts']).lawmaking, false);

  // frontend-shared zit in elk image dat een frontend bouwt. De twee
  // poc-images hoorden hier vanaf het begin bij en stonden er niet in: ze
  // kopiëren het pakket wel en draaien `npm ci` op dezelfde lock.
  const shared = namesOf(['packages/frontend-shared/src/auth.ts']);
  assert.equal(shared.editor, true);
  assert.equal(shared.admin, true);
  assert.equal(shared.lawmaking, true);
  assert.equal(shared.demo, true);
  assert.equal(shared.poc, true);
  assert.equal(shared['poc-napp'], true);
});

test('het poc-portaal volgt zijn register, zijn corpus en zijn frontends', () => {
  // Het register bepaalt wat het portaal serveert en welke wachtwoorden het
  // verwacht, dus het raakt beide images.
  const register = namesOf(['pocs/registry.yaml']);
  assert.equal(register.poc, true);
  assert.equal(register['poc-napp'], true);

  // Het portaal bakt heel corpus-poc/ in; napp alleen zijn eigen wetten. Een
  // wijziging in de napp-casus raakt daarom allebei.
  const nappCorpus = namesOf(['corpus-poc/napp/law/kieswet/1989-09-28.yaml']);
  assert.equal(nappCorpus.poc, true);
  assert.equal(nappCorpus['poc-napp'], true);

  // Een andere casus raakt het portaal wel en napp niet.
  const andereCasus = namesOf(['corpus-poc/terugbetaalregimes/data/personas.yaml']);
  assert.equal(andereCasus.poc, true);
  assert.equal(andereCasus['poc-napp'], false);

  // De frontend van napp zit alleen in het napp-image.
  const nappFrontend = namesOf(['frontend-poc-napp/src/main.js']);
  assert.equal(nappFrontend['poc-napp'], true);
  assert.equal(nappFrontend.poc, false);

  // En de statische pocs alleen in het portaal.
  assert.equal(namesOf(['frontend-poc-terugbetaalregimes/src/App.vue']).poc, true);
  assert.equal(namesOf(['frontend-poc-terugbetaalregimes/src/App.vue'])['poc-napp'], false);
});

test('vakantieverlof volgt zijn cel, zijn regelwerken en zijn frontend', () => {
  // De cel van de school en de regelwerken die zij draait, zitten alleen in
  // het vakantieverlof-image, niet in het portaal.
  for (const pad of [
    'corpus/cells/obs_de_klimop/reducties/reductie_verlof_al_verleend_in_schooljaar/2026-08-01.yaml',
    'corpus/regulation/nl/wet/leerplichtwet_1969/2026-08-01.yaml',
    'frontend-poc-vakantieverlof/src/App.vue',
  ]) {
    const geraakt = namesOf([pad]);
    assert.equal(geraakt['poc-vakantieverlof'], true, pad);
    assert.equal(geraakt.poc, false, pad);
  }

  // De crate van de cel volgt uit de graaf.
  assert.equal(namesOf(['packages/cel/src/cell.rs'])['poc-vakantieverlof'], true);

  // Een wet die de cel niet draait, raakt het niet.
  assert.equal(
    namesOf(['corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml'])['poc-vakantieverlof'],
    false,
  );
});

test('een engine-wijziging bouwt het poc-image, want dat bakt de engine als WASM', () => {
  // Het portaal zelf hangt niet aan de engine; het image bouwt hem wel, voor de
  // pocs in de browser. Met alleen de portaal-crate bleef het poc-image na een
  // engine-wijziging staan, terwijl napp en de demo wel meeschoven.
  const engine = namesOf(['packages/engine/src/engine.rs']);
  assert.equal(engine.poc, true);
  assert.equal(engine.demo, true);
  assert.equal(engine['poc-napp'], true);

  // En de portaal-crate telt nog steeds, naast de engine.
  assert.equal(namesOf(['packages/poc-portal/src/app.rs']).poc, true);
});

test('de beleidsassistent laat het poc-image bouwen', () => {
  // De assistent is JavaScript, geen crate, dus de cargo-graaf vindt hem
  // nooit — terwijl het portaal-image hem wel meekopieert. Zonder de
  // handmatige regel in COMPONENTS bouwt een wijziging hier geen image en
  // blijft productie stil op de oude assistent staan.
  const assistent = namesOf(['packages/poc-assistent/index.js']);
  assert.equal(assistent.poc, true);
  assert.equal(assistent.editor, false);
});

test('de gedeelde nginx-headers raken beide nginx-images', () => {
  // Het bestand ligt buiten docs/ en frontend-lawmaking/, terwijl beide images
  // het meekopiëren. Zonder deze regel scherpt iemand een header aan, wordt de
  // build groen, en rolt er niets uit — precies de stille overslag waarvoor
  // dit script bestaat.
  const hit = namesOf(['deploy/nginx/security-headers.conf']);
  assert.equal(hit.docs, true);
  assert.equal(hit.lawmaking, true);
  assert.equal(hit.editor, false);
  assert.equal(hit.grafana, false);
});

test('de skills raken alleen de enrich-worker', () => {
  // Drie componenten hangen aan dezelfde crate; alleen de enrich-worker neemt
  // de skills mee in zijn image.
  const hit = namesOf(['.claude/skills/law-generate/SKILL.md']);
  assert.equal(hit['enrich-worker'], true);
  assert.equal(hit['harvester-worker'], false);
  assert.equal(hit['pipeline-api'], false);
});

test('een workspace-brede wijziging raakt elk Rust-image', () => {
  for (const path of ['packages/Cargo.lock', 'packages/Cargo.toml', 'rust-toolchain.toml']) {
    const hit = namesOf([path]);
    assert.equal(hit.editor, true, path);
    assert.equal(hit.admin, true, path);
    assert.equal(hit['pipeline-api'], true, path);
    // docs compileert de engine (WASM-stage in docs/Dockerfile), dus een
    // workspace-brede wijziging raakt ook dat image.
    assert.equal(hit.docs, true, path);
  }
});

test('zonder lijst met gewijzigde bestanden bouwt alles', () => {
  // Fail-open: weten we niets, dan is een overbodige build goedkoper dan een
  // stille overslag.
  const hit = componentsFor([], null);
  for (const name of allComponents) {
    assert.equal(hit[name], true, name);
  }
});

test('elk component in de tabel levert bruikbare prefixen op', () => {
  // Vangt een tikfout in een cratenaam: die gooit, en de fallback zou dan bij
  // elke run alles bouwen zonder dat iemand het merkt.
  for (const [name, spec] of Object.entries(COMPONENTS)) {
    const prefixes = prefixesFor(spec, graph);
    assert.ok(prefixes.length > 0, `${name} leverde geen prefixen op`);
    for (const prefix of prefixes) {
      assert.equal(typeof prefix, 'string');
      assert.ok(prefix.length > 0, `${name} leverde een lege prefix op`);
    }
  }
});

// --- Het vangnet zelf ---
//
// De tests hierboven dekken de stille kant: een verkeerde check levert `false`
// op zonder te gooien. De twee hieronder dekken de luide kant, en die is het
// vangnet waar de rest op leunt. Gaat er iets stuk in de afleiding, dan moet
// alles gebouwd worden; blijft dat vangnet ongetest, dan kan het wegvallen
// zonder dat iemand het merkt, en is een storing weer een stille overslag.

test('een onbekende crate in de tabel gooit, zodat het vangnet aanslaat', () => {
  assert.throws(
    () => prefixesFor({ crate: 'regelrecht-bestaat-niet', paths: [] }, graph),
    /niet gevonden in de workspace/,
  );
});

test('faalt de afleiding, dan bouwt het script alles met een zichtbare waarschuwing', () => {
  // Het script als geheel, niet de losse functies: het vangnet zit in de
  // top-level try/catch. Zonder cargo op PATH faalt `cargo metadata`.
  const result = spawnSync(process.execPath, [SCRIPT, 'packages/auth/src/lib.rs'], {
    encoding: 'utf8',
    env: { ...process.env, PATH: '/nonexistent' },
  });

  assert.equal(result.status, 0, 'het script hoort niet te falen, maar alles te bouwen');
  assert.match(result.stdout, /::warning::/, 'de fallback hoort zichtbaar te zijn in het log');
  for (const name of allComponents) {
    assert.match(
      result.stdout,
      new RegExp(`^${name}=true$`, 'm'),
      `${name} hoort op true te staan als de afleiding faalt`,
    );
  }
});

test('een schemawijziging raakt elk Rust-image', () => {
  // Elk Rust-image doet `COPY schema/`, en corpus bakt er een schema uit in met
  // include_str!. Zonder deze regel levert een schemawijziging stille,
  // verouderde images op: de gevaarlijkste uitkomst, want niets wordt rood.
  const hit = namesOf(['schema/v0.5.6/schema.json']);
  for (const name of ['editor', 'admin', 'harvester-worker', 'enrich-worker', 'pipeline-api']) {
    assert.equal(hit[name], true, name);
  }
});

test('een Dockerfile raakt alleen de images die hem gebruiken', () => {
  // packages/pipeline/Dockerfile ligt in de cratemap van pipeline, en editor en
  // admin hangen via de graaf aan die crate. Ze gebruiken die Dockerfile niet,
  // dus ze horen niet mee te bouwen.
  const pipeline = namesOf(['packages/pipeline/Dockerfile']);
  assert.equal(pipeline['harvester-worker'], true);
  assert.equal(pipeline['enrich-worker'], true);
  assert.equal(pipeline['pipeline-api'], true);
  assert.equal(pipeline.editor, false);
  assert.equal(pipeline.admin, false);

  // En andersom blijft de crate zelf wel gewoon werken.
  const source = namesOf(['packages/pipeline/src/worker.rs']);
  assert.equal(source.editor, true);
  assert.equal(source.admin, true);
});
