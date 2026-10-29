/**
 * Het spel van packages/poc-chronolexografie, als WASM in deze browser. Er is
 * geen server: de cel, de engine en de kroniek draaien hier, één spel per
 * tabblad, op de bestanden die de pagina meekrijgt (public/bundel.json, zie
 * scripts/copy-assets.js). Eerst kies je een casus (`kies`); elke andere stap
 * werkt op de gekozen casus. Elk antwoord is JSON, zoals de HTTP-API het gaf
 * toen de app nog een server had, en een weigering van de cel is een fout met
 * de melding van de cel.
 *
 * Naast de bundel krijgt de cel de bronnen die de pagina bij het laden
 * ophaalt (src/bronnen.js): een casus uit een bron draait alleen op die
 * bron. Een bron die er niet is, houdt de app niet tegen.
 *
 * Elk pad gaat langs `b()`: achter het poc-portaal staat de app onder
 * /chronolexografie/. De JS-lijm van wasm-bindgen komt via een blob-URL
 * binnen, omdat Vite geen imports uit public/ toestaat (zoals useEngine.js in
 * frontend-poc-terugbetaalregimes).
 */
import { haalBronnen } from './bronnen.js';

const b = (pad) => import.meta.env.BASE_URL + String(pad).replace(/^\//, '');

async function tekst(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status} ${res.statusText}`);
  return res.text();
}

/**
 * De bestanden van de bundel, per pad in de repository: de uitlegpagina leest
 * er de YAML van haar panelen uit. De cel haalt dezelfde bundel op; de tweede
 * keer komt hij uit de cache van de browser.
 */
export const bestanden = async () => JSON.parse(await tekst(b('/bundel.json')));

let laden = null;

/** Het spel van dit tabblad; de eerste keer laadt het de cel. */
function poc() {
  laden ??= (async () => {
    try {
      const lijm = await tekst(b('/wasm/pkg/regelrecht_poc_chronolexografie.js'));
      const url = URL.createObjectURL(new Blob([lijm], { type: 'application/javascript' }));
      const wasm = await import(/* @vite-ignore */ url);
      URL.revokeObjectURL(url);
      await wasm.default({ module_or_path: b('/wasm/pkg/regelrecht_poc_chronolexografie_bg.wasm') });
      const [bundel, bronnen] = await Promise.all([
        tekst(b('/bundel.json')),
        haalBronnen(b('/bronnen.json'), (naam) => b(`/bron/${encodeURIComponent(naam)}`)),
      ]);
      return new wasm.Poc(bundel, JSON.stringify(bronnen));
    } catch (e) {
      // Een volgende stap probeert het opnieuw.
      laden = null;
      throw new Error(`the cell did not load: ${e?.message ?? e}`);
    }
  })();
  return laden;
}

/**
 * Een stap van het spel; het antwoord is JSON. Een panic in de cel breekt de
 * WASM-instantie af (een `WebAssembly.RuntimeError`), en daarna weigert elke
 * stap: dat zegt de melding, en een herlaad gaat verder bij de laatste stap
 * die wel lukte (state.js bewaart het spel na elke stap).
 */
let gestopt = null;
async function roep(stap) {
  const p = await poc();
  if (gestopt) throw new Error(gestopt);
  try {
    return JSON.parse(stap(p));
  } catch (e) {
    if (e instanceof WebAssembly.RuntimeError) {
      gestopt = `the cell stopped: ${e.message}`;
      throw new Error(gestopt);
    }
    throw e;
  }
}

const json = (v) => JSON.stringify(v ?? {});

export const api = {
  casussen: () => roep((p) => p.casussen()),
  casus: () => roep((p) => p.casus()),
  kies: (casus) => roep((p) => p.kies(casus)),
  vormen: (datum) => roep((p) => p.vormen(datum)),
  kroniek: () => roep((p) => p.kroniek()),
  aanvragen: (velden, datum) => roep((p) => p.aanvragen(json(velden), datum)),
  // Een andere cel van de casus legt een handeling vast, zoals de GGD een
  // geneeskundig onderzoek (RFC-052).
  vastleggen: (cel, feit, velden, datum) => roep((p) => p.vastleggen(cel, feit, json(velden), datum)),
  besluiten: (op, bevindingen, datum) => roep((p) => p.besluiten(op, json(bevindingen), datum)),
  // Wat het besluit zou worden, op een kopie van de cel: er wordt niets
  // vastgelegd. Een weigering van de cel komt terug als `{ geweigerd }`.
  voorvertoningBesluit: (op, bevindingen, datum) =>
    roep((p) => p.voorvertoningBesluit(op, json(bevindingen), datum)),
  bezwaren: (besluit, velden, datum) => roep((p) => p.bezwaren(besluit, json(velden), datum)),
  heroverweging: (bezwaar, datum) => roep((p) => p.heroverweging(bezwaar, datum)),
  beslissenOpBezwaar: (op, bevindingen, datum) =>
    roep((p) => p.beslissenOpBezwaar(op, json(bevindingen), datum)),
  voorvertoningOpBezwaar: (op, bevindingen, datum) =>
    roep((p) => p.voorvertoningOpBezwaar(op, json(bevindingen), datum)),
  // Het beroep bij de bestuursrechter (RFC-053): de aanvrager stelt het in,
  // de rechtbank doet uitspraak en zendt die toe, en het bestuursorgaan
  // beslist opnieuw op het bezwaar.
  beroepInstellen: (besluit, velden, datum) =>
    roep((p) => p.beroepInstellen(besluit, json(velden), datum)),
  uitspraakDoen: (op, bevindingen, datum) => roep((p) => p.uitspraakDoen(op, json(bevindingen), datum)),
  voorvertoningUitspraak: (op, bevindingen, datum) =>
    roep((p) => p.voorvertoningUitspraak(op, json(bevindingen), datum)),
  toezenden: (uitspraak, datum) => roep((p) => p.toezenden(uitspraak, datum)),
  heroverwegingNaUitspraak: (ontvangst) => roep((p) => p.heroverwegingNaUitspraak(ontvangst)),
  opnieuwBeslissenOpBezwaar: (ontvangst, bevindingen, datum) =>
    roep((p) => p.opnieuwBeslissenOpBezwaar(ontvangst, json(bevindingen), datum)),
  voorvertoningNaUitspraak: (ontvangst, bevindingen, datum) =>
    roep((p) => p.voorvertoningNaUitspraak(ontvangst, json(bevindingen), datum)),
  reducties: (datum) => roep((p) => p.reducties(datum)),
  logboek: () => roep((p) => p.logboek()),
  reset: () => roep((p) => p.reset()),
  // Het spel tot nu toe (de casus en de kroniek van elke cel), om een
  // herlaad te overleven, en terug.
  bewaren: () => roep((p) => p.bewaren()),
  herstellen: (bewaard) => roep((p) => p.herstellen(JSON.stringify(bewaard))),
};
