/**
 * The overview at the top of the explanation page (SchemaOverzicht.vue), as
 * data: every YAML file one case runs on as a box, and every reference from
 * a name in one file to a name in another as a line. Nothing here is
 * written by hand: `schema` reads the files of the bundle (api.js), starts
 * at the case file and follows what each article it reaches names, so a
 * change in the corpus shows up here the next time the page loads.
 *
 * A file is a box (`bestanden`), with the names in it that a line starts or
 * ends at (`begrippen`), in the order of their articles. A line
 * (`lijnen`) runs from the name that refers to the name it refers to, with
 * the kind of reference (`soort`):
 *
 *  - `vult_in`: an article fills in an open term of another
 *    (`implements`).
 *  - `leest`: an input takes the output of another file
 *    (`source.regulation`), or an article replaces an output of
 *    another (`overrides`).
 *  - `vraagt`: an input asks a cell (`source.reduction`), and the cell
 *    names the reduction that answers (`answers` in its `cel.yaml`).
 *  - `kroniek`: a reduction reads what a cell recorded: decisions under an
 *    article (`source.decisions`) or facts an article requires
 *    (`source.facts`).
 *  - `casus`: the case file names its cells and the article of the
 *    application.
 *
 * A reference within one file is no line, but it does take the walk along:
 * the article it names counts as reached. An article is reached from the
 * case's article, along every reference, and back along every `implements`
 * that fills in one of its open terms. Only files of the case count: its
 * corpus, and the cells it names with their reductions (src/casus.rs). Of a
 * file with more versions, the one valid on the first day of the case
 * shows.
 */
import { load } from 'js-yaml';

/** What a box is, after its place in the corpus; the order is the column. */
export const SOORTEN = [
  { soort: 'casus', label: 'Casus' },
  { soort: 'wet', label: 'Wet en regeling' },
  { soort: 'beleid', label: 'Beleid' },
  { soort: 'cel', label: 'Cel' },
  { soort: 'reductie', label: 'Reductie' },
];

