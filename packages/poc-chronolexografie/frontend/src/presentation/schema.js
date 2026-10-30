/**
 * The picture of one value on the explanation page (SchemaOverzicht.vue), as
 * data: the YAML files one value of a case is computed from as boxes, and
 * every reference from one name to another along the way as a line. Nothing
 * here is written by hand: `keten` reads the files of the bundle (api.js)
 * and follows the value back through what each article names, so a change
 * in the corpus shows up here the next time the page loads.
 *
 * A file is a box (`bestanden`), with the names in it that a line starts or
 * ends at (`begrippen`), in the order of their articles. A line (`lijnen`)
 * runs between two names, with the kind of reference (`soort`, see
 * `LIJNSOORTEN` and `keten`). Only files of the case count: its corpus, and
 * the cells it names with their reductions (src/casus.rs). Of a file with
 * more versions, the one valid on the first day of the case shows.
 */
import { load } from 'js-yaml';

/** What a box is, after its place in the corpus; the order is the column. */
export const SOORTEN = [
  { soort: 'wet', label: 'Wet en regeling' },
  { soort: 'beleid', label: 'Beleid' },
  { soort: 'cel', label: 'Cel' },
  { soort: 'reductie', label: 'Reductie' },
];

/** The kinds of line, in the order of the legend. */
export const LIJNSOORTEN = [
  { soort: 'rekent', label: 'rekent met een naam uit hetzelfde regelwerk', yaml: 'actions, source.parameters' },
  { soort: 'vult_in', label: 'vult een open begrip in', yaml: 'implements' },
  { soort: 'leest', label: 'gebruikt een uitkomst', yaml: 'source.regulation, overrides' },
  { soort: 'vraagt', label: 'vraagt een cel, die een reductie aanwijst', yaml: 'source.reduction, answers' },
  { soort: 'kroniek', label: 'leest de kroniek', yaml: 'source.decisions, source.facts' },
];

const CASUSSEN = 'packages/poc-chronolexografie/casussen/';

/** `<law>#<article>` in its two parts. */
const splits = (ref) => {
  const [law, article] = String(ref).split('#');
  return { law, article };
};

/** The versions of each law in the bundle, per `$id`; and the cells. */
function index(alle, corpus, cellen) {
  const inScope = (pad) =>
    corpus.some((dir) => pad.startsWith(`corpus/${dir}/`)) ||
    cellen.some((cel) => pad.startsWith(`corpus/cells/${cel}/`));
  const wetten = new Map();
  const cel = new Map();
  for (const [pad, tekst] of Object.entries(alle)) {
    if (!pad.endsWith('.yaml') || !inScope(pad)) continue;
    const doc = load(tekst);
    if (pad.endsWith('/cel.yaml')) {
      cel.set(pad.split('/').at(-2), { pad, doc });
    } else if (doc?.$id && Array.isArray(doc.articles)) {
      if (!wetten.has(doc.$id)) wetten.set(doc.$id, []);
      wetten.get(doc.$id).push({ pad, doc });
    }
  }
  return { wetten, cel };
}

/** The version of a law valid on `dag`: the last that starts on or before it. */
function versie(versies, dag) {
  const geldig = versies
    .filter(({ doc }) => String(doc.valid_from) <= dag)
    .sort((a, b) => String(a.doc.valid_from).localeCompare(String(b.doc.valid_from)));
  return geldig.at(-1) ?? versies[0];
}

/** The column of a law, after its layer. */
function soortVan(doc) {
  if (doc.regulatory_layer === 'REDUCTIE') return 'reductie';
  if (doc.regulatory_layer === 'UITVOERINGSBELEID') return 'beleid';
  return 'wet';
}

/**
 * The files of case `id` (`alle`, per path in the repository), the boxes and
 * lines as they are added, and the result in the order of the columns
 * (`uitkomst`).
 */
