/**
 * A record as a message to the citizen, by the message spec of the source
 * (`berichten.rs` on the server): a decision, or a submission or a fact the
 * authority writes about. The spec binds a kind of message to a record, gives
 * its parts in the places of a message, and says per key of the text where
 * its value comes from; the text itself comes from the text layer of the
 * authority. So no text stands loose from the record: a key without a value
 * shows as missing, and the invariants of the spec are checked on the
 * message as it stands. Every violation is listed (`schendingen`); a screen
 * shows them, never drops them.
 *
 * Without a spec, a decision gets the parts the Awb requires, from the law
 * alone ([`STANDAARDSOORT`]); another record gets no message.
 */
import { longDate } from './datum';
import { kaalSchermveld, stukWaarde } from './formulier';
import { artikelNaam, isBesluit, rolVan } from './keten';
import type {
  Alinea,
  Berichtdeel,
  Berichtenspec,
  Berichtplek,
  Berichtsoort,
  Gram,
  Kanaalcode,
  Rol,
  Sjabloon,
  Stand,
  Sleutelbron,
  Tekstlaag,
  Uitvoeruitleg,
  Vormen,
  Waarom,
} from './types';

/** One step of the chain of a decision, as `waarom` gives it. */
type Stap = NonNullable<Waarom['keten']>[number];

/** `tekst` with a capital. */
const hoofdletter = (tekst: string): string => tekst.charAt(0).toUpperCase() + tekst.slice(1);

/** The kind of a record in words: `AANVRAAG` as `aanvraag`. */
const soortInWoorden = (kind: string): string => kind.toLowerCase().replace(/_/g, ' ');

/** The places of a message in their order (`berichten::PLEKKEN`): the order of its parts. */
export const PLEKKEN: Berichtplek[] = [
  'kop',
  'inleiding',
  'dictum',
  'grondslag',
  'motivering',
  'onderbouwing',
  'specificatie',
  'mededeling',
  'ondertekening',
  'rechtsmiddelen',
  'bijlage',
];

/** The heading a part has when neither the template nor the spec gives one. */
const KOPPEN: Record<Berichtplek, string> = {
  kop: '',
  inleiding: '',
  dictum: 'Ons besluit',
  grondslag: 'Grondslag',
  motivering: 'Waarom wij zo besluiten',
  onderbouwing: 'Waar wij van uitgingen',
  specificatie: 'Specificatie',
  mededeling: '',
  ondertekening: '',
  rechtsmiddelen: 'Bent u het niet eens met dit besluit?',
  bijlage: 'Bijlage',
};

/** The channels a message goes out by, with the articles that govern them (`berichten::KANALEN`). */
export const KANALEN: Record<Kanaalcode, { naam: string; grond: string[] }> = {
  email: { naam: 'Per e-mail', grond: ['Awb 2:8'] },
  portaal: { naam: 'In het portaal, met een kennisgeving', grond: ['Awb 2:8', 'Awb 2:10'] },
  papier: { naam: 'Op papier', grond: ['Awb 3:41'] },
};

/**
 * The kind of message a decision has when the source has no spec: the parts
 * the Awb requires of a decision (the pattern `beschikking`), each filled
 * from the law.
 */
export const STANDAARDSOORT: Berichtsoort = {
  id: 'besluit',
  naam: 'besluit',
  patroon: 'beschikking',
  vastlegging: { rol: 'besluit', artikel: '' },
  delen: [
    { id: 'kop', plek: 'kop' },
    { id: 'grondslag', plek: 'grondslag', vereist_door: 'Awb 3:47', bron: { wet: 'grondslag' } },
    { id: 'motivering', plek: 'motivering', vereist_door: 'Awb 3:46', bron: { wet: 'keten' } },
    { id: 'ondertekening', plek: 'ondertekening', bron: { wet: 'ondertekening' } },
    { id: 'rechtsmiddelen', plek: 'rechtsmiddelen', vereist_door: 'Awb 3:45', bron: { wet: 'rechtsmiddelen' } },
  ],
};

/** A paragraph as the message shows it. */
export interface Berichtalinea {
  kop?: string;
  tekst: string;
  punten: string[];
}

/** A field and its value. */
export interface Berichtregel {
  naam: string;
  label: string;
  waarde: string;
  toelichting?: string;
}

/** A row of a specification: what it is and its amount. */
export interface Berichtrij {
  label: string;
  waarde: string;
  bedrag: number | null;
}

/** A cell of an annex: its text, and a second line under it. */
export interface Bijlagecel {
  tekst: string;
  onder?: string;
}

/** A row of an annex: its cells and its amount. */
export interface Bijlagerij {
  cellen: Bijlagecel[];
  bedrag: number | null;
}

/** A group of rows of an annex, such as the rows under one register, with their sum. */
export interface Bijlagegroep {
  titel?: string;
  rijen: Bijlagerij[];
  /** The sum of the amounts of the rows, with its label. */
  subtotaal: { label: string; waarde: string; bedrag: number } | null;
}

/** An annex in columns as a screen shows it (`BerichtTabel.vue`). */
export interface Berichtbijlage {
  titel?: string;
  kolommen: string[];
  groepen: Bijlagegroep[];
  totaal: { label: string; waarde: string; bedrag: number } | null;
}

