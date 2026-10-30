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
 * `LIJNSOORTEN` and `keten`). Only files of the case count, as src/casus.rs
 * counts them: the directories its corpus names, its own regelwerken (under
 * `casussen/<id>/regulation/nl`), and the cells it names with their
 * reductions; all under the corpus of the poc (corpus.js). Of a file with
 * more versions, the one valid on the first day of the case shows.
 */
import { load } from 'js-yaml';
import { CORPUS } from '../corpus.js';

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
  { soort: 'leest', label: 'gebruikt een uitkomst', yaml: 'source.regulation' },
  { soort: 'vraagt', label: 'vraagt een cel, die een reductie aanwijst', yaml: 'source.reduction, answers' },
  { soort: 'kroniek', label: 'leest de kroniek', yaml: 'source.decisions, source.facts' },
];

/** Where the case files are, each `<id>.yaml` with its own regelwerken in `<id>/`. */
export const CASUSSEN = `${CORPUS}/casussen/`;

/** A name in a box as one string: `begrip` is its key in `begrippen`. */
export const sleutel = (pad, begrip) => `${pad}|${begrip}`;
const id = (e) => sleutel(e.pad, e.begrip);

/** `<law>#<article>` in its two parts. */
const splits = (ref) => {
  const [law, article] = String(ref).split('#');
  return { law, article };
};