function opzet(alle, id) {
  const casus = load(alle[`${CASUSSEN}${id}.yaml`]);
  const cellen = [casus.cel, ...(casus.andere_cellen ?? []).map((c) => c.cel)];
  const { wetten, cel } = index(alle, casus.corpus, cellen);
  const dag = String(casus.speeldatum);
  const wet = (lawId) => (wetten.has(lawId) ? versie(wetten.get(lawId), dag) : null);

  /** The boxes, per path, each with its names per key. */
  const bestanden = new Map();
  const lijnen = [];
  const sleutels = new Set();

  function bestand(pad, kop) {
    if (!bestanden.has(pad)) bestanden.set(pad, { pad, ...kop, begrippen: new Map() });
    return bestanden.get(pad);
  }
  const wetBestand = ({ pad, doc }) =>
    bestand(pad, { id: doc.$id, titel: doc.name ?? doc.$id, soort: soortVan(doc) });
  const celBestand = (naam) => {
    const c = cel.get(naam);
    return c && bestand(c.pad, { id: naam, titel: c.doc.name ?? naam, soort: 'cel' });
  };

  /** A name in a box: `naam` in article `artikel`, as a `rol`. */
  function begrip(box, { naam, artikel = null, rol }) {
    const key = `${artikel ?? ''}#${rol}#${naam}`;
    if (!box.begrippen.has(key)) box.begrippen.set(key, { key, naam, artikel, rol });
    return key;
  }
  function lijn(soort, van, vanBegrip, naar, naarBegrip, yaml) {
    const van_ = begrip(van, vanBegrip);
    const naar_ = begrip(naar, naarBegrip);
    const k = `${van.pad}|${van_}|${naar.pad}|${naar_}`;
    if (sleutels.has(k)) return;
    sleutels.add(k);
    lijnen.push({ soort, yaml, van: { pad: van.pad, begrip: van_ }, naar: { pad: naar.pad, begrip: naar_ } });
  }

  const artikelVan = (doc, nummer) => doc.articles.find((a) => String(a.number) === String(nummer));
  const metUitvoer = (doc, naam) =>
    doc.articles.find((a) => a.machine_readable?.execution?.output?.some((o) => o.name === naam));
  const metFeit = (doc, feit) => doc.articles.find((a) => a.machine_readable?.requires?.some((r) => r.fact === feit));

  // Every `implements` in the case's files, per open term it fills in, for
  // the way back from an open term to what fills it in.
  const invullers = new Map();
  for (const [lawId] of wetten) {
    const w = wet(lawId);
    for (const a of w.doc.articles) {
      for (const imp of a.machine_readable?.implements ?? []) {
        const k = `${imp.law}#${imp.article}`;
        if (!invullers.has(k)) invullers.set(k, []);
        invullers.get(k).push({ lawId, artikel: String(a.number), open_term: imp.open_term });
      }
    }
  }

  const volgorde = (b) => SOORTEN.findIndex((s) => s.soort === b.soort);
  const nummer = (n) => String(n ?? '').split(/[^0-9]+/).map(Number);
  const opArtikel = (x, y) => {
    const [p, q] = [nummer(x.artikel), nummer(y.artikel)];
    for (let i = 0; i < Math.max(p.length, q.length); i++) if ((p[i] ?? -1) !== (q[i] ?? -1)) return (p[i] ?? -1) - (q[i] ?? -1);
    // Within an article, a name before the names computed from it.
    return (x.volgorde ?? -1) - (y.volgorde ?? -1);
  };
  const uitkomst = () => ({
    bestanden: [...bestanden.values()]
      .map((b) => ({ ...b, begrippen: [...b.begrippen.values()].sort(opArtikel) }))
      .sort((x, y) => volgorde(x) - volgorde(y)),
    lijnen,
  });

  return { cellen, wet, cel, wetBestand, celBestand, begrip, lijn, artikelVan, metUitvoer, metFeit, invullers, uitkomst };
}

/** Every `$name` a value uses, by the part before a dot. */
function namenIn(waarde, uit = new Set()) {
  if (typeof waarde === 'string' && waarde.startsWith('$')) uit.add(waarde.slice(1).split('.')[0]);
  else if (Array.isArray(waarde)) waarde.forEach((w) => namenIn(w, uit));
  else if (waarde && typeof waarde === 'object') Object.values(waarde).forEach((w) => namenIn(w, uit));
  return uit;
}

/**
 * The picture of one value of case `id`: the name `naam` in article
 * `article` of regelwerk `law`, and everything it is computed from, back
 * along the chain. A line runs from what is needed to what needs it:
 *
 *  - `rekent`: an output is computed from a name in the same article
 *    (`actions`), or an input passes one on (`source.parameters`).
 *  - `leest`: from an output to the input in another file that takes it
 *    (`source.regulation`).
 *  - `vult_in`: from the output that fills in an open term to the term
 *    (`implements`).
 *  - `vraagt`: from the reduction to the cell that names it (`answers` in
 *    its `cel.yaml`), and on to the input that asks it (`source.reduction`).
 *  - `kroniek`: from what a cell recorded to the input that reads it:
 *    decisions under an article (`source.decisions`) or facts an article
 *    requires (`source.facts`).
 *
 * Parameters are left out: they are what the case gives (a KvK number, the
 * day of the decision), not something the chain decides. An open term that
 * nothing in the case fills in ends the chain: someone decides it per case.
 */
