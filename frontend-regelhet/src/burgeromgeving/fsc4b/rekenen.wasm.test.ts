// @vitest-environment node
// The parking card of Wilgendaal on the real kern, built as WebAssembly
// (`just burgerapp-wasm`), on the source the RegelHet server runs
// (packages/regelhet/bron.yaml): replay a dossier with an earlier card, and
// predict a new application on the GGD's advice. Without the WASM build this
// file is skipped, except in CI (the job "Burgerapp on the real kern").
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { Gegevens, Record_ } from './gegevens';
import { Rekenkern } from './rekenen';
import type { MaakPoc, PocWasm } from './rekenen';

const app = fileURLToPath(new URL('../../../', import.meta.url));
const repo = join(app, '..');
const pkg = process.env.BURGERAPP_WASM_PKG ?? join(app, 'burgerapp', 'wasm', 'pkg');
const gebouwd = existsSync(join(pkg, 'regelrecht_poc_chronolexografie_bg.wasm'));

/**
 * The files of the source the manifest names, by their path in the source, as
 * `GET /fsc4b/regelwerken` gives them: each directory of `directories` under
 * its name on the left, and the UI spec (packages/corpus/src/source_manifest.rs).
 */
function bron(): Record<string, string> {
  const manifest = readFileSync(join(repo, 'packages/regelhet/bron.yaml'), 'utf8');
  const uit: Record<string, string> = {};
  const lees = (dir: string, onder: string, als: string) => {
    for (const naam of readdirSync(join(dir, onder))) {
      const rel = onder ? `${onder}/${naam}` : naam;
      if (statSync(join(dir, rel)).isDirectory()) lees(dir, rel, als);
      else if (/\.ya?ml$/.test(naam)) uit[`${als}/${rel}`] = readFileSync(join(dir, rel), 'utf8');
    }
  };
  let mappen = false;
  for (const regel of manifest.split('\n')) {
    const spec = /^ui_spec:\s*(\S+)/.exec(regel);
    if (spec?.[1]) uit['ui_spec.yaml'] = readFileSync(join(repo, spec[1]), 'utf8');
    if (/^directories:/.test(regel)) mappen = true;
    else if (/^\S/.test(regel)) mappen = false;
    const m = mappen ? /^\s+([^:#\s]+):\s*(\S+)/.exec(regel) : null;
    if (m?.[1] && m[2]) lees(join(repo, m[2]), '', m[1]);
  }
  return uit;
}

async function maak(): Promise<MaakPoc> {
  const glue = (await import(/* @vite-ignore */ join(pkg, 'regelrecht_poc_chronolexografie.js'))) as {
    default: (o: { module_or_path: Uint8Array }) => Promise<unknown>;
    Poc: new (b: string, s: string) => PocWasm;
  };
  await glue.default({ module_or_path: readFileSync(join(pkg, 'regelrecht_poc_chronolexografie_bg.wasm')) });
  return (b, s) => new glue.Poc(b, s);
}

const CASUS = 'gehandicaptenparkeerkaart';
const ARTIKEL = 'besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49';
const GGD_ADVIES = 'beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht';
const BSN = '999990032';
const aanvraag = (dag: string) => ({
  bsn_aanvrager: BSN,
  kaart_als_bestuurder: true,
  kaart_als_passagier: false,
  aanvraag_ondertekend: true,
  naam_aanvrager: 'Test Burger',
  adres_aanvrager: 'Voorbeeldlaan 1, 1234 AB Wilgendaal',
  dagtekening: dag,
  gevraagde_beschikking: 'gehandicaptenparkeerkaart',
});
const bijAanvraag = [{ feit: GGD_ADVIES, taak: 'Geneeskundig advies uitbrengen' }];

/**
 * The records of a dossier with a card granted in March, as the executor's
 * cells recorded them, and what the executor's cell hands out on 13 October.
 */
function eerdereKaart(m: MaakPoc, bestanden: Record<string, string>): { records: Record_[]; uitgegeven: Pick<Gegevens, 'antwoorden' | 'regelwerken'> } {
  const p = m('{}', JSON.stringify([{ naam: 'regelhet', bestanden }]));
  p.kies('gehandicaptenparkeerkaart');
  const a = JSON.parse(p.indienen(ARTIKEL, JSON.stringify(aanvraag('2026-03-02')), '2026-03-02')) as { id: string };
  const advies = { bsn_aanvrager: BSN, datum_onderzoek: '2026-03-05', langdurige_loopbeperking: true, continu_afhankelijk_van_hulp_bestuurder: false, permanent_rolstoelgebonden: false, ernstige_beperkingen_andere_dan_loopbeperkingen: false };
  p.vastleggen('ggd_wilgendaal', GGD_ADVIES, JSON.stringify(advies), '2026-03-05');
  (p as PocWasm & { beslissen(op: string, b: string, d: string): string }).beslissen(a.id, '{}', '2026-03-09');
  const k = JSON.parse((p as PocWasm & { kroniek(): string }).kroniek()) as { grammen: Record_[]; andere: { grammen: Record_[] }[] };
  const uitgegeven = JSON.parse(p.brongegevens('bsn_aanvrager', BSN, '2026-10-13')) as Pick<Gegevens, 'antwoorden' | 'regelwerken'>;
  return { records: [...k.grammen, ...k.andere.flatMap((x) => x.grammen)], uitgegeven };
}

// CI builds the kern first and sets BURGERAPP_WASM_VEREIST, so there a
// missing build fails instead of skipping.
it.runIf(process.env.BURGERAPP_WASM_VEREIST === '1' && !gebouwd)('has the WASM build of the kern', () => {
  throw new Error(`no WASM build in ${pkg}: run \`just burgerapp-wasm\` first`);
});

// Each test loads the kern and makes up to thirty-two trial decisions, which
// takes seconds on a busy runner.
describe.skipIf(!gebouwd)('the parking card on the real kern', { timeout: 120_000 }, () => {
  it('predicts a first application conditionally on the GGD’s advice', async () => {
    const kern = Rekenkern.laad(await maak(), bron(), CASUS);
    kern.naspelen([]);
    const v = kern.voorspel({ artikel: ARTIKEL, velden: aanvraag('2026-10-13'), datum: '2026-10-13', bijAanvraag, inlogveld: 'bsn_aanvrager', bsn: BSN });
    expect(v.zeker).toBe(false);
    const per = Object.fromEntries(v.takken.map((t) => [t.vastgesteld.join(',') || 'geen', t.uitkomst]));
    expect(per['langdurige loopbeperking']).toBe('toegekend');
    expect(per.geen).toBe('afgewezen');
    expect(v.takken.every((t) => t.celNaam === 'GGD Wilgendaal')).toBe(true);
  });

  it('replays a dossier with an earlier card into the chronicles, decisions included, and predicts on it', async () => {
    const m = await maak();
    const bestanden = bron();
    const { records, uitgegeven } = eerdereKaart(m, bestanden);
    expect(records.some((r) => r.kind === 'BESCHIKKING')).toBe(true);
    const kern = Rekenkern.laad(m, bestanden, CASUS);
    kern.naspelen(records);
    const vergeleken = kern.vergelijk(
      { ...uitgegeven, iss: '', sub: BSN, iat: 0, bundel_hash: '', peildatum: '2026-10-13', casus: kern.casus.id, cel: kern.casus.cel, records, bij_aanvraag: bijAanvraag },
      'bsn_aanvrager',
    );
    expect(vergeleken.regelwerkenGelijk).toBe(true);
    expect(vergeleken.antwoorden.length).toBeGreaterThan(0);
    expect(vergeleken.antwoorden.every((v) => v.gelijk)).toBe(true);
    expect(vergeleken.antwoorden.find((v) => v.naam === 'aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt')?.lokaal).toBe(true);
    const invoer = { artikel: ARTIKEL, velden: aanvraag('2026-10-13'), datum: '2026-10-13', bijAanvraag, inlogveld: 'bsn_aanvrager', bsn: BSN };
    const voor = (kern as unknown as { poc: PocWasm }).poc.bewaren();
    const v = kern.voorspel(invoer);
    // The card of March is valid until 2031: a new one is refused, whatever the GGD finds.
    expect(v.zeker).toBe(true);
    expect(v.takken.map((t) => t.uitkomst)).toEqual(['afgewezen']);
    // Nothing of the trial stays in the real kern, and a second prediction is the same.
    expect((kern as unknown as { poc: PocWasm }).poc.bewaren()).toBe(voor);
    expect(kern.voorspel(invoer)).toEqual(v);
  });

  it('finds a passenger’s card, which needs two findings of the GGD together', async () => {
    const kern = Rekenkern.laad(await maak(), bron(), CASUS);
    kern.naspelen([]);
    const velden = { ...aanvraag('2026-10-13'), kaart_als_bestuurder: false, kaart_als_passagier: true };
    const v = kern.voorspel({ artikel: ARTIKEL, velden, datum: '2026-10-13', bijAanvraag, inlogveld: 'bsn_aanvrager', bsn: BSN });
    // Regeling gehandicaptenparkeerkaart, art. 2: a passenger's card needs a
    // walking limitation and dependence on help together, or one of the others.
    expect(v.takken.map((t) => [t.vastgesteld, t.uitkomst, !!t.anders])).toEqual([
      [['langdurige loopbeperking', 'continu afhankelijk van hulp bestuurder'], 'toegekend', false],
      [['permanent rolstoelgebonden'], 'toegekend', false],
      [['ernstige beperkingen andere dan loopbeperkingen'], 'toegekend', false],
      [[], 'afgewezen', true],
    ]);
  });

  it('refuses a dossier whose records do not make a chronicle', async () => {
    const m = await maak();
    const bestanden = bron();
    const { records } = eerdereKaart(m, bestanden);
    const kern = Rekenkern.laad(m, bestanden, CASUS);
    // The decision before the application it decides on.
    const omgedraaid = [...records].reverse();
    expect(() => kern.naspelen(omgedraaid)).toThrow();
  });
});