/** The versions of each law of case `id` in the bundle, per `$id`; and the cells. */
function index(alle, id, corpus, cellen) {
  const mappen = [
    ...corpus.map((dir) => `${CORPUS}/${dir}/`),
    `${CASUSSEN}${id}/regulation/nl/`,
    ...cellen.map((cel) => `${CORPUS}/cells/${cel}/`),
  ];
  const inScope = (pad) => mappen.some((m) => pad.startsWith(m));
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
  const { wetten, cel } = index(alle, id, casus.corpus ?? [], cellen);
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

  /** A name in a box: `naam` in article `artikel`, as a `rol`; `omschrijving` is what the law says it is. */
  function begrip(box, { naam, artikel = null, rol, omschrijving }) {
    const key = `${artikel ?? ''}#${rol}#${naam}`;
    if (!box.begrippen.has(key)) box.begrippen.set(key, { key, naam, artikel, rol, omschrijving });
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
 * Once drawn, the picture is made simpler (`eenvoudig`): an output that only
 * passes on an input is one name with it, as is an input that only takes an
 * output of its own file, inputs that are the same are one name, and a
 * reduction shows only what it reads and what it answers. Every name then
 * gets its step (`genummerd`).
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
        // An output that is just an input of its own article: a candidate to merge.
        if (d && actie.value === `$${v}` && d.box === box && d.begrip.rol === 'input') {
          doorgaven.push({ pad: box.pad, uit: begrip(box, uit.begrip), in: begrip(box, d.begrip) });
        }
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
      uit = { box, begrip: { ...hier('open_term'), omschrijving: mr.open_terms.find((t) => t.id === naam).description } };
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
  const doorgaven = [];

  const start = knoop(law, String(article), naam);
  const doel = start && { pad: start.box.pad, begrip: `${start.begrip.artikel ?? ''}#${start.begrip.rol}#${start.begrip.naam}` };
  return { ...genummerd(eenvoudig(uitkomst(), doorgaven), doel), doel };
}

/**
 * The picture made as simple as it can be without losing a step of the law,
 * once it is drawn:
 *
 *  - An output that only passes on an input of its own article
 *    (`value: $input`) and that input become one name: the output, with the
 *    lines into the input drawn into it, of the same kind. Not when the
 *    input is used by something else in the picture as well; `doorgaven`
 *    holds the candidates.
 *  - An input that only takes an output of its own file is that output: what
 *    uses the input, uses the output. A source of another file stays, because
 *    the input is where the line into the box ends.
 *  - Inputs in one file with the same name and the same sources are one name:
 *    the first.
 *  - A reduction shows only the names that connect to another file: what it
 *    reads and what it answers. The steps in between are how it reduces, not
 *    what the chain needs; a `rekent` line from each name it reads to each
 *    name it answers that this feeds takes their place. In a law or policy
 *    the steps in between are its reasoning, so they stay.
 */
function eenvoudig({ bestanden, lijnen }, doorgaven) {
  const weg = new Set();
  let na = lijnen;
  /** Let names take the place of others (`plek`: by id, the name that stays). */
  const vervang = (plek) => {
    const ander = (e) => plek.get(id(e)) ?? e;
    na = na.map((l) => ({ ...l, van: ander(l.van), naar: ander(l.naar) })).filter((l) => id(l.van) !== id(l.naar));
    for (const k of plek.keys()) weg.add(k);
  };
  const naarToe = (e) => na.filter((l) => id(l.naar) === id(e));

  vervang(
    new Map(
      doorgaven
        .map((d) => ({ uit: { pad: d.pad, begrip: d.uit }, invoer: { pad: d.pad, begrip: d.in } }))
        .filter(({ uit, invoer }) => {
          const van = na.filter((l) => id(l.van) === id(invoer));
          return van.length === 1 && id(van[0].naar) === id(uit);
        })
        .map(({ uit, invoer }) => [id(invoer), uit]),
    ),
  );

  const invoeren = bestanden.flatMap((b) =>
    b.begrippen.filter((g) => g.rol === 'input').map((g) => ({ pad: b.pad, begrip: g.key, naam: g.naam })),
  );
  const isInvoer = new Set(invoeren.map(id));
  vervang(
    new Map(
      na
        .filter(
          (l) =>
            isInvoer.has(id(l.naar)) &&
            l.van.pad === l.naar.pad &&
            ['source.output', 'source.regulation'].includes(l.yaml) &&
            naarToe(l.naar).length === 1,
        )
        .map((l) => [id(l.naar), l.van]),
    ),
  );

  const eerste = new Map();
  const dubbel = new Map();
  for (const e of invoeren) {
    const bronnen = naarToe(e).map((l) => `${l.soort}|${id(l.van)}`).sort();
    if (weg.has(id(e)) || !bronnen.length) continue;
    const k = JSON.stringify([e.pad, e.naam, bronnen]);
    if (eerste.has(k)) dubbel.set(id(e), { pad: e.pad, begrip: eerste.get(k).begrip });
    else eerste.set(k, e);
  }
  vervang(dubbel);

  const soort = new Map(bestanden.map((b) => [b.pad, b.soort]));
  const buiten = new Set(na.filter((l) => l.van.pad !== l.naar.pad).flatMap((l) => [id(l.van), id(l.naar)]));
  const tussen = (e) => soort.get(e.pad) === 'reductie' && !buiten.has(id(e));
  const verder = new Map();
  for (const l of na) if (tussen(l.van)) verder.set(id(l.van), [...(verder.get(id(l.van)) ?? []), l.naar]);

  const uit = [];
  const gezien = new Set();
  const voeg = (l) => {
    const k = `${id(l.van)}|${id(l.naar)}`;
    if (k !== `${id(l.van)}|${id(l.van)}` && !gezien.has(k)) uit.push(l), gezien.add(k);
  };
  for (const l of na) {
    if (tussen(l.van)) continue;
    if (!tussen(l.naar)) {
      voeg(l);
      continue;
    }
    // Through the steps in between, to every name that stays.
    const bezocht = new Set();
    const rij = [l.naar];
    while (rij.length) {
      const e = rij.pop();
      if (bezocht.has(id(e))) continue;
      bezocht.add(id(e));
      if (tussen(e)) rij.push(...(verder.get(id(e)) ?? []));
      else voeg({ soort: 'rekent', yaml: 'actions', van: l.van, naar: e });
    }
  }

  return {
    bestanden: bestanden.map((b) => ({
      ...b,
      begrippen: b.begrippen.filter((g) => !weg.has(id({ pad: b.pad, begrip: g.key })) && !tussen({ pad: b.pad, begrip: g.key })),
    })),
    lijnen: uit,
  };
}

/**
 * The order of the chain, from the question it answers. A step is a group of
 * names: those of one article that `rekent` lines hold together, or all of a
 * reduction. The steps first walk one path, from `doel` along the longest way
 * over these groups, to the sources at the end of it, which are one step
 * together. Of two ways that are as long, the path takes the one with the most
 * sources behind it. Everything else is one last step, `reststap`; without
 * anything else there is none (null). `stappen` is how many there are.
 */
function genummerd({ bestanden, lijnen }, doel) {
  const namen = bestanden.flatMap((b) => b.begrippen.map((g) => ({ k: sleutel(b.pad, g.key), soort: b.soort, pad: b.pad, artikel: g.artikel })));
  const ouder = new Map(namen.map((g) => [g.k, g.k]));
  const wortel = (k) => (ouder.get(k) === k ? k : wortel(ouder.get(k)));
  const samen = (a, b) => ouder.set(wortel(a), wortel(b));
  const artikel = new Map(namen.map((g) => [g.k, g.artikel]));
  for (const g of namen) {
    const eerste = namen.find((x) => x.pad === g.pad);
    if (g.soort === 'reductie') samen(g.k, eerste.k);
  }
  for (const l of lijnen) {
    if (l.soort === 'rekent' && l.van.pad === l.naar.pad && artikel.get(id(l.van)) === artikel.get(id(l.naar))) samen(id(l.van), id(l.naar));
  }

  // What each group needs, as groups.
  const voor = new Map();
  for (const l of lijnen) {
    const [van, naar] = [wortel(id(l.van)), wortel(id(l.naar))];
    if (van !== naar) voor.set(naar, (voor.get(naar) ?? new Set()).add(van));
  }
  const nodig = (g) => [...(voor.get(g) ?? [])];
  const lengtes = new Map();
  const lengte = (g) => {
    if (!lengtes.has(g)) {
      lengtes.set(g, 0); // A group that needs itself ends the walk here.
      lengtes.set(g, 1 + Math.max(-1, ...nodig(g).map(lengte)));
    }
    return lengtes.get(g);
  };
  const bronnen = new Map();
  const bronnenVan = (g) => {
    if (!bronnen.has(g)) {
      bronnen.set(g, new Set());
      bronnen.set(g, new Set(nodig(g).length ? nodig(g).flatMap((x) => [...bronnenVan(x)]) : [g]));
    }
    return bronnen.get(g);
  };

  const pad = [];
  const gehad = new Set();
  for (let g = doel && wortel(id(doel)); g && !gehad.has(g); ) {
    gehad.add(g);
    pad.push([g]);
    const verder = nodig(g).filter((x) => !gehad.has(x));
    if (verder.length && verder.every((x) => !nodig(x).length)) {
      pad.push(verder);
      break;
    }
    g = verder.reduce((a, b) => (lengte(b) - lengte(a) || bronnenVan(b).size - bronnenVan(a).size) > 0 ? b : a, verder[0]);
  }
  const stap = new Map(pad.flatMap((groepen, i) => groepen.map((g) => [g, i + 1])));
  const reststap = namen.some((g) => !stap.has(wortel(g.k))) ? pad.length + 1 : null;
  return {
    bestanden: bestanden.map((b) => ({
      ...b,
      begrippen: b.begrippen.map((g) => ({ ...g, stap: stap.get(wortel(sleutel(b.pad, g.key))) ?? reststap })),
    })),
    lijnen,
    stappen: reststap ?? pad.length,
    reststap,
  };
}

/**
 * What step `n` of `schema` shows: the names by key of the step (`nu`) and of
 * it and those before it (`gehad`), and of a line, by the keys of its ends,
 * whether it belongs to the step (`nu`), to one before it (`gehad`), or to what
 * is yet to come (null).
 */
export function stapBeeld({ bestanden }, n) {
  const namen = bestanden.flatMap((b) => b.begrippen.map((g) => ({ k: sleutel(b.pad, g.key), stap: g.stap })));
  const nu = new Set(namen.filter((g) => g.stap === n).map((g) => g.k));
  const gehad = new Set(namen.filter((g) => g.stap <= n).map((g) => g.k));
  const lijn = ({ van, naar }) => {
    if (!gehad.has(van) || !gehad.has(naar)) return null;
    return nu.has(van) || nu.has(naar) ? 'nu' : 'gehad';
  };
  return { nu, gehad, lijn };
}

/** The step `stapje` away from `stap` (null is the whole picture), within the chain. */
export const andereStap = (stap, stapje, stappen) => Math.min(stappen, Math.max(1, (stap ?? 0) + stapje));

/**
 * What a step can say a name is, after the line that leaves it: one, many, and
 * where it comes from. `vraagt` is the cell's side, `vraagt_reductie` the
 * reduction's. The order is the order in the sentence.
 */
const HERKOMST = {
  rekent: ['naam', 'namen', 'uit hetzelfde regelwerk'],
  vult_in: ['invulling', 'invullingen', 'uit een andere regeling'],
  leest: ['uitkomst', 'uitkomsten', 'uit een andere regeling'],
  vraagt: ['vraag', 'vragen', 'aan een cel'],
  vraagt_reductie: ['antwoord', 'antwoorden', 'van een reductie'],
  kroniek: ['bron', 'bronnen', 'uit de kroniek'],
};
/** How long a sentence may be with the names in it: two lines above the picture. */
const LANG = 150;

const leesbaar = (naam) => naam.replaceAll('_', ' ');
const opsomming = (delen) => [delen.slice(0, -1).join(', '), ...delen.slice(-1)].filter(Boolean).join(' en ');

/** The question a name puts: what the law says it is, or else its name. */
function vraag({ naam, omschrijving }) {
  const tekst = (omschrijving ?? '').replace(/\s+/g, ' ').trim().replace(/\s*\([^()]*\)\.?$/, '').replace(/\.$/, '');
  const klein = tekst.charAt(0).toLowerCase() + tekst.slice(1);
  if (klein.startsWith('of ')) return `${klein}?`;
  return `wat is ${klein || leesbaar(naam)}?`;
}

/**
 * What step `n` of `schema` is, in one sentence, from the question: step 1
 * asks it, a later step says what the step before it needs and where that
 * comes from, the last of the path says that the chain is at its source, and
 * `reststap` that everything else works the same way, and where it ends up.
 * With null, the whole picture.
 */
export function stapZin({ bestanden, lijnen, stappen, reststap }, n) {
  if (!n) return `Het hele schema, in ${stappen} stappen.`;
  const kop = `Stap ${n} van ${stappen}`;
  const hier = new Map(
    bestanden.flatMap((b) => b.begrippen.filter((g) => g.stap === n).map((g) => [sleutel(b.pad, g.key), { ...g, soort: b.soort, titel: b.titel }])),
  );
  const kortOfLang = (zin) => (zin(true).length <= LANG ? zin(true) : zin(false));
  if (n === reststap) {
    const cellen = [...new Set([...hier.values()].filter((g) => g.soort === 'cel').map((g) => g.titel))];
    const via = cellen.length ? ` via ${opsomming(cellen)}` : '';
    const uitKroniek = lijnen.filter((l) => l.soort === 'kroniek' && hier.has(id(l.van)));
    const namen = [...new Set(uitKroniek.map((l) => leesbaar(hier.get(id(l.van)).naam)))];
    return kortOfLang((metNamen) => `${kop}, de andere invoer werkt op dezelfde manier en komt${via} bij de bron uit${metNamen && namen.length ? ` (${namen.join(', ')})` : ''}.`);
  }
  if (n === 1) return `${kop}, de vraag: ${vraag([...hier.values()][0])}`;

  const per = new Map();
  for (const l of lijnen) {
    const g = hier.get(id(l.van));
    if (!g || hier.has(id(l.naar))) continue;
    const k = l.soort === 'vraagt' && g.soort === 'reductie' ? 'vraagt_reductie' : l.soort;
    per.set(k, (per.get(k) ?? new Set()).add(leesbaar(g.naam)));
  }
  const laatste = n === (reststap ? reststap - 1 : stappen);
  return kortOfLang((metNamen) => {
    const delen = Object.entries(HERKOMST)
      .filter(([k]) => per.has(k))
      .map(([k, [een, veel, waar]]) => {
        const namen = [...per.get(k)];
        const deel = `${namen.length === 1 ? `een ${een}` : `${namen.length} ${veel}`} ${waar}`;
        return metNamen ? `${deel} (${namen.join(', ')})` : deel;
      });
    return `${kop}, ${laatste ? 'hier komt de keten bij de bron uit' : 'daarvoor is nodig'}: ${opsomming(delen)}.`;
  });
}
