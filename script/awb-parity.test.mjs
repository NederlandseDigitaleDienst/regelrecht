/**
 * De Awb staat in twee corpora en moet daar hetzelfde zeggen.
 *
 * Het hoofdcorpus en het democorpus dragen allebei de beschikkingsprocedure en
 * de drie artikelen die eraan haken (3:46, 6:7, 6:8). Ze zijn met de hand
 * overgezet, en juist daar ging het twee keer mis: één keer viel de laatste
 * regel van artikel 6:8 weg (`weeks: $bezwaartermijn_weken`, nu `$termijn_weken`), waardoor de
 * einddatum van de bezwaartermijn gelijk werd aan de bekendmakingsdatum, en één
 * keer de omschrijving van de fase BEZWAAR.
 *
 * Geen van beide werd ergens gezien: het blijft geldige YAML, de schemacontrole
 * keurt het goed, en geen scenario raakt artikel 6:8. Deze test vergelijkt de
 * twee stukken tekst en niets anders, want dat is precies het soort fout dat
 * verder onzichtbaar blijft.
 *
 * Dat de demo een eigen kopie heeft, is geen mooie toestand — maar zolang die
 * er is, hoort hij te kloppen.
 */
import { strict as assert } from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const HOOFD = path.join(root, 'corpus/regulation/nl/wet/algemene_wet_bestuursrecht/1994-01-01.yaml');
const DEMO = path.join(
  root,
  'corpus/demo/regulation/nl/algemene_wet_bestuursrecht/artikel_1_1_bestuursorgaan/AWB-1994-01-01.yaml',
);

/**
 * De beschikkingsprocedure: het blok van `procedure:` tot aan de volgende
 * procedure of `articles:`. Het hoofdcorpus kent daarnaast de procedure voor
 * het beroep bij de bestuursrechter (hoofdstuk 8); de demo heeft die niet
 * nodig, dus alleen deze ene procedure moet gelijk zijn.
 */
function procedureBlock(text) {
  const from = text.indexOf('\nprocedure:');
  const to = text.indexOf('\narticles:', from);
  assert.ok(from !== -1, 'geen procedure-blok gevonden');
  assert.ok(to !== -1, 'geen articles-sleutel gevonden na de procedure');
  const next = text.indexOf('\n  - id:', text.indexOf('\n  - id:', from) + 1);
  return text.slice(from, next !== -1 && next < to ? next : to).trimEnd();
}

/** Eén artikel uit de wet, van zijn nummer tot het volgende artikel of het eind. */
function article(text, number) {
  const start = text.indexOf(`  - number: '${number}'`);
  assert.ok(start !== -1, `artikel ${number} niet gevonden`);
  const next = text.indexOf('\n  - number:', start + 1);
  return text.slice(start, next === -1 ? undefined : next).trimEnd();
}

/**
 * Voorbeeld van schema v0.9.0: het hoofdcorpus noemt soorten, de demo nog het
 * rechtskarakter. Vergeleken wordt wat er na die vertaling staat; omzetten van
 * de demo maakt deze stap overbodig.
 */
function withoutKinds(text) {
  return text
    .replace(/^ {4}# .*\n/gm, '')
    .replace(/^ {8}kind: .*\n/gm, '')
    .replace(/kind: algemene_wet_bestuursrecht\/beschikking/g, 'legal_character: BESCHIKKING')
    .replace(/kind: algemene_wet_bestuursrecht\/besluit/g, 'legal_character: BESCHIKKING');
}

const hoofd = withoutKinds(fs.readFileSync(HOOFD, 'utf8'));
const demo = fs.readFileSync(DEMO, 'utf8');

test('de beschikkingsprocedure is in beide corpora gelijk', () => {
  assert.equal(procedureBlock(demo), procedureBlock(hoofd));
});

for (const number of ['3:46', '6:7', '6:8']) {
  test(`artikel ${number} is in beide corpora gelijk`, () => {
    assert.equal(article(demo, number), article(hoofd, number));
  });
}

test('artikel 6:8 rekent de einddatum met een termijn en niet alleen met een datum', () => {
  // De fout die dit bestand aanleiding gaf, apart benoemd: zonder `weeks` geeft
  // DATE_ADD de bekendmakingsdatum terug, en dat is juridisch onzin.
  const zes_acht = article(hoofd, '6:8');
  assert.match(zes_acht, /termijn_einddatum[\s\S]*weeks: \$termijn_weken/);
});
