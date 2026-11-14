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
import { haalBronnen, leesBronLijst } from './bronnen.js';

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

let lijst = null;
let bronnen = null;

/**
 * The sources the build names, `{ naam, verborgen }`, read once per tab: the
 * choice screen leaves out the cases of a hidden one (verborgen.js).
 */
export function bronLijst() {
  lijst ??= leesBronLijst(b('/bronnen.json'));
  return lijst;
}

/**
 * The sources, fetched once per tab: the cell gets them when it loads, and
 * the sheet of an article reads its text from them for a case from a source.
 */
export function opgehaaldeBronnen() {
  bronnen ??= bronLijst().then((l) => haalBronnen(l, (naam) => b(`/bron/${encodeURIComponent(naam)}`)));
  return bronnen;
}

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
      const [bundel, opgehaald] = await Promise.all([
        tekst(b('/bundel.json')),
        opgehaaldeBronnen(),
      ]);
      return new wasm.Poc(bundel, JSON.stringify(opgehaald));
    } catch (e) {
      // A next step tries again, the sources included.
      laden = null;
      lijst = null;
      bronnen = null;
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
  // What the sources give per field of the application before anything is
  // submitted (voorinvulling.js): `bronnen` per field
  // `<regelwerk>#<article>#<output>`. Nothing is recorded.
  voorinvullen: (bronnen, velden, datum) =>
    roep((p) => p.voorinvullen(json(bronnen), json(velden), datum)),
  // De generieke stappen: het spel noemt geen stap van een procedure. Welke
  // indieningen er zijn en wie erop beslist, leidt het af uit de wet
  // (`casus.indieningen`, `casus.ambtshalve`); awb.js speelt er de stappen
  // van de Awb mee zoals de pagina ze toont.
  // Een indiening onder `artikel`, die verwijst naar besluit `op`, of geen.
  indienen: (artikel, velden, datum, op = null) =>
    roep((p) => p.indienen(artikel, json(velden), datum, op ?? undefined)),
  // Een andere cel van de casus legt een handeling vast, zoals de GGD een
  // geneeskundig onderzoek (RFC-048). `verwijstNaar` is het kenmerk van een
  // eerder record in het begin van die cel waarover het feit gaat, of null.
  vastleggen: (cel, feit, velden, datum, verwijstNaar = null) =>
    roep((p) => p.vastleggen(cel, feit, json(velden), datum, verwijstNaar ?? undefined)),
  // Beslissen op indiening `op`; `na` is een later record waarna opnieuw
  // wordt beslist, zoals de ontvangst van een uitspraak.
  beslissen: (op, bevindingen, datum, na = null) =>
    roep((p) => p.beslissen(op, json(bevindingen), datum, na ?? undefined)),
  // Wat beslissen op `op` zou geven met wat het bestuursorgaan over het
  // beslissende artikel zelf oordeelt (`oordeel`) en zonder nieuwe
  // bevindingen, en wat het opnieuw kan vinden.
  voorBeslissen: (op, oordeel, datum, na = null) =>
    roep((p) => p.voorBeslissen(op, json(oordeel), datum, na ?? undefined)),
  // Wat het besluit zou worden, op een kopie van de cel: er wordt niets
  // vastgelegd. Een weigering van de cel komt terug als `{ geweigerd }`.
  voorvertoning: (op, bevindingen, datum, na = null) =>
    roep((p) => p.voorvertoning(op, json(bevindingen), datum, na ?? undefined)),
  // Een besluit dat een cel ambtshalve neemt onder `artikel`, over record
  // `over` of geen.
  ambtshalve: (artikel, gegeven, datum, over = null) =>
    roep((p) => p.ambtshalve(artikel, json(gegeven), datum, over ?? undefined)),
  // De cel die besluit `besluit` nam, zendt er een afschrift van naar de cel
  // over wier besluit het gaat.
  toezenden: (besluit, datum) => roep((p) => p.toezenden(besluit, datum)),
  logboek: () => roep((p) => p.logboek()),
  reset: () => roep((p) => p.reset()),
  // Het spel tot nu toe (de casus en de kroniek van elke cel), om een
  // herlaad te overleven, en terug.
  bewaren: () => roep((p) => p.bewaren()),
  herstellen: (bewaard) => roep((p) => p.herstellen(JSON.stringify(bewaard))),
};