/** One part of the message as a screen shows it. */
export interface Berichtonderdeel {
  id: string;
  plek: Berichtplek;
  titel: string;
  /** The element of the design system that shows it. */
  nldd: string;
  alineas: Berichtalinea[];
  regels: Berichtregel[];
  /** A short list, such as the articles the decision rests on. */
  opsomming: string[];
  /** A specification: its rows and its total. */
  tabel: { rijen: Berichtrij[]; totaal: Berichtrij | null } | null;
  /** An annex in columns, grouped, with sums (`bron.bijlage`); its rows also stand in `tabel` for the invariants. */
  bijlage: Berichtbijlage | null;
  /** The article that requires the part. */
  vereist_door?: string;
  /** Set on a part the law requires that has nothing to say. */
  ontbreekt: boolean;
}

/** Where the message stands: a concept before the decision, laid down, or made known. */
export type Berichtstatus = 'concept' | 'vastgesteld' | 'bekendgemaakt';

/** A violation of the template: what does not hold, and on what. */
export interface Schending {
  soort: 'ontbreekt' | 'sleutel_zonder_waarde' | 'sleutel_zonder_bron' | 'bedrag_is_totaal' | 'oordeel_past' | 'rechtskarakter' | 'kanaal' | 'spec';
  tekst: string;
}

/** The message as a screen shows it. */
export interface Berichtmodel {
  soort: string;
  naam: string;
  onderwerp: string;
  afzender: string;
  ondertekenaar?: string;
  geadresseerde: string[];
  datum: string;
  kenmerk: string;
  aanhef?: string;
  slot?: string;
  status: Berichtstatus;
  kanaal: { code: Kanaalcode | 'onbekend'; naam: string; grond: string[] } | null;
  onderdelen: Berichtonderdeel[];
  schendingen: Schending[];
  /** The text in the band on top of the letter: the sender, never a logo (`briefhoofd.lint`). */
  lint?: string;
  /** The return address over the address block (`briefhoofd.retouradres`). */
  retouradres?: string;
  /** The lines of the footer (`briefhoofd.voettekst`). */
  voettekst?: string[];
  /** The day of the submission the message answers ("Uw brief van"). */
  uwBriefVan?: string;
  /** Set on a message read from the fact of sending: when it went, the hash, what it replaced, the fact. */
  verzonden?: { op: string; hash: string; vervangt: string | null; feit: string };
}

/** A submission the law allows against a decision, with the cell that decides on it. */
export interface Rechtsmiddel {
  soort: string;
  cel_naam: string;
}

/** What a message is made from. */
export interface Berichtbronnen {
  gram: Gram;
  rol: Rol | undefined;
  vormen: Vormen | null;
  /** The records the reader may see, to find the submission the record answers. */
  grammen?: Gram[];
  /** Why the decision came out so (`waarom`), with its chain. */
  waarom?: Waarom | null;
  /** What a submission gives under its own article (`uitkomsten`). */
  uitkomsten?: Record<string, unknown> | null;
  /** The name of the cell that wrote the record. */
  afzender: string;
  /** The name of a cell, for the remedy the forms give. */
  celNaam?: (cel: string) => string;
  /** A preview: nothing recorded yet. */
  concept?: boolean;
}

/**
 * The kind of message of `gram` in `spec`: the first whose record is the
 * record's role and article, and for a fact its fact. A decision with none
 * gets [`STANDAARDSOORT`]; another record with none has no message.
 */
export function berichtsoortVoor(gram: Gram, rol: Rol | undefined, spec?: Berichtenspec | null): Berichtsoort | undefined {
  /** The condition of a binding (`vastlegging.als`) on the record itself: an outcome or a field, with one test. */
  const geldtVoor = (als: Sleutelbron | undefined): boolean => {
    if (!als) return true;
    const v = als.uitkomst ? gram.outputs?.[als.uitkomst] : als.veld ? (gram.fields?.[als.veld] ?? gram.findings?.[als.veld]) : undefined;
    if ('is' in als) return v === als.is;
    if ('gelijk' in als) return v === als.gelijk;
    if (als.groter_dan !== undefined) return typeof v === 'number' && v > als.groter_dan;
    if (als.gevuld) return zegtIets(v);
    if (als.niet_gevuld) return !zegtIets(v);
    return v === true;
  };
  const past = spec?.berichtsoorten.find(
    (s) =>
      s.vastlegging.rol === rol &&
      s.vastlegging.artikel === gram.basis &&
      (rol !== 'vastlegging' || s.vastlegging.feit === `${gram.basis}#${gram.fact ?? ''}`) &&
      geldtVoor(s.vastlegging.als),
  );
  return past ?? (rol === 'besluit' ? STANDAARDSOORT : undefined);
}

/**
 * The records `gram` answers or is about, nearest first: what it refers to
 * or received, or the submission a fact names (`aanvraag`), up the chain.
 */
function voorgangers(gram: Gram, grammen: Gram[]): Gram[] {
  const uit: Gram[] = [];
  const gezien = new Set<string>([gram.id]);
  let g: Gram | undefined = gram;
  while (g) {
    const ouder: unknown = g.received?.id ?? g.refers_to ?? g.fields?.aanvraag;
    const volgende: Gram | undefined = typeof ouder === 'string' ? grammen.find((x) => x.id === ouder) : undefined;
    if (!volgende || gezien.has(volgende.id)) break;
    gezien.add(volgende.id);
    uit.push(volgende);
    g = volgende;
  }
  return uit;
}

/** What the law says of each output, over every form: decisions and submissions alike. */
function uitlegVan(vormen: Vormen | null): Record<string, Uitvoeruitleg> {
  const uit: Record<string, Uitvoeruitleg> = {};
  for (const v of Object.values(vormen?.indieningen ?? {})) Object.assign(uit, v.uitleg ?? {});
  for (const v of Object.values(vormen?.besluiten ?? {})) Object.assign(uit, v.uitleg ?? {});
  return uit;
}