export function keten(alle, id, { law, article, naam }) {
  const { wet, cel, cellen, celBestand, wetBestand, begrip, lijn, artikelVan, metUitvoer, metFeit, invullers, uitkomst } = opzet(alle, id);
  const klaar = new Map();

  /**
   * The box and name that hold `naam` in article `nummer` of `lawId`, after
   * drawing the lines into it; null for a parameter or a name the article
   * does not have (the item of a FOREACH, say). `gegeven` holds the literal
   * values a caller passed, for a source that names a parameter
   * (`decisions: $basis`).
   */
  function knoop(lawId, nummer, naam, gegeven = {}) {
    const k = `${lawId}#${nummer}#${naam}#${JSON.stringify(gegeven)}`;
    if (klaar.has(k)) return klaar.get(k);
    klaar.set(k, null);
    const w = wet(lawId);
    const mr = w && artikelVan(w.doc, nummer)?.machine_readable;
    if (!mr) return null;
    const box = wetBestand(w);
    const ex = mr.execution ?? {};
    const hier = (rol) => ({ naam, artikel: String(nummer), rol });
    let uit = null;

    const actie = ex.actions?.find((a) => a.output === naam);
    const invoer = ex.input?.find((i) => i.name === naam);
    if (actie) {
      uit = { box, begrip: hier('output') };
      klaar.set(k, uit);
      for (const v of namenIn(actie.value)) {
        const d = knoop(lawId, nummer, v, gegeven);
        if (d) lijn('rekent', d.box, d.begrip, box, uit.begrip, 'actions');
      }
    } else if (invoer) {
      uit = { box, begrip: hier('input') };
      klaar.set(k, uit);
      const s = invoer.source ?? {};
      // What the input passes on to its source, and what that source is given.
      const door = {};
      for (const [p, v] of Object.entries(s.parameters ?? {})) {
        if (typeof v !== 'string') continue;
        if (!v.startsWith('$')) door[p] = v;
        else if (gegeven[v.slice(1)]) door[p] = gegeven[v.slice(1)];
        const d = v.startsWith('$') && knoop(lawId, nummer, v.slice(1), gegeven);
        if (d) lijn('rekent', d.box, d.begrip, box, uit.begrip, 'source.parameters');
      }
      if (s.regulation || s.output) {
        const doelId = s.regulation ?? lawId;
        const doel = wet(doelId);
        const art = doel && metUitvoer(doel.doc, s.output);
        const d = art && knoop(doelId, art.number, s.output, s.regulation ? door : { ...gegeven, ...door });
        if (d) lijn(s.regulation && s.regulation !== lawId ? 'leest' : 'rekent', d.box, d.begrip, box, uit.begrip, s.regulation ? 'source.regulation' : 'source.output');
      } else if (s.reduction) {
        for (const c of cellen) {
          const reductie = cel.get(c)?.doc.answers?.[s.reduction];
          const doel = reductie && wet(reductie);
          const art = doel && metUitvoer(doel.doc, s.reduction);
          if (!art) continue;
          const celBox = celBestand(c);
          const vraag = { naam: s.reduction, rol: 'answers' };
          const d = knoop(reductie, art.number, s.reduction, door);
          if (d) lijn('vraagt', d.box, d.begrip, celBox, vraag, 'answers');
          lijn('vraagt', celBox, vraag, box, uit.begrip, 'source.reduction');
        }
      } else if (s.facts) {
        const [fLaw, fArt, feit] = String(s.facts).split('#');
        const doel = wet(fLaw);
        const art = doel && (metFeit(doel.doc, feit) ?? artikelVan(doel.doc, fArt));
        if (art) lijn('kroniek', wetBestand(doel), { naam: feit, artikel: String(art.number), rol: 'fact' }, box, uit.begrip, 'source.facts');
      } else if (s.decisions) {
        const ref = String(s.decisions).startsWith('$') ? gegeven[String(s.decisions).slice(1)] : s.decisions;
        const { law: dLaw, article: dArt } = splits(ref ?? '');
        const doel = wet(dLaw);
        if (doel) lijn('kroniek', wetBestand(doel), { naam: `besluiten onder artikel ${dArt}`, artikel: dArt, rol: 'decisions' }, box, uit.begrip, 'source.decisions');
      }
    } else if (mr.open_terms?.some((t) => t.id === naam)) {
      uit = { box, begrip: hier('open_term') };
      klaar.set(k, uit);
      // Draw the term even when nothing fills it in: then the chain ends here.
      begrip(box, uit.begrip);
      for (const inv of invullers.get(`${lawId}#${nummer}`) ?? []) {
        if (inv.open_term !== naam) continue;
        const d = knoop(inv.lawId, inv.artikel, naam);
        if (d) lijn('vult_in', d.box, d.begrip, box, uit.begrip, 'implements');
      }
    }
    // Finished: everything it needs is drawn, so it comes after those.
    if (uit) uit.box.begrippen.get(begrip(uit.box, uit.begrip)).volgorde = volgorde++;
    return uit;
  }
  let volgorde = 0;

  const doel = knoop(law, String(article), naam);
  return { ...uitkomst(), doel: doel && { pad: doel.box.pad, begrip: `${doel.begrip.artikel ?? ''}#${doel.begrip.rol}#${doel.begrip.naam}` } };
}