/** The kinds of line, in the order of the legend. */
export const LIJNSOORTEN = [
  { soort: 'vult_in', label: 'vult een open begrip in', yaml: 'implements' },
  { soort: 'leest', label: 'gebruikt een uitkomst', yaml: 'source.regulation, overrides' },
  { soort: 'vraagt', label: 'vraagt een cel, die een reductie aanwijst', yaml: 'source.reduction, answers' },
  { soort: 'kroniek', label: 'leest de kroniek', yaml: 'source.decisions, source.facts' },
  { soort: 'casus', label: 'casus kiest cel en artikel', yaml: 'cel, artikel' },
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
 * The boxes and lines of case `id`, from the files of the bundle (`alle`, per
 * path in the repository).
 */
export function schema(alle, id) {
  const casusPad = `${CASUSSEN}${id}.yaml`;
  const casus = load(alle[casusPad]);
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

  // The walk over articles, as `<$id>#<number>`.
  const bereikt = new Set();
  const rij = [];
  const bereik = (lawId, nummer) => {
    const k = `${lawId}#${nummer}`;
    if (bereikt.has(k) || !wet(lawId)) return;
    bereikt.add(k);
    rij.push([lawId, String(nummer)]);
  };
  const artikelVan = (doc, nummer) => doc.articles.find((a) => String(a.number) === String(nummer));
  const metUitvoer = (doc, naam) =>
    doc.articles.find((a) => a.machine_readable?.execution?.output?.some((o) => o.name === naam));
  const metFeit = (doc, feit) => doc.articles.find((a) => a.machine_readable?.requires?.some((r) => r.fact === feit));

  // The case file: its cells and the article of the application.
  const casusBox = bestand(casusPad, { id, titel: casus.titel, soort: 'casus' });
  for (const [i, naam] of cellen.entries()) {
    const box = celBestand(naam);
    if (box) lijn('casus', casusBox, { naam, rol: i ? 'andere_cellen' : 'cel' }, box, { naam: box.titel, rol: 'cel' }, 'cel');
  }
  {
    const { law, article } = splits(casus.artikel);
    const w = wet(law);
    if (w) {
      lijn('casus', casusBox, { naam: casus.artikel, rol: 'artikel' }, wetBestand(w), { naam: `artikel ${article}`, artikel: article, rol: 'artikel' }, 'artikel');
      bereik(law, article);
    }
  }

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

  // What a parameter is given by the callers of a reduction, for a source
  // that names it (`decisions: $basis`): literal values only, and only from
  // callers this case reaches. The reduction of the Awb serves every cell,
  // so its other callers belong to other cases.
  const meegegeven = (lawId, param) => {
    const waarden = new Set();
    for (const k of bereikt) {
      const [anderId, nummer] = [k.slice(0, k.indexOf('#')), k.slice(k.indexOf('#') + 1)];
      for (const a of [artikelVan(wet(anderId).doc, nummer)].filter(Boolean)) {
        for (const i of a.machine_readable?.execution?.input ?? []) {
          const v = i.source?.regulation === lawId ? i.source.parameters?.[param] : undefined;
          if (typeof v === 'string' && !v.startsWith('$')) waarden.add(v);
        }
      }
    }
    return [...waarden];
  };

  while (rij.length) {
    const [lawId, nummer] = rij.shift();
    const w = wet(lawId);
    const box = wetBestand(w);
    const a = artikelVan(w.doc, nummer);
    const mr = a?.machine_readable;
    if (!mr) continue;

    // Open terms of this article, filled in elsewhere.
    for (const inv of invullers.get(`${lawId}#${nummer}`) ?? []) {
      bereik(inv.lawId, inv.artikel);
      if (inv.lawId === lawId) continue;
      lijn('vult_in', wetBestand(wet(inv.lawId)), { naam: inv.open_term, artikel: inv.artikel, rol: 'output' }, box, { naam: inv.open_term, artikel: nummer, rol: 'open_term' }, 'implements');
    }
    for (const imp of mr.implements ?? []) bereik(imp.law, imp.article);

    for (const o of mr.overrides ?? []) {
      const doel = wet(o.law);
      if (!doel) continue;
      bereik(o.law, o.article);
      lijn('leest', box, { naam: o.output, artikel: nummer, rol: 'output' }, wetBestand(doel), { naam: o.output, artikel: String(o.article), rol: 'output' }, 'overrides');
    }

    for (const inp of mr.execution?.input ?? []) {
      const s = inp.source ?? {};
      const hier = { naam: inp.name, artikel: nummer, rol: 'input' };
      if (s.regulation) {
        const doel = wet(s.regulation);
        const art = doel && metUitvoer(doel.doc, s.output);
        if (!art) continue;
        bereik(s.regulation, art.number);
        if (s.regulation !== lawId) {
          lijn('leest', box, hier, wetBestand(doel), { naam: s.output, artikel: String(art.number), rol: 'output' }, 'source.regulation');
        }
      } else if (s.output) {
        const art = metUitvoer(w.doc, s.output);
        if (art) bereik(lawId, art.number);
      } else if (s.reduction) {
        // Every cell of the case that answers the question.
        for (const naam of cellen) {
          const c = cel.get(naam);
          const reductie = c?.doc.answers?.[s.reduction];
          if (!reductie) continue;
          const celBox = celBestand(naam);
          lijn('vraagt', box, hier, celBox, { naam: s.reduction, rol: 'answers' }, 'source.reduction');
          const doel = wet(reductie);
          const art = doel && metUitvoer(doel.doc, s.reduction);
          if (!art) continue;
          bereik(reductie, art.number);
          lijn('vraagt', celBox, { naam: s.reduction, rol: 'answers' }, wetBestand(doel), { naam: s.reduction, artikel: String(art.number), rol: 'output' }, 'answers');
        }
      } else if (s.facts) {
        const [law, article, feit] = String(s.facts).split('#');
        const doel = wet(law);
        const art = doel && (metFeit(doel.doc, feit) ?? artikelVan(doel.doc, article));
        if (!art) continue;
        bereik(law, art.number);
        lijn('kroniek', box, hier, wetBestand(doel), { naam: feit, artikel: String(art.number), rol: 'fact' }, 'source.facts');
      } else if (s.decisions) {
        const refs = String(s.decisions).startsWith('$')
          ? meegegeven(lawId, String(s.decisions).slice(1))
          : [s.decisions];
        for (const ref of refs) {
          const { law, article } = splits(ref);
          const doel = wet(law);
          if (!doel) continue;
          bereik(law, article);
          lijn('kroniek', box, hier, wetBestand(doel), { naam: `besluiten onder artikel ${article}`, artikel: article, rol: 'decisions' }, 'source.decisions');
        }
      }
    }
  }

  const volgorde = (b) => SOORTEN.findIndex((s) => s.soort === b.soort);
  const nummer = (n) => String(n ?? '').split(/[^0-9]+/).map(Number);
  const opArtikel = (x, y) => {
    const [p, q] = [nummer(x.artikel), nummer(y.artikel)];
    for (let i = 0; i < Math.max(p.length, q.length); i++) if ((p[i] ?? -1) !== (q[i] ?? -1)) return (p[i] ?? -1) - (q[i] ?? -1);
    return 0;
  };
  return {
    bestanden: [...bestanden.values()]
      .map((b) => ({ ...b, begrippen: [...b.begrippen.values()].sort(opArtikel) }))
      .sort((x, y) => volgorde(x) - volgorde(y)),
    lijnen,
  };
}