/** An amount in eurocent as a message writes it: `€ 19.136,00`. */
export const euro = (cent: number): string =>
  new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' }).format(cent / 100);

/** Whether `v` says something: not empty, and not a value the engine could not determine. */
const zegtIets = (v: unknown): boolean =>
  v !== null && v !== undefined && v !== '' && !(typeof v === 'object' && v !== null && '__unknown' in v);

/** An outcome in the chain of a decision, by its name: the first, depth first. */
function uitKeten(keten: Stap[] | undefined, naam: string): unknown {
  for (const s of keten ?? []) {
    if (s.soort === 'uitkomst' && s.naam === naam) return s.waarde;
    const dieper = uitKeten(s.stappen, naam);
    if (dieper !== undefined) return dieper;
  }
  return undefined;
}

/**
 * What the law allows against the decision `gram`, from the forms of the
 * process, in the order the Awb sets: against a decision on an application,
 * the submission the same cell decides on (an objection); against that
 * cell's decision on such a submission, what another cell decides on (an
 * appeal). With the last day, where the decision gives the field the form
 * reads as its deadline.
 */
export function rechtsmiddelenVan(
  gram: Gram,
  vormen: Vormen | null,
  celNaam: (cel: string) => string = (c) => c,
): { middelen: Rechtsmiddel[]; termijn: { einde: string } | null } {
  const alle = Object.values(vormen?.indieningen ?? {});
  const aanvraag = (v: { soort: string }) => v.soort === 'AANVRAAG';
  const tegenBesluit = alle.filter((v) => !aanvraag(v));
  const hier = tegenBesluit.filter((v) => v.cel === gram.recording_actor);
  const opAanvraag = alle.some((v) => aanvraag(v) && v.besluit === gram.basis);
  // Only a decision of the executor, the cell whose own forms these are, is contested further.
  const opBezwaar = !opAanvraag && gram.recording_actor === vormen?.eigen?.cel && hier.some((v) => v.besluit === gram.basis);
  const gekozen = opAanvraag ? hier : opBezwaar ? tegenBesluit.filter((v) => v.cel !== gram.recording_actor) : [];
  const einde = gekozen.map((v) => (v.termijn ? gram.outputs?.[v.termijn] : undefined)).find((x) => typeof x === 'string');
  return {
    middelen: gekozen.map((v) => ({ soort: hoofdletter(v.soort.toLowerCase()), cel_naam: celNaam(v.cel) })),
    termijn: typeof einde === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(einde) ? { einde } : null,
  };
}

/** The articles the chain of a decision ran, in words and each once: what the decision rests on. */
function artikelenUitKeten(keten: Stap[] = []): string[] {
  const uit: string[] = [];
  const loop = (stappen: Stap[]) => {
    for (const s of stappen) {
      if ((s.soort === 'artikel' || s.soort === 'hook') && s.wet && s.artikel && !s.uitvoer) {
        const naam = artikelNaam(`${s.wet}#${s.artikel}`);
        if (!uit.includes(naam)) uit.push(naam);
      }
      loop(s.stappen ?? []);
    }
  };
  loop(keten);
  return uit;
}

/** The template of a kind of message in the text layer. */
const sjabloonVan = (soort: Berichtsoort, laag: Tekstlaag | undefined): Sjabloon | undefined =>
  soort.sjabloon ? laag?.sjablonen?.find((s) => s.id === soort.sjabloon) : undefined;

/**
 * The message of the record `gram` by the kind of message `soort`: per part
 * what its sources give, with every key of its text filled from the record,
 * and the violations of the template.
 */
export function maakBericht(soort: Berichtsoort, b: Berichtbronnen, spec?: Berichtenspec | null): Berichtmodel {
  const { gram, vormen } = b;
  const laag = spec?.tekstlaag;
  const sjabloon = sjabloonVan(soort, laag);
  const grammen = b.grammen ?? [];
  const eerder = voorgangers(gram, grammen);
  // The submission the record answers: the nearest record before it that is a submission of the forms.
  const indieningen = Object.values(vormen?.indieningen ?? {});
  const aanvraag = eerder.find((g) => indieningen.some((v) => v.basis === g.basis && v.soort === g.kind)) ?? eerder.at(-1);
  const uitleg = uitlegVan(vormen);
  const schendingen: Schending[] = [];
  const meld = (s: Schending) => {
    if (!schendingen.some((x) => x.soort === s.soort && x.tekst === s.tekst)) schendingen.push(s);
  };
  for (const m of spec?.meldingen ?? []) if (m.startsWith(`${soort.id}:`)) meld({ soort: 'spec', tekst: m });
  if (soort.sjabloon && !sjabloon) meld({ soort: 'spec', tekst: `Het sjabloon '${soort.sjabloon}' staat niet in de tekstlaag.` });

  /** The raw value of a source. */
  const ruw = (bron: Sleutelbron): unknown => {
    if (bron.uitkomst) return gram.outputs?.[bron.uitkomst];
    if (bron.veld) {
      const waar = bron.eigen ? [gram.fields, gram.findings] : [gram.fields, gram.findings, ...eerder.map((g) => g.fields)];
      return waar.map((x) => x?.[bron.veld!]).find(zegtIets);
    }
    if (bron.keten) return uitKeten(b.waarom?.keten, bron.keten);
    if (bron.indiening) return b.uitkomsten?.[bron.indiening];
    if (bron.record === 'recorded_on') return gram.recorded_on;
    if (bron.record === 'id') return gram.id;
    if (bron.record === 'kenmerk') return gram.id.slice(0, 8).toUpperCase();
    if (bron.label) return kaalSchermveld(bron.label, vormen?.teksten).label;
    if (bron.afzender) {
      const a = laag?.afzender;
      if (bron.afzender === 'organisatie') return a?.organisatie;
      if (bron.afzender === 'namens') return a?.namens;
      return a?.rollen?.[bron.afzender];
    }
    if (bron.zin) return zin(bron.zin);
    return undefined;
  };
  /** Whether a source with a test holds; a source without one, whether it is true. */
  const toets = (bron: Sleutelbron): boolean => {
    if (bron.alle) return bron.alle.every(toets);
    if (bron.een_van) return bron.een_van.some(toets);
    if (bron.niet) return !toets(bron.niet);
    const v = bron.sleutel ? sleutelRuw(bron.sleutel) : ruw(bron);
    if ('is' in bron) return v === bron.is;
    if ('gelijk' in bron) return v === bron.gelijk;
    if (bron.groter_dan !== undefined) return typeof v === 'number' && v > bron.groter_dan;
    if (bron.gevuld) return zegtIets(v);
    if (bron.niet_gevuld) return !zegtIets(v);
    return v === true;
  };
  const isVoorwaarde = (bron: Sleutelbron) =>
    ['is', 'gelijk', 'groter_dan', 'gevuld', 'niet_gevuld', 'alle', 'een_van', 'niet'].some((k) => k in bron);
  /** The value of a key: a condition gives true or false, another source its value. */
  const sleutelRuw = (naam: string): unknown => {
    const bron = soort.sleutels?.[naam];
    if (!bron) return undefined;
    return isVoorwaarde(bron) ? toets(bron) : ruw(bron);
  };
  /** A value as a message writes it: by the form the spec gives, or by what the law says of it. */
  const inWoorden = (naam: string, v: unknown, vorm?: Sleutelbron['vorm']): string => {
    const u = uitleg[naam];
    if (typeof v === 'number' && (vorm === 'bedrag' || u?.eenheid === 'eurocent' || (u?.type === 'amount' && !u?.eenheid))) return euro(v);
    if (typeof v === 'string' && (vorm === 'datum' || /^\d{4}-\d{2}-\d{2}$/.test(v))) return longDate(v);
    if (Array.isArray(v)) {
      return v
        .map((rij) =>
          rij && typeof rij === 'object'
            ? Object.values(rij as Record<string, unknown>)
                .filter(zegtIets)
                .map((x) => String(x))
                .join(', ')
            : String(rij),
        )
        .join('; ');
    }
    if (vorm === 'tekst' && typeof v === 'string') return v;
    return stukWaarde(naam, v, vormen?.teksten);
  };
  const bronNaam = (bron: Sleutelbron): string =>
    bron.uitkomst ?? bron.veld ?? bron.keten ?? bron.indiening ?? bron.label ?? bron.zin ?? '';
  /** A key in a text: its value in words, or a visible gap and a violation. */
  const vul = (tekst: string, waar: string): string =>
    tekst.replace(/\{([A-Za-z0-9_]+)\}/g, (_, naam: string) => {
      const bron = soort.sleutels?.[naam];
      if (!bron) {
        meld({ soort: 'sleutel_zonder_bron', tekst: `${waar} noemt {${naam}}, maar de berichtenspec zegt niet waar die vandaan komt.` });
        return `[${naam}: geen bron]`;
      }
      const v = sleutelRuw(naam);
      if (!zegtIets(v)) {
        if (bron.zin) return '';
        meld({ soort: 'sleutel_zonder_waarde', tekst: `${waar} noemt {${naam}}, maar het stuk geeft geen waarde.` });
        return `[${naam} ontbreekt]`;
      }
      return bron.zin || bron.afzender || bron.label ? String(v) : inWoorden(bronNaam(bron), v, bron.vorm);
    });
  /** A sentence of the text layer: empty when one of its keys has no value. */
  const zin = (id: string): string | undefined => {
    const tekst = laag?.zinnen?.[id];
    if (!tekst) return undefined;
    const sleutels = [...tekst.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((m) => m[1]!);
    if (sleutels.some((s) => !zegtIets(sleutelRuw(s)))) return '';
    return vul(tekst, `De zin '${id}'`);
  };
  /** Whether a condition by key name holds; an unknown key is a violation, not a silent no. */
  const geldt = (naam: string | undefined, waar: string): boolean => {
    if (!naam) return true;
    if (!soort.sleutels?.[naam]) {
      meld({ soort: 'sleutel_zonder_bron', tekst: `${waar} staat er als '${naam}', maar de berichtenspec zegt niet waar dat vandaan komt.` });
      return false;
    }
    // A key that is a condition holds when true; a key that is a value, when it has one.
    const v = sleutelRuw(naam);
    return isVoorwaarde(soort.sleutels[naam]!) ? v === true : zegtIets(v) && v !== false;
  };

  const alineas = sjabloon?.alineas ?? [];
  const alineaNaam = (a: Alinea, i: number) => (a.kop ? `De alinea '${a.kop}'` : `Alinea ${i + 1}`);
  const besluit = b.rol === 'besluit';
  const delen = PLEKKEN.flatMap((plek) => soort.delen.filter((d) => d.plek === plek));

  const kopdeel = delen.find((d) => d.plek === 'kop');
  const geadresseerde = (kopdeel?.bron?.regels ?? ['naam_aanvrager', 'adres_aanvrager', 'naam_indiener', 'adres_indiener'])
    .map((n) => ruw({ veld: n }))
    .filter((v): v is string => typeof v === 'string' && v !== '');

  const onderdeel = (d: Berichtdeel): Berichtonderdeel => {
    const bron = d.bron ?? {};
    const tekst: Berichtalinea[] = [];
    alineas.forEach((a, i) => {
      const gekozen = (bron.tekst ?? []).some((t) => (typeof t === 'number' ? t === i : t === (a.kop ?? '')));
      if (!gekozen || !geldt(a.als, alineaNaam(a, i))) return;
      const waar = alineaNaam(a, i);
      tekst.push({ kop: a.kop, tekst: vul(a.tekst ?? '', waar), punten: (a.punten ?? []).map((p) => vul(p, waar)) });
    });
    const regels: Berichtregel[] = (bron.regels ?? []).flatMap((n) => {
      const v = [ruw({ veld: n }), gram.outputs?.[n], b.uitkomsten?.[n]].find(zegtIets);
      if (!zegtIets(v)) return [];
      const toelichting = uitleg[n]?.beschrijving ?? undefined;
      return [{ naam: n, label: kaalSchermveld(n, vormen?.teksten).label, waarde: inWoorden(n, v), toelichting: toelichting || undefined }];
    });
    const opsomming: string[] = [];
    const eigen: Berichtalinea[] = [];
    if (bron.wet === 'grondslag' && besluit) opsomming.push(artikelNaam(gram.basis));
    if (bron.wet === 'keten' && besluit) opsomming.push(...artikelenUitKeten(b.waarom?.keten));
    if (bron.wet === 'rechtsmiddelen' && besluit && !tekst.length) {
      const { middelen, termijn } = rechtsmiddelenVan(gram, vormen, b.celNaam);
      const wat = middelen.map((r) => `${r.soort.toLowerCase()} indienen bij ${r.cel_naam}`);
      if (wat.length) {
        eigen.push({
          tekst: termijn ? `U kunt tot en met ${longDate(termijn.einde)} ${wat.join(' of ')}.` : `U kunt ${wat.join(' of ')}.`,
          punten: [],
        });
      }
    }
    let tabel: Berichtonderdeel['tabel'] = null;
    if (bron.tabel) {
      const rij = (naam: string): Berichtrij => {
        const v = sleutelRuw(naam);
        const bedrag = typeof v === 'number' ? v : null;
        if (!zegtIets(v)) meld({ soort: 'sleutel_zonder_waarde', tekst: `De specificatie noemt '${naam}', maar het stuk geeft geen waarde.` });
        const s = soort.sleutels?.[naam];
        return { label: hoofdletter(naam.replace(/_/g, ' ')), waarde: zegtIets(v) ? inWoorden(s ? bronNaam(s) : naam, v, s?.vorm) : '[ontbreekt]', bedrag };
      };
      tabel = { rijen: bron.tabel.rijen.map(rij), totaal: bron.tabel.totaal ? rij(bron.tabel.totaal) : null };
    }
    let bijlage: Berichtbijlage | null = null;
    if (bron.bijlage) {
      const bb = bron.bijlage;
      const labels = laag?.bijlagen?.[bb.labels] ?? {};
      const label = (k: string) => {
        const t = labels[k];
        if (t === undefined) meld({ soort: 'sleutel_zonder_bron', tekst: `De bijlage noemt de tekst '${k}', maar de tekstlaag heeft haar niet.` });
        return t ?? `[${k}]`;
      };
      const getal = (n: number) => new Intl.NumberFormat('nl-NL').format(n);
      /** A label with the fields of `rij` in braces; undefined when one of them has no value. */
      const vulRij = (k: string, rij: Record<string, unknown>): string | undefined => {
        let leeg = false;
        const t = label(k).replace(/\{([A-Za-z0-9_]+)\}/g, (_, v: string) => {
          const w = rij[v];
          if (!zegtIets(w)) {
            leeg = true;
            return '';
          }
          if (typeof w === 'number') return (bb.bedragen ?? []).includes(v) ? euro(w) : getal(w);
          return String(w).replace(/_/g, ' ');
        });
        return leeg ? undefined : t;
      };
      /** The rows: the value of a key that is a list of rows, or rows the spec builds from keys (`velden`, under `als`). */
      let waarde: unknown;
      if (typeof bb.rijen === 'string') {
        waarde = sleutelRuw(bb.rijen);
      } else {
        waarde = bb.rijen
          .filter((r) => geldt(r.als, 'Een regel van de bijlage'))
          .map((r) =>
            Object.fromEntries(
              Object.entries(r.velden).map(([veld, sleutel]) => {
                const v = sleutelRuw(sleutel);
                const s = soort.sleutels?.[sleutel];
                return [veld, typeof v === 'number' ? v : zegtIets(v) ? inWoorden(s ? bronNaam(s) : sleutel, v, s?.vorm) : null];
              }),
            ),
          );
      }
      if (!Array.isArray(waarde)) {
        meld({ soort: 'sleutel_zonder_waarde', tekst: `De bijlage leest '${String(bb.rijen)}', maar het stuk geeft geen rijen.` });
      } else if (waarde.length) {
        const rijen = waarde as Record<string, unknown>[];
        const groepen: Bijlagegroep[] = [];
        for (const r of rijen) {
          const titel = bb.groep ? vulRij('groep', r) : undefined;
          let g = groepen.find((x) => x.titel === titel);
          if (!g) {
            g = { titel, rijen: [], subtotaal: null };
            groepen.push(g);
          }
          const bedrag = bb.bedrag && typeof r[bb.bedrag] === 'number' ? (r[bb.bedrag] as number) : null;
          g.rijen.push({
            cellen: bb.kolommen.map((k) => ({
              tekst: vulRij(k.tekst, r) ?? '',
              onder: k.onder && (!k.onder_als || Number(r[k.onder_als.veld] ?? 0) > k.onder_als.groter_dan) ? vulRij(k.onder, r) : undefined,
            })),
            bedrag,
          });
        }
        const alle = groepen.flatMap((g) => g.rijen);
        bijlage = { titel: labels.titel, kolommen: bb.kolommen.map((k) => label(k.kop)), groepen, totaal: null };
        if (bb.bedrag) {
          // The amounts are the model's, per row. A sum per group is the sum of its rows, shown as such; the total
          // is the model's own (`totaal`) when the spec names it. What must hold: the rows of a group make its sum,
          // the sums of the groups make the total, and the total is the amount of the decision (bedrag_is_totaal).
          const som = (rs: Bijlagerij[]) => rs.reduce((a, x) => a + (x.bedrag ?? 0), 0);
          for (const g of groepen) {
            if (bb.groep && labels.samen !== undefined) {
              const t = som(g.rijen);
              g.subtotaal = { label: label('samen').replace('{groep}', g.titel ?? ''), waarde: euro(t), bedrag: t };
            }
          }
          const sommen = groepen.reduce((a, g) => a + (g.subtotaal?.bedrag ?? som(g.rijen)), 0);
          const model = bb.totaal ? sleutelRuw(bb.totaal) : undefined;
          const t = typeof model === 'number' ? model : sommen;
          bijlage.totaal = { label: label('totaal'), waarde: euro(t), bedrag: t };
          if (alle.some((x) => x.bedrag === null)) {
            meld({ soort: 'sleutel_zonder_waarde', tekst: `Een regel van de bijlage heeft geen bedrag ('${bb.bedrag}').` });
          }
          if (bb.totaal && typeof model !== 'number') {
            meld({ soort: 'sleutel_zonder_waarde', tekst: `Het totaal van de bijlage ('${bb.totaal}') heeft geen bedrag.` });
          } else if (sommen !== t) {
            meld({
              soort: 'bedrag_is_totaal',
              tekst: `De regels van de bijlage tellen op tot ${euro(sommen)}, maar het totaal is ${euro(t)}.`,
            });
          }
          // For the invariant bedrag_is_totaal: the rows as one list, with the total.
          tabel = {
            rijen: alle.map((x) => ({ label: x.cellen[0]?.tekst ?? '', waarde: x.bedrag === null ? '[ontbreekt]' : euro(x.bedrag), bedrag: x.bedrag })),
            totaal: { label: label('totaal'), waarde: euro(t), bedrag: t },
          };
        }
      }
    }
    const leeg = !tekst.length && !eigen.length && !regels.length && !opsomming.length && !tabel && !bijlage;
    const ontbreekt = leeg && !!d.vereist_door && d.plek !== 'ondertekening';
    if (ontbreekt) {
      meld({ soort: 'ontbreekt', tekst: `Het deel '${d.titel ?? (KOPPEN[d.plek] || d.id)}' is leeg, en ${d.vereist_door} eist het.` });
    }
    return {
      id: d.id,
      plek: d.plek,
      titel: d.titel ?? tekst[0]?.kop ?? KOPPEN[d.plek],
      nldd: d.nldd ?? (d.plek === 'dictum' || d.plek === 'rechtsmiddelen' ? 'nldd-box' : bron.tabel ? 'nldd-table' : 'nldd-rich-text'),
      alineas: [...tekst, ...eigen],
      regels,
      opsomming,
      tabel,
      bijlage,
      vereist_door: d.vereist_door,
      ontbreekt,
    };
  };

  // A part under a condition stands only when it holds; the place of the signature has its own content.
  const staat = (d: Berichtdeel) => geldt(d.als, `Het deel '${d.id}'`);
  const onderdelen = delen
    .filter((d) => d.plek !== 'kop' && staat(d))
    .map(onderdeel)
    .filter((o) => o.plek === 'ondertekening' || o.ontbreekt || o.alineas.length + o.regels.length + o.opsomming.length > 0 || o.tabel || o.bijlage);

  // Every paragraph of the template stands in a part, or the spec said so (`meldingen`).
  // The signature: the closing, the organisation, under mandate on whose behalf (Awb 10:10), and the role.
  const ondertekening = onderdelen.find((o) => o.plek === 'ondertekening');
  const rol = sjabloon?.ondertekenaar ? laag?.afzender?.rollen?.[sjabloon.ondertekenaar] : undefined;
  if (ondertekening) {
    const organisatie = laag?.afzender?.organisatie ?? b.afzender;
    const regels = [organisatie, ...(laag?.afzender?.in_mandaat && laag.afzender.namens ? [laag.afzender.namens] : []), ...(rol ? [rol] : [])];
    ondertekening.alineas = [...ondertekening.alineas, ...regels.map((r) => ({ tekst: r, punten: [] }))];
    ondertekening.regels = [];
  }

  // Invariants.
  if (soort.vastlegging.rechtskarakter && besluit && gram.kind !== soort.vastlegging.rechtskarakter) {
    meld({
      soort: 'rechtskarakter',
      tekst: `Het stuk is een ${soort.vastlegging.rechtskarakter.toLowerCase()} volgens de berichtenspec, maar de vastlegging is een ${gram.kind.toLowerCase()}.`,
    });
  }
  for (const inv of soort.invarianten ?? []) {
    if (inv.soort === 'bedrag_is_totaal' && inv.bedrag && inv.specificatie) {
      const spec = onderdelen.find((o) => o.id === inv.specificatie);
      if (!spec?.tabel) continue;
      const bedrag = sleutelRuw(inv.bedrag);
      const rijen = spec.tabel.rijen.map((r) => r.bedrag);
      const som = rijen.every((r) => r !== null) ? (rijen as number[]).reduce((a, r) => a + r, 0) : null;
      const totaal = spec.tabel.totaal?.bedrag ?? som;
      if (som !== null && spec.tabel.totaal && totaal !== som && !spec.bijlage) {
        meld({ soort: 'bedrag_is_totaal', tekst: `Het totaal van de specificatie (${euro(totaal ?? 0)}) is niet de som van haar regels (${euro(som)}).` });
      }
      if (typeof bedrag !== 'number' || totaal === null || bedrag !== totaal) {
        meld({
          soort: 'bedrag_is_totaal',
          tekst: `Het bedrag in het besluit (${typeof bedrag === 'number' ? euro(bedrag) : 'geen bedrag'}) is niet het totaal van de specificatie (${totaal === null ? 'geen totaal' : euro(totaal)}).`,
        });
      }
    }
    if (inv.soort === 'oordeel_past' && inv.oordelen) {
      const gelden = Object.keys(inv.oordelen).filter((o) => sleutelRuw(o) === true);
      if (gelden.length !== 1) {
        meld({
          soort: 'oordeel_past',
          tekst: gelden.length
            ? `Het bericht geeft meer dan één oordeel tegelijk: ${gelden.join(' en ')}.`
            : `Het bericht geeft geen oordeel: geen van ${Object.keys(inv.oordelen).join(', ')} geldt.`,
        });
      }
      for (const o of gelden) {
        if (!toets(inv.oordelen[o]!)) {
          meld({ soort: 'oordeel_past', tekst: `Het oordeel '${o}' past niet bij de uitkomst van het stuk.` });
        }
      }
    }
  }

  // Status and channel.
  const status: Berichtstatus = b.concept ? 'concept' : soort.status && toets(soort.status.bekendgemaakt) ? 'bekendgemaakt' : 'vastgesteld';
  let kanaal: Berichtmodel['kanaal'] = null;
  if (soort.kanaal) {
    const code = soort.kanaal.vast ?? (soort.kanaal.sleutel ? soort.kanaal.waarden?.[String(sleutelRuw(soort.kanaal.sleutel))] : undefined);
    if (code && KANALEN[code]) kanaal = { code, ...KANALEN[code] };
    else {
      kanaal = { code: 'onbekend', naam: 'Kanaal onbekend', grond: [] };
      meld({ soort: 'kanaal', tekst: 'Het bericht zegt niet langs welke weg het de burger bereikt.' });
    }
  }

  const naam = soort.naam ?? sjabloon?.titel ?? soort.id;
  const betreftBij = sjabloon?.betreft_bij ? Object.entries(sjabloon.betreft_bij).find(([k]) => geldt(k, 'Het onderwerp')) : undefined;
  const betreft = betreftBij?.[1] ?? sjabloon?.betreft;
  const over = aanvraag ? ` op uw ${soortInWoorden(aanvraag.kind)} van ${longDate(aanvraag.recorded_on)}` : '';
  const onderwerp = betreft ? vul(betreft, 'Het onderwerp') : `${hoofdletter(naam)}${over}`;
  return {
    soort: soort.id,
    naam,
    onderwerp: b.concept ? `Concept: ${onderwerp}` : onderwerp,
    afzender: laag?.afzender?.organisatie ?? b.afzender,
    ondertekenaar: rol,
    geadresseerde,
    datum: longDate(gram.recorded_on),
    kenmerk: gram.id.slice(0, 8).toUpperCase(),
    aanhef: sjabloon?.aanhef,
    slot: sjabloon?.slot,
    status,
    kanaal,
    onderdelen,
    schendingen,
    lint: laag?.briefhoofd?.lint ?? laag?.afzender?.organisatie ?? b.afzender,
    retouradres: laag?.briefhoofd?.retouradres,
    voettekst: laag?.briefhoofd?.voettekst,
    uwBriefVan: aanvraag ? longDate(aanvraag.recorded_on) : undefined,
  };
}

/** SHA-256 of `tekst` (UTF-8), in hexadecimal: the hash of the frozen view of a message. */
export function sha256(tekst: string): string {
  const k = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01,
    0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08,
    0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  const h = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
  const bytes = Array.from(new TextEncoder().encode(tekst));
  const lengte = bytes.length * 8;
  bytes.push(0x80);
  while (bytes.length % 64 !== 56) bytes.push(0);
  for (let i = 7; i >= 0; i--) bytes.push(Math.floor(lengte / 2 ** (8 * i)) & 0xff);
  const r = (x: number, n: number) => (x >>> n) | (x << (32 - n));
  const w = new Array<number>(64).fill(0);
  for (let o = 0; o < bytes.length; o += 64) {
    for (let i = 0; i < 16; i++) {
      w[i] = (bytes[o + 4 * i]! << 24) | (bytes[o + 4 * i + 1]! << 16) | (bytes[o + 4 * i + 2]! << 8) | bytes[o + 4 * i + 3]!;
    }
    for (let i = 16; i < 64; i++) {
      const s0 = r(w[i - 15]!, 7) ^ r(w[i - 15]!, 18) ^ (w[i - 15]! >>> 3);
      const s1 = r(w[i - 2]!, 17) ^ r(w[i - 2]!, 19) ^ (w[i - 2]! >>> 10);
      w[i] = (w[i - 16]! + s0 + w[i - 7]! + s1) | 0;
    }
    let [a, b, c, d, e, f, g, hh] = h as [number, number, number, number, number, number, number, number];
    for (let i = 0; i < 64; i++) {
      const t1 = (hh + (r(e, 6) ^ r(e, 11) ^ r(e, 25)) + ((e & f) ^ (~e & g)) + k[i]! + w[i]!) | 0;
      const t2 = ((r(a, 2) ^ r(a, 13) ^ r(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) | 0;
      hh = g;
      g = f;
      f = e;
      e = (d + t1) | 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) | 0;
    }
    [a, b, c, d, e, f, g, hh].forEach((x, i) => (h[i] = (h[i]! + x) | 0));
  }
  return h.map((x) => (x >>> 0).toString(16).padStart(8, '0')).join('');
}

/** A message as it was sent: its frozen content and the hash of it, as the fact of sending records them. */
export interface Bevroren {
  inhoud: string;
  hash: string;
}

/**
 * The message `model` frozen for sending: the message as the citizen reads it now, as text, and the hash of that
 * text. Sending is the step that makes it known, so the frozen message is no concept and stands as made known;
 * a later change of the spec, the text layer or the record does not change it.
 */
export function bevries(model: Berichtmodel): Bevroren {
  const inhoud = JSON.stringify({ ...model, status: 'bekendgemaakt', onderwerp: model.onderwerp.replace(/^Concept: /, '') });
  return { inhoud, hash: sha256(inhoud) };
}

/**
 * The fields of the fact of sending (`verzending.feit`) for the message `model` of the record `gram`: the kind of
 * message, the record, the channel, the day, the frozen content and its hash, what it replaces, and the further
 * fields the spec names (`verzending.velden`, each a field of the record or of the records it answers).
 */
export function verzendvelden(
  model: Berichtmodel,
  gram: Gram,
  grammen: Gram[],
  spec: Berichtenspec,
  dag: string,
  vervangt?: string | null,
): Record<string, unknown> {
  const { inhoud, hash } = bevries(model);
  const eerder = voorgangers(gram, grammen);
  const extra = Object.fromEntries(
    Object.entries(spec.verzending?.velden ?? {}).map(([naam, bron]) => {
      const veld = bron.veld;
      const waarde = veld ? [gram.fields, gram.findings, ...eerder.map((g) => g.fields)].map((x) => x?.[veld]).find(zegtIets) : undefined;
      return [naam, waarde ?? null];
    }),
  );
  return {
    ...extra,
    berichtsoort: model.soort,
    vastlegging: gram.id,
    kanaal: model.kanaal && model.kanaal.code !== 'onbekend' ? model.kanaal.code : null,
    verzonden_op: dag,
    inhoud,
    hash,
    vervangt: vervangt ?? null,
  };
}

/** Whether `gram` is a fact of sending or forwarding under `spec` (`verzending`). */
export function isVerzending(gram: Gram, spec?: Berichtenspec | null): boolean {
  const v = spec?.verzending;
  if (!v || !gram.fact) return false;
  const feit = `${gram.basis}#${gram.fact}`;
  if (feit === v.feit || feit === v.doorsturen) return true;
  // The receipt of a forwarded message: a fact of the same article that holds the frozen content.
  return !!v.doorsturen && gram.basis === v.doorsturen.split('#').slice(0, 2).join('#') && typeof gram.fields?.inhoud === 'string';
}

/** The bottom sheet of the letter about a record, over whatever is open: `#bericht/<id>` (BriefSheet.vue). */
export const BERICHT = '#bericht/';
export const berichtHash = (id: string) => `${BERICHT}${id}`;

/**
 * Whether a letter about `gram` is read apart from its page: a fact
 * of sending, or a record other than a decision that the message spec writes
 * about, such as the acknowledgement of an application. A decision is not one:
 * its page is the letter that makes it known.
 */
export function heeftBrief(gram: Gram, stand: Pick<Stand, 'rollen'>, spec?: Berichtenspec | null): boolean {
  if (isVerzending(gram, spec)) return true;
  return !isBesluit(gram, stand) && !!berichtsoortVoor(gram, rolVan(gram, stand), spec);
}

/** The facts of sending of the record `id` among `grammen`, oldest first; the last one stands. */
export function verzendingenVan(id: string, grammen: Gram[], spec?: Berichtenspec | null): Gram[] {
  return grammen
    .filter((g) => isVerzending(g, spec) && g.fields?.vastlegging === id)
    .sort((a, b) => (a.recorded_on < b.recorded_on ? -1 : a.recorded_on > b.recorded_on ? 1 : 0));
}

/**
 * The message as it was sent, from the fact `gram` that holds it: its frozen content, never rebuilt. With the day
 * it went, the hash and what it replaced. Content that no longer matches its hash is shown with a violation.
 */
export function verzondenBericht(gram: Gram): Berichtmodel | null {
  const inhoud = gram.fields?.inhoud;
  if (typeof inhoud !== 'string') return null;
  let model: Berichtmodel;
  try {
    model = JSON.parse(inhoud) as Berichtmodel;
  } catch {
    return null;
  }
  const hash = String(gram.fields?.hash ?? '');
  model.verzonden = {
    op: String(gram.fields?.verzonden_op ?? gram.recorded_on),
    hash,
    vervangt: typeof gram.fields?.vervangt === 'string' ? gram.fields.vervangt : null,
    feit: gram.id,
  };
  if (sha256(inhoud) !== hash) {
    model.schendingen = [
      ...(model.schendingen ?? []),
      { soort: 'spec', tekst: 'De inhoud van het verzonden bericht klopt niet met de hash die bij het verzenden werd vastgelegd.' },
    ];
  }
  return model;
}
