/*
 * Roadmap: constants, data loading and the integrity checks behind /roadmap.
 *
 * The pages are a read-only rendering of content that lives in the repo:
 * werkpakketten as a content collection (src/content/roadmap/werkpakketten/),
 * the fase/discipline matrix as JSON under src/data/. Editing goes through a
 * pull request; there is no write path.
 *
 * The lists below were the app's shared/constants.js, imported by both its
 * server (validation) and its client (rendering). Here they are the single
 * source for the zod enums in content.config.ts and for the labels the pages
 * render, so a value can never render as a tag the schema would have rejected.
 */
import { z } from 'astro:content';
import { normaliseerZoekterm, GEEN_TREFFER } from '~/lib/roadmap-zoek';
import configJson from '~/data/roadmap-config.json';
import paperHeadings from '~/research/rules-as-executed.headings.json';
import { getRfcs } from '~/lib/rfcs';

export interface Fase {
  id: string;
  volgnummer: number;
  naam: string;
  ondertitel: string;
}

export interface Discipline {
  id: string;
  naam: string;
  ondertitel: string;
}

/**
 * A swimlane: the matrix's real grouping on the vertical axis. Disciplines
 * are the rows the matrix draws; a swimlane says which of them belong
 * together and in which order, top to bottom. One discipline belongs to
 * exactly one swimlane — enforced at load time, see the check below
 * `disciplines`.
 */
export interface Swimlane {
  id: string;
  naam: string;
  disciplineIds: string[];
}

/**
 * De velden van een onderzoeksvraag in objectvorm, zoals het zod-schema in
 * content.config.ts ze oplevert; de betekenis staat daar bij `vraagVelden`.
 */
export interface VraagVelden {
  vraag: string;
  id?: string;
  paper?: string;
  status: string;
  doel: string;
  verwant: string[];
}

/** Een deelvraag: een string of de objectvorm, zonder eigen deelvragen. */
export type DeelvraagData = string | VraagVelden;

/**
 * Een user story zoals hij in de frontmatter staat, onder `stories` van een
 * hoofdvraag; de betekenis van de velden staat bij `storyVelden` in
 * content.config.ts.
 */
export interface StoryData {
  titel: string;
  id?: string;
  verhaal: string;
  stand: string;
  acceptatiecriteria: string[];
  voorstel: boolean;
}

/** Een onderzoeksvraag zoals hij in de frontmatter staat. */
export type VraagData =
  | string
  | (VraagVelden & { deelvragen: DeelvraagData[]; stories: StoryData[] });

/** A werkpakket's frontmatter, mirroring the zod schema in content.config.ts. */
export interface WerkpakketData {
  id: string;
  titel: string;
  faseId: string;
  disciplineId: string;
  prioriteit: string;
  omvang: string;
  categorie: string;
  capability: string;
  capaciteit: string;
  toelichting: string;
  volgorde: number;
  onderzoeksvragen: VraagData[];
  samenhangIds: string[];
  afhankelijkVan: string[];
  onderzoek: string;
  bouw: string;
  belegging: { stand: string; sinds?: string };
  rfcs: number[];
}

export const PRIORITEITEN = [
  { id: 'hoog', label: 'Hoog', tagColor: 'critical' },
  { id: 'midden', label: 'Midden', tagColor: 'warning' },
  { id: 'laag', label: 'Laag', tagColor: 'success' },
] as const;

export const OMVANGEN = ['S', 'M', 'L', 'XL'] as const;

/*
 * Two axes, deliberately separate.
 *
 * `onderzoek` is how far the question is answered; `bouw` is how much of it
 * stands in the codebase. They diverge in both directions, which is why one
 * combined status would be a lie in half the cases.
 *
 * The tag colours follow the same reading as elsewhere on the site: neutral
 * for "not started", warning for "under way", success for "done".
 */
export const ONDERZOEK_STANDEN = [
  { id: 'open', label: 'Open', tagColor: 'neutral' },
  { id: 'loopt', label: 'Loopt', tagColor: 'warning' },
  { id: 'beantwoord', label: 'Beantwoord', tagColor: 'success' },
] as const;

export const BOUW_STANDEN = [
  { id: 'niet', label: 'Niet gebouwd', tagColor: 'neutral' },
  { id: 'deels', label: 'Deels gebouwd', tagColor: 'warning' },
  { id: 'wel', label: 'Gebouwd', tagColor: 'success' },
] as const;

/*
 * A third axis, and deliberately not a third progress field.
 *
 * `onderzoek` and `bouw` say how far the work is. This says whether it is
 * belegd: whether anyone has put their hand up for it. Those are independent —
 * a werkpakket can be half built by someone who has since moved on, and a
 * werkpakket nobody has touched can be firmly claimed.
 *
 * `klaar` is a stand here rather than something derived from `onderzoek:
 * beantwoord` plus `bouw: wel`. A verkenning finishes without anything being
 * built, and would never reach the derived version of "done"; saying so is a
 * judgement about the werkpakket as a whole, which is a person's to make.
 *
 * '' is 'vrij'. Een leeg veld betekent dat niemand zijn hand heeft opgestoken,
 * en dat is precies wat vrij betekent: er kan iemand op. Er is geen vierde
 * stand naast de drie hieronder.
 *
 * Dit stond hier andersom, en die redenering was: '' betekent dat er niets
 * gezegd is, terwijl 'vrij' een redactionele daad is, en een roadmap waar
 * alles 'vrij' zegt omdat dat de standaard is nodigt niemand uit. Het bezwaar
 * klopt over uitnodigen, maar het antwoord was de verkeerde kant op. Wie op de
 * roadmap zoekt naar werk dat open ligt, moet ook de werkpakketten zien waar
 * nog niemand over vergaderd heeft — juist die. Ze buiten 'Vrij' houden gaf ze
 * een eigen hokje ('Niet bepaald') dat zich gedroeg als een vierde stand, en
 * dan moet iemand twee vakjes aanvinken voor één vraag.
 *
 * Wat het bezwaar wél terecht wilde voorkomen — dat de kaarten volstromen met
 * een tag die zegt dat er niets aan de hand is — blijft staan, en op de plek
 * waar het thuishoort: RoadmapMatrixCel toont geen tag voor 'vrij'. Vrij is de
 * stilzwijgende meerderheid; alleen 'opgepakt' en 'klaar' verdienen een tag.
 *
 * The colours deliberately leave the neutral/warning/success ladder that
 * `onderzoek` and `bouw` share: those two are progress, this is ownership, and
 * three tags climbing one ladder with three meanings makes a card unreadable.
 * `info` reads as an invitation, `warning` as "someone is on it", and
 * `success` is reserved so green on a card means one thing only: done.
 */
export const BELEGGING_STANDEN = [
  { id: 'vrij', label: 'Vrij', tagColor: 'info' },
  { id: 'opgepakt', label: 'Opgepakt', tagColor: 'warning' },
  { id: 'klaar', label: 'Klaar', tagColor: 'success' },
] as const;

export const getOnderzoek = (id: string) =>
  ONDERZOEK_STANDEN.find((s) => s.id === id);
export const getBouw = (id: string) => BOUW_STANDEN.find((s) => s.id === id);

/**
 * De beleggingsstand, met een leeg veld als 'vrij'.
 *
 * Deze ene functie is waar '' naar 'vrij' gaat, en daarom staat het hier en
 * niet op de aanroepplekken: de kaart, de detailpagina en het filter lezen
 * allemaal hierlangs, en drie kopieën van dezelfde `|| 'vrij'` is drie kansen
 * om er één te vergeten. Anders dan getOnderzoek en getBouw hiernaast geeft
 * deze dus nooit undefined terug voor een leeg veld.
 */
export const getBelegging = (id: string) =>
  BELEGGING_STANDEN.find((s) => s.id === (id || 'vrij'));

export const CATEGORIEEN = [
  { id: 'bar', label: 'Bar' },
  { id: 'pivot', label: 'Pivot' },
  { id: 'bet', label: 'Bet' },
] as const;

export const CAPABILITIES = [
  { id: 'basis', label: 'Basis' },
  { id: 'ontwikkelen', label: 'Ontwikkelen van wet- en regelgeving' },
  { id: 'simuleren', label: 'Simuleren van wet- en regelgeving' },
  { id: 'publiceren', label: 'Publiceren van wet- en regelgeving' },
  { id: 'analyseren', label: 'Analyseren van wet- en regelgeving' },
  { id: 'implementeren', label: 'Implementeren van wet- en regelgeving' },
  { id: 'verifieren', label: 'Verifiëren en simuleren van besluitvorming' },
] as const;

/*
 * The id lists the zod schema in content.config.ts builds its enums from.
 *
 * The tuple type is what z.enum() requires. Deriving the enums here rather
 * than repeating the ids in the schema is what makes the "single source"
 * above true: with two copies, adding a value to the schema alone would let
 * it validate while getPrioriteit() returns undefined, and the card would
 * silently render no tag for a value that was set.
 */
type NonEmpty = [string, ...string[]];

/**
 * The value `data-categorie` carries for a werkpakket without a categorie.
 * Twenty of the forty-nine have none, so the filter needs a way to show them:
 * without one, checking any box hides them with no control to bring them
 * back. Shared by the page and the stylesheet's selectors.
 */
export const GEEN_CATEGORIE = 'geen';

/**
 * De waarde van `data-status` op een onderzoeksvraag, met een leeg veld als
 * 'geen' — dezelfde afbeelding als GEEN_CATEGORIE maakt voor een kaart
 * zonder categorie, en om dezelfde reden: het statusfilter op het overzicht
 * heeft een vakje "Niet bepaald" nodig om die vragen terug te halen, en dat
 * vakje moet een waarde hebben om op te matchen.
 */
export const GEEN_STATUS = 'geen';
export const vraagStatusWaarde = (status: string) => status || GEEN_STATUS;


/**
 * De beleggingsstand zoals hij op `data-belegging` komt te staan, met een leeg
 * veld als 'vrij' — dezelfde afbeelding die getBelegging() maakt.
 *
 * Dit moet dezelfde waarde opleveren als de knop in het filter, anders vinkt
 * iemand 'Vrij' aan en verdwijnen juist de werkpakketten waar nog niemand iets
 * over gezegd heeft. Er is daarom geen `GEEN_BELEGGING` meer: er is geen
 * vierde stand om een eigen waarde voor te hebben.
 */
export const beleggingStand = (stand: string) => stand || 'vrij';

/**
 * De vinkjes van het beleggingsfilter: de drie standen, en niet meer.
 *
 * Anders dan FILTER_OPTIES hieronder heeft deze geen 'zonder'-optie, omdat er
 * geen werkpakket zonder belegging is: een leeg veld is 'vrij'. Bij categorie
 * ligt dat wel zo — daar is geen categorie een echte toestand van twintig van
 * de negenenveertig, en zonder vakje ervoor zijn ze niet meer terug te halen.
 */
export const BELEGGING_FILTER_OPTIES = BELEGGING_STANDEN.map((s) => ({
  id: s.id,
  label: s.label,
}));

/** The filter's checkboxes: every categorie, plus the ones without one. */
export const FILTER_OPTIES = [
  ...CATEGORIEEN.map((c) => ({ id: c.id, label: c.label })),
  { id: GEEN_CATEGORIE, label: 'Zonder categorie' },
];

/**
 * Een filtergroep in de kopbalk: de knop, de vinkjes erachter, en hoe het
 * filter werkt.
 *
 * Twee soorten. Een CSS-filter (geen `attribuut`) werkt via de
 * `:has(#rr-<id>-<optie>[checked])`-regels in roadmap.css en heeft geen
 * script nodig. Een JS-filter vergelijkt `data-<attribuut>` op elk item met de
 * aangevinkte opties en zet `verbergKlasse` op wat niet matcht; roadmap.css
 * verbergt die klasse. Waarom het tweede filter niet óók CSS kon zijn staat
 * bij de verbergregel in roadmap.css.
 *
 * RoadmapKop.astro rendert de groep en zet de velden als data-attributen op de
 * DOM, zodat het script niets van belegging of status hoeft te weten: de
 * pagina zegt wat er staat.
 */
export interface FilterGroep {
  /** Het korte id in de element-ids: `rr-<id>-knop`, `rr-<id>-<optie>`. */
  id: string;
  knop: string;
  titel: string;
  /** De klasse op elk vinkje; het script telt en leest erop. */
  optieKlasse: string;
  opties: { id: string; label: string }[];
  /**
   * De dataset-sleutel op het item die het script vergelijkt: `belegging` voor
   * `data-belegging`. Eén woord, want het script leest `item.dataset[attribuut]`
   * en een naam met een koppelteken zou daar als camelCase moeten staan.
   * Afwezig bij een CSS-filter.
   */
  attribuut?: string;
  /** De klasse die het script zet op een item dat niet matcht. Afwezig bij een CSS-filter. */
  verbergKlasse?: string;
}

export type FilterGroepId = 'categorie' | 'belegging' | 'stand';

export const FILTERGROEPEN: Record<FilterGroepId, FilterGroep> = {
  categorie: {
    id: 'cat',
    knop: 'Categorie',
    titel: 'Filter op categorie',
    optieKlasse: 'rr-filter__option',
    opties: FILTER_OPTIES,
  },
  belegging: {
    id: 'bel',
    knop: 'Belegging',
    titel: 'Filter op belegging',
    optieKlasse: 'rr-filter__belegging-option',
    opties: BELEGGING_FILTER_OPTIES,
    attribuut: 'belegging',
    verbergKlasse: 'rr-geen-belegging',
  },
  /*
   * De stand van een onderzoeksvraag: vrij, opgepakt of klaar, zoals
   * vraagLane() hem bepaalt. Dezelfde drie vinkjes als het beleggingsfilter,
   * want een vraag staat naast zijn werkpakket; de kaart draagt de uitkomst
   * op `data-lane`.
   */
  stand: {
    id: 'stand',
    knop: 'Stand',
    titel: 'Filter op stand van de vraag',
    optieKlasse: 'rr-filter__stand-option',
    opties: BELEGGING_FILTER_OPTIES,
    attribuut: 'lane',
    verbergKlasse: 'rr-vraag--geen-stand',
  },
};

/*
 * De verbergklasse van het zoekfilter staat in lib/roadmap-zoek.ts, omdat het
 * script dat hem zet alleen die module kan importeren; hier opnieuw
 * geëxporteerd zodat de pagina's en assertFilterRules() één bron hebben.
 */
export { GEEN_TREFFER };

/**
 * De weergaven van de roadmap, in de volgorde van de tab-bar in de kop. Elke
 * weergave is een eigen route met dezelfde kop (RoadmapKop.astro); de
 * tab-bar verschijnt pas zodra er meer dan één is, want één weergave is geen
 * keuze.
 */
export const WEERGAVEN = [
  { id: 'matrix', label: 'Matrix', href: '/roadmap' },
  { id: 'bord', label: 'Bord', href: '/roadmap/bord' },
  {
    id: 'onderzoeksvragen',
    label: 'Onderzoeksvragen',
    href: '/roadmap/onderzoeksvragen',
  },
] as const;

export type WeergaveId = (typeof WEERGAVEN)[number]['id'];

/**
 * Fail the build when the filter's stylesheet has no show-rule for an option
 * the page renders.
 *
 * The checkboxes come from FILTER_OPTIES, but the rules that show their cards
 * again live in roadmap.css, which cannot read this list. Add a categorie and
 * its checkbox appears while its cards stay hidden behind the blanket
 * hide-rule, with nothing to bring them back — silent, and only on the
 * filtered view. Reading the stylesheet here keeps the two in step.
 */
export function assertFilterRules(css: string): void {
  const missing = FILTER_OPTIES.filter(
    (optie) => !css.includes(`#rr-cat-${optie.id}[checked]`),
  ).map((optie) => optie.id);

  if (missing.length) {
    throw new Error(
      `roadmap.css mist een toon-regel voor filteroptie(s) ${missing
        .map((id) => `"${id}"`)
        .join(', ')}. Voeg een ` +
        `\`.rr-roadmap:has(#rr-cat-<id>[checked]) [data-categorie='<id>']\`-regel toe, ` +
        'anders blijven die kaarten verborgen zodra er gefilterd wordt.',
    );
  }

  /*
   * Het zoekfilter en elke JS-filtergroep verbergen via een klasse die het
   * script zet, dus elk heeft één regel nodig in plaats van één per optie.
   * Ontbreekt die, dan faalt het even stil: de vinkjes toggelen een klasse
   * die niets opmaakt, en het filter lijkt aangesloten terwijl er niets op
   * het scherm verandert.
   */
  const verbergKlassen = [
    GEEN_TREFFER,
    ...Object.values(FILTERGROEPEN).flatMap((g) =>
      g.verbergKlasse ? [g.verbergKlasse] : [],
    ),
  ];
  for (const klasse of verbergKlassen) {
    if (!css.includes(`.${klasse}`)) {
      throw new Error(
        `roadmap.css mist de verberg-regel \`.${klasse}\`. Voeg hem toe aan de ` +
          '`display: none !important`-regel naast `.rr-geen-treffer`, anders ' +
          'doet het filter dat deze klasse zet niets.',
      );
    }
  }

  /*
   * De afhankelijkhedenschakelaar is een nldd-toggle-button, en die
   * reflecteert zijn stand naar `selected` — niet naar `checked`, zoals de
   * nldd-checkbox-field die hij verving.
   *
   * Dat verschil is onzichtbaar tot het misgaat: een `[checked]` dat hier
   * bleef staan matcht nooit, dus de weergave gaat gewoon nooit aan, zonder
   * fout en zonder spoor in de console. Dezelfde val als bij de filterregels
   * hierboven, dus dezelfde behandeling — omvallen tijdens de build.
   */
  if (css.includes('#rr-afhankelijkheden[checked]')) {
    throw new Error(
      'roadmap.css leest `#rr-afhankelijkheden[checked]`, maar de schakelaar ' +
        'is een nldd-toggle-button en die reflecteert `selected`. Vervang ' +
        '`[checked]` door `[selected]`, anders gaat de ' +
        'afhankelijkhedenweergave nooit aan.',
    );
  }
  if (!css.includes('#rr-afhankelijkheden[selected]')) {
    throw new Error(
      'roadmap.css mist de regels achter `#rr-afhankelijkheden[selected]`. ' +
        'Zonder die selector blijft de matrix in de gewone stapelweergave ' +
        'staan, ook met de schakelaar aan.',
    );
  }
}

export const ONDERZOEK_IDS = ONDERZOEK_STANDEN.map((s) => s.id) as NonEmpty;
export const BOUW_IDS = BOUW_STANDEN.map((s) => s.id) as NonEmpty;
export const BELEGGING_IDS = BELEGGING_STANDEN.map((s) => s.id) as NonEmpty;
export const PRIORITEIT_IDS = PRIORITEITEN.map((p) => p.id) as NonEmpty;
export const OMVANG_IDS = [...OMVANGEN] as NonEmpty;
export const CATEGORIE_IDS = CATEGORIEEN.map((c) => c.id) as NonEmpty;
export const CAPABILITY_IDS = CAPABILITIES.map((c) => c.id) as NonEmpty;

export const getPrioriteit = (id: string) =>
  PRIORITEITEN.find((p) => p.id === id);
export const getCategorie = (id: string) => CATEGORIEEN.find((c) => c.id === id);
export const getCapability = (id: string) =>
  CAPABILITIES.find((c) => c.id === id);

/*
 * The config JSON gets the same build-time validation the werkpakketten get
 * from their collection schema. Without it a hand-edit that drops a key fails
 * far from its cause: a missing `naam` on a fase surfaces as "Cannot read
 * properties of undefined" out of a page template, naming neither the file nor
 * the entry. parse() throws during the build instead, with the path to the
 * offending field.
 */
const configSchema = z.object({
  fases: z
    .array(
      z.object({
        id: z.string().min(1),
        volgnummer: z.number(),
        naam: z.string().min(1),
        ondertitel: z.string(),
      }),
    )
    .min(1),
  disciplines: z
    .array(
      z.object({
        id: z.string().min(1),
        naam: z.string().min(1),
        ondertitel: z.string(),
      }),
    )
    .min(1),
  swimlanes: z
    .array(
      z.object({
        id: z.string().min(1),
        naam: z.string().min(1),
        disciplineIds: z.array(z.string().min(1)).min(1),
      }),
    )
    .min(1),
});

const config = configSchema.parse(configJson);

export const fases: Fase[] = [...config.fases].sort(
  (a, b) => a.volgnummer - b.volgnummer,
);
export const disciplines: Discipline[] = config.disciplines;
export const swimlanes: Swimlane[] = config.swimlanes;

export const getFase = (id: string) => fases.find((f) => f.id === id);
export const getDiscipline = (id: string) =>
  disciplines.find((d) => d.id === id);

/**
 * Fail the build when swimlanes and disciplines disagree about which rows
 * exist: a disciplineId a swimlane points at but that isn't in
 * `disciplines`, a discipline in two swimlanes at once (it would render
 * twice), or a discipline in none (it would silently not render at all,
 * the same failure mode `assertReferencesResolve` guards against for
 * werkpakketten). This runs at import time, like the zod parse above,
 * because roadmap-config.json is static — there is no per-request state
 * that could still make it valid.
 */
function assertSwimlanesMatchDisciplines(): void {
  const disciplineIds = new Set(disciplines.map((d) => d.id));
  const gezien = new Set<string>();
  const problems: string[] = [];

  for (const lane of swimlanes) {
    for (const id of lane.disciplineIds) {
      if (!disciplineIds.has(id)) {
        problems.push(
          `swimlane "${lane.id}" verwijst naar onbekende disciplineId "${id}"`,
        );
        continue;
      }
      if (gezien.has(id)) {
        problems.push(`disciplineId "${id}" zit in meer dan één swimlane`);
        continue;
      }
      gezien.add(id);
    }
  }

  for (const id of disciplineIds) {
    if (!gezien.has(id)) {
      problems.push(`discipline "${id}" zit in geen enkele swimlane`);
    }
  }

  if (problems.length) {
    throw new Error(
      `roadmap-config.json: swimlanes en disciplines komen niet overeen:\n  ${problems.join('\n  ')}`,
    );
  }
}

assertSwimlanesMatchDisciplines();

/** Placeholder wording for fields the roadmap has not filled in yet. */
export const NIET_BEPAALD = 'Nog niet bepaald';
export const NIET_GESPECIFICEERD = 'Niet gespecificeerd';

/**
 * The werkpakketten of one matrix cell, in the order the roadmap puts them.
 * `volgorde` was maintained by drag-and-drop in the app; here it is just a
 * number in the frontmatter, so a gap or a duplicate is harmless.
 */
export function werkpakkettenInCel<T extends { data: WerkpakketData }>(
  alle: T[],
  faseId: string,
  disciplineId: string,
): T[] {
  return alle
    .filter(
      (w) => w.data.faseId === faseId && w.data.disciplineId === disciplineId,
    )
    .sort((a, b) => a.data.volgorde - b.data.volgorde);
}

/*
 * De rang van elke discipline op de matrix: swimlane voor swimlane, en
 * daarbinnen de volgorde van `disciplineIds`. Dezelfde volgorde als
 * `matrixRijen` in pages/roadmap/index.astro, hier als getal zodat een
 * sortering erop kan.
 */
const rijRang = new Map(
  swimlanes.flatMap((lane) => lane.disciplineIds).map((id, i) => [id, i]),
);

/**
 * De volgorde van werkpakketten buiten de matrix: eerst de fase (de kolom),
 * dan de rij zoals de matrix hem tekent, dan `volgorde` binnen de cel, en als
 * laatste het id zodat de uitkomst stabiel is.
 *
 * Dat is de leesvolgorde van de matrix, links naar rechts en van boven naar
 * beneden. Het bord toont zo dezelfde werkpakketten in dezelfde volgorde als
 * een rondgang over de matrix, en wie van de ene weergave naar de andere gaat
 * hoeft niet opnieuw te zoeken.
 *
 * Een onbekende fase of discipline sorteert achteraan; assertReferencesResolve
 * heeft die bij de build al gemeld, dus dit is alleen de val als die controle
 * er een keer niet voor stond.
 */
export function werkpakketVolgorde(a: WerkpakketData, b: WerkpakketData): number {
  const fase =
    (getFase(a.faseId)?.volgnummer ?? Infinity) -
    (getFase(b.faseId)?.volgnummer ?? Infinity);
  if (fase) return fase;
  const rij =
    (rijRang.get(a.disciplineId) ?? Infinity) -
    (rijRang.get(b.disciplineId) ?? Infinity);
  if (rij) return rij;
  return a.volgorde - b.volgorde || a.id.localeCompare(b.id);
}

/** Een tag op een kaart, zoals nldd-tag hem rendert. */
export interface KaartTag {
  color: string;
  text: string;
  /** De toegankelijke naam; de zichtbare tekst is soms een afkorting. */
  label: string;
  icon?: string;
}

/**
 * De tags van een werkpakket op een kaart: prioriteit, omvang, categorie en
 * capability, in die volgorde, alleen de velden die ingevuld zijn. De
 * belegging zit er niet bij: de werkpakketkaart zet die vooraan, de
 * vraagtegel zegt er iets anders over (zie RoadmapVraagRij.astro).
 *
 * Eén helper voor beide kaarten, zodat een vraag dezelfde tags draagt als
 * het werkpakket waar hij in staat, met dezelfde kleuren en dezelfde
 * afkortingen.
 */
export function werkpakketTags(data: WerkpakketData): KaartTag[] {
  const prioriteit = getPrioriteit(data.prioriteit);
  const categorie = getCategorie(data.categorie);
  const capability = getCapability(data.capability);
  return [
    prioriteit
      ? { color: prioriteit.tagColor, text: prioriteit.label, label: prioriteit.label }
      : undefined,
    data.omvang
      ? { color: 'neutral', text: data.omvang, label: `Omvang ${data.omvang}` }
      : undefined,
    categorie
      ? { color: 'neutral', text: categorie.label, label: categorie.label }
      : undefined,
    capability
      ? {
          color: 'accent',
          text: capability.label.split(' ')[0],
          label: capability.label,
        }
      : undefined,
  ].filter((tag): tag is KaartTag => tag !== undefined);
}

/** "13 RFC's", of undefined zonder RFC's; de stille telling naast de tags. */
export function rfcTekst(data: WerkpakketData): string | undefined {
  return data.rfcs.length
    ? `${data.rfcs.length} RFC${data.rfcs.length > 1 ? "'s" : ''}`
    : undefined;
}

/**
 * "Fase I · Techniek & Architectuur": de cel van de matrix, in woorden, voor
 * een weergave waar die cel niet te zien is.
 */
export function kaartOndertitel(data: WerkpakketData): string {
  return [getFase(data.faseId)?.naam, getDiscipline(data.disciplineId)?.naam]
    .filter(Boolean)
    .join(' · ');
}


/**
 * Everything of a werkpakket that the zoekfilter on /roadmap matches against,
 * as one lowercased string.
 *
 * Full text in the literal sense: the toelichting and the onderzoeksvragen are
 * in here too, and those render only on the detail page. Searching the card's
 * visible words alone would mean a term you read in a toelichting finds
 * nothing, which is the case where a search earns its place — 34 of the 49
 * werkpakketten have a toelichting and 32 have onderzoeksvragen.
 *
 * The labels go in next to the ids (`hoog`, not just `Hoog`), because someone
 * types what the tag says, not what the frontmatter stores. Fase and discipline
 * come from the matrix axes, so "garantie" finds that column's cards even
 * though the word is nowhere on them.
 *
 * Normalised through the same function the typed query goes through — it lives
 * in lib/roadmap-zoek.ts precisely so both sides can import it — so a search
 * for "verifieren" matches the capability "Verifiëren en simuleren".
 */
export function zoektekst(data: WerkpakketData): string {
  // De vraag, zijn doel en zijn deelvragen: alles wat op de detailpagina bij
  // de vraag staat, zodat een woord uit een deelvraag het werkpakket vindt.
  const vragen = onderzoeksvraagLijst(data.onderzoeksvragen).flatMap(vraagTekst);

  return normaliseerZoekterm(
    [
      data.titel,
      data.toelichting,
      ...vragen,
      getFase(data.faseId)?.naam,
      getFase(data.faseId)?.ondertitel,
      getDiscipline(data.disciplineId)?.naam,
      getDiscipline(data.disciplineId)?.ondertitel,
      getPrioriteit(data.prioriteit)?.label,
      getCategorie(data.categorie)?.label,
      getCapability(data.capability)?.label,
      getOnderzoek(data.onderzoek)?.label,
      getBouw(data.bouw)?.label,
      getBelegging(data.belegging.stand)?.label,
      data.omvang && `omvang ${data.omvang}`,
      data.capaciteit,
      // Every way an RFC gets written: "RFC-013" as the site renders it, plus
      // the unpadded "rfc-13" and "rfc 13" people actually type. A bare number
      // is left out on purpose — "13" would match every werkpakket whose text
      // happens to contain it.
      ...data.rfcs.flatMap((n) => [
        `rfc-${String(n).padStart(3, '0')}`,
        `rfc-${n}`,
        `rfc ${n}`,
      ]),
    ]
      .filter(Boolean)
      .join(' '),
  );
}

/**
 * A research question as the pages render it: the text, the paper section it
 * belongs to when there is one, and the structure around it (status, doel,
 * verwant, deelvragen) as the frontmatter wrote it.
 */
export interface Onderzoeksvraag {
  vraag: string;
  /** The slug from the frontmatter; absent on a question nothing points at. */
  id?: string;
  /** `vraag-<id>`: het anker op de pagina's. Alleen met een id. */
  anker?: string;
  /** The paper section, resolved from its anchor. Absent when unlinked. */
  paper?: PaperSectie;
  /** open, loopt, beantwoord, of '' voor niet bepaald; zie ONDERZOEK_STANDEN. */
  status: string;
  doel: string;
  /** De ids uit de frontmatter, één kant op; verwantIndex() leest beide. */
  verwant: string[];
  deelvragen: Onderzoeksvraag[];
  /** De user stories onder de vraag, in prioriteitsvolgorde; leeg op een deelvraag. */
  stories: UserStory[];
}

/**
 * Een user story zoals de pagina's hem renderen. Dezelfde velden als
 * StoryData, plus het anker voor een story met een id.
 */
export interface UserStory extends StoryData {
  /** `story-<id>`: het anker op de pagina's. Alleen met een id. */
  anker?: string;
}

/** Het anker van een story op de werkpakketpagina en het overzicht. */
export const storyAnker = (id: string) => `story-${id}`;

/** Het anker van een vraag op de werkpakketpagina en het overzicht. */
export const vraagAnker = (id: string) => `vraag-${id}`;

/**
 * De tekst van een vraag met zijn doel, deelvragen en stories, voor het
 * zoeken. De stories tellen mee: wie op "aanvraagscherm" zoekt vindt de
 * vraag waar die story onder hangt, ook als de vraag zelf het woord niet
 * gebruikt.
 */
function vraagTekst(v: Onderzoeksvraag): string[] {
  return [
    v.vraag,
    v.doel,
    ...v.deelvragen.flatMap(vraagTekst),
    ...v.stories.flatMap((st) => [st.titel, st.verhaal, ...st.acceptatiecriteria]),
  ];
}

/**
 * Wat het zoekfilter op /roadmap/onderzoeksvragen over een vraag doorzoekt:
 * de vraag met zijn doel en deelvragen, de titel van het werkpakket (wie op
 * een werkpakket zoekt vindt zijn vragen), de status zoals de tag hem
 * schrijft, en de papersectie op nummer en titel. Genormaliseerd zoals
 * zoektekst() voor de kaarten.
 */
export function vraagZoektekst(
  vraag: Onderzoeksvraag,
  werkpakket: WerkpakketData,
): string {
  return normaliseerZoekterm(
    [
      ...vraagTekst(vraag),
      werkpakket.titel,
      getOnderzoek(vraag.status)?.label,
      vraag.paper && `§ ${vraag.paper.nummer}`,
      vraag.paper?.titel,
    ]
      .filter(Boolean)
      .join(' '),
  );
}


/** A section of the position paper, addressable by its anchor. */
export interface PaperSectie {
  /** The anchor, e.g. "sec:traceaccess". */
  slug: string;
  /** The section number, e.g. "4.5". */
  nummer: string;
  /** The section title without its number, e.g. "The Recipient's Check". */
  titel: string;
  /** The href a link should use. */
  href: string;
}

export const PAPER_PAD = '/research/rules-as-executed';

/*
 * The paper's sections, keyed by anchor.
 *
 * Read from the headings JSON the research page already ships, so a section
 * number or title can never drift from the paper: both are the paper's own
 * words. The heading text is "4.5 The Recipient's Check", number and title in
 * one string, which is why they are split here.
 */
export const paperSectieLijst: PaperSectie[] = (
  paperHeadings as { slug: string; text: string }[]
).map((h) => {
  const m = /^([\d.]+)\s+(.*)$/.exec(h.text);
  return {
    slug: h.slug,
    nummer: m ? m[1] : '',
    titel: m ? m[2] : h.text,
    href: `${PAPER_PAD}#${h.slug}`,
  };
});

const paperSecties = new Map(paperSectieLijst.map((s) => [s.slug, s]));

export const getPaperSectie = (slug: string) => paperSecties.get(slug);

function normaliseerVraag(v: VraagData | DeelvraagData): Onderzoeksvraag {
  if (typeof v === 'string') {
    return {
      vraag: v,
      status: '',
      doel: '',
      verwant: [],
      deelvragen: [],
      stories: [],
    };
  }
  return {
    vraag: v.vraag,
    id: v.id,
    anker: v.id ? vraagAnker(v.id) : undefined,
    paper: v.paper ? getPaperSectie(v.paper) : undefined,
    status: v.status,
    doel: v.doel,
    verwant: v.verwant,
    deelvragen: ('deelvragen' in v ? v.deelvragen : []).map(normaliseerVraag),
    stories: ('stories' in v ? v.stories : []).map((st) => ({
      ...st,
      anker: st.id ? storyAnker(st.id) : undefined,
    })),
  };
}

/**
 * One shape for the page: both the plain-string and the object form of a
 * research question come out as an Onderzoeksvraag, deelvragen included.
 */
export function onderzoeksvraagLijst(
  vragen: WerkpakketData['onderzoeksvragen'],
): Onderzoeksvraag[] {
  return vragen.map(normaliseerVraag);
}

/** Een (deel)vraag met het werkpakket waar hij in staat. */
export interface VraagInWerkpakket {
  vraag: Onderzoeksvraag;
  /** De bovenliggende vraag, bij een deelvraag. */
  ouder?: Onderzoeksvraag;
  werkpakket: WerkpakketData;
  /** "vraag 3" of "vraag 3, deelvraag 2": de plek in het bestand, voor meldingen. */
  plek: string;
}

/**
 * Alle onderzoeksvragen van de roadmap op één rij, deelvragen achter hun
 * ouder, elk met zijn werkpakket. De bron voor het overzicht, voor
 * verwantIndex() en voor assertOnderzoeksvragen().
 */
export function alleOnderzoeksvragen(
  werkpakketten: { data: WerkpakketData }[],
): VraagInWerkpakket[] {
  const uit: VraagInWerkpakket[] = [];
  for (const { data } of werkpakketten) {
    onderzoeksvraagLijst(data.onderzoeksvragen).forEach((vraag, i) => {
      uit.push({ vraag, werkpakket: data, plek: `vraag ${i + 1}` });
      vraag.deelvragen.forEach((deel, j) => {
        uit.push({
          vraag: deel,
          ouder: vraag,
          werkpakket: data,
          plek: `vraag ${i + 1}, deelvraag ${j + 1}`,
        });
      });
    });
  }
  return uit;
}

/** Een verwante vraag zoals de pagina hem linkt. */
export interface VraagVerwijzing {
  id: string;
  vraag: string;
  werkpakketTitel: string;
  /** De werkpakketpagina, op het anker van de vraag. */
  href: string;
}

/**
 * Per vraag-id de verwante vragen, beide kanten gelezen.
 *
 * `verwant` staat in de frontmatter aan één kant, zoals afhankelijkVan: wie A
 * aan B koppelt hoeft B niet ook aan A te koppelen, en mag dat ook niet, want
 * dan staat dezelfde relatie twee keer en raakt hij bij een wijziging aan één
 * kant uit de pas. Deze index leest de geschreven kant en leidt de andere af,
 * ontdubbeld, in de volgorde waarin de collectie de werkpakketten aanlevert.
 *
 * Een verwijzing naar een id dat niet bestaat valt hier stil weg;
 * assertOnderzoeksvragen() heeft hem bij de build al gemeld.
 */
export function verwantIndex(
  werkpakketten: { data: WerkpakketData }[],
): Map<string, VraagVerwijzing[]> {
  const alle = alleOnderzoeksvragen(werkpakketten);
  const opId = new Map(
    alle.filter((v) => v.vraag.id).map((v) => [v.vraag.id!, v]),
  );
  const verwijzing = (v: VraagInWerkpakket): VraagVerwijzing => ({
    id: v.vraag.id!,
    vraag: v.vraag.vraag,
    werkpakketTitel: v.werkpakket.titel,
    href: `/roadmap/werkpakket/${v.werkpakket.id}#${v.vraag.anker}`,
  });
  const index = new Map<string, VraagVerwijzing[]>();
  const voeg = (van: string, naar: VraagInWerkpakket) => {
    const lijst = index.get(van) ?? [];
    if (!lijst.some((x) => x.id === naar.vraag.id)) lijst.push(verwijzing(naar));
    index.set(van, lijst);
  };
  for (const v of alle) {
    if (!v.vraag.id) continue;
    for (const doel of v.vraag.verwant) {
      const ander = opId.get(doel);
      if (!ander || doel === v.vraag.id) continue;
      voeg(v.vraag.id, ander);
      voeg(doel, v);
    }
  }
  return index;
}

/** Hoeveel deelvragen beantwoord zijn, voor "2 van 3 deelvragen beantwoord". */
export function telDeelvragen(vraag: Onderzoeksvraag): {
  beantwoord: number;
  totaal: number;
} {
  return {
    beantwoord: vraag.deelvragen.filter((d) => d.status === 'beantwoord').length,
    totaal: vraag.deelvragen.length,
  };
}

/** Een bovenliggende vraag met het werkpakket waar hij in staat. */
export interface VraagMetWerkpakket {
  vraag: Onderzoeksvraag;
  werkpakket: WerkpakketData;
}

/**
 * In welke lane van het vragenbord een vraag staat: vrij, opgepakt of klaar.
 *
 * De eigen status van de vraag gaat voor: open is vrij, loopt is opgepakt,
 * beantwoord is klaar. Zonder eigen status volgt de vraag de belegging van
 * zijn werkpakket: wie een werkpakket oppakt, pakt de vragen erin op, tot er
 * per vraag iets anders over gezegd is. De kaart laat zien welke van de twee
 * het was (`eigen`), zodat een vraag die alleen via zijn werkpakket op
 * Opgepakt staat niet leest als een vraag waar iemand aan werkt.
 */
export const LANE_VAN_STATUS: Record<string, string> = {
  open: 'vrij',
  loopt: 'opgepakt',
  beantwoord: 'klaar',
};

export function vraagLane(
  vraag: Onderzoeksvraag,
  werkpakket: WerkpakketData,
): { lane: string; eigen: boolean } {
  const eigen = LANE_VAN_STATUS[vraag.status];
  if (eigen) return { lane: eigen, eigen: true };
  return { lane: beleggingStand(werkpakket.belegging.stand), eigen: false };
}

/**
 * Dezelfde afleiding één laag dieper: een deelvraag staat in de lane van zijn
 * eigen status, en zonder eigen status in die van zijn ouder. Wie aan een
 * vraag werkt, werkt aan de deelvragen erin, tot er per deelvraag iets anders
 * over gezegd is.
 */
export function deelvraagLane(
  deelvraag: Onderzoeksvraag,
  ouderLane: string,
): { lane: string; eigen: boolean } {
  const eigen = LANE_VAN_STATUS[deelvraag.status];
  if (eigen) return { lane: eigen, eigen: true };
  return { lane: ouderLane, eigen: false };
}

/**
 * De lane van een user story: zijn eigen stand (vrij, opgepakt, klaar), en
 * zonder eigen stand die van zijn vraag. Dezelfde afleiding als bij een
 * deelvraag, maar zonder vertaling: een story spreekt al het vocabulaire van
 * de lanes.
 */
export function storyLane(
  story: UserStory,
  ouderLane: string,
): { lane: string; eigen: boolean } {
  if (story.stand) return { lane: story.stand, eigen: true };
  return { lane: ouderLane, eigen: false };
}

/** "3 user stories" of "1 user story", voor tellingen op tegels en tickets. */
export const storiesTekst = (n: number) =>
  `${n} ${n === 1 ? 'user story' : 'user stories'}`;

/**
 * De tellingen van de user stories: hoeveel er zijn, hoeveel per lane (zoals
 * storyLane() ze afleidt, dus met de stand van de vraag erbij), en hoeveel
 * daarvan nog een voorstel zijn. Voor de regel boven het vragenoverzicht.
 */
export function telStories(werkpakketten: { data: WerkpakketData }[]): {
  totaal: number;
  perLane: Record<string, number>;
  voorstel: number;
} {
  const perLane: Record<string, number> = {};
  let totaal = 0;
  let voorstel = 0;
  for (const { data } of werkpakketten) {
    for (const vraag of onderzoeksvraagLijst(data.onderzoeksvragen)) {
      const ouder = vraagLane(vraag, data).lane;
      for (const story of vraag.stories) {
        const { lane } = storyLane(story, ouder);
        perLane[lane] = (perLane[lane] ?? 0) + 1;
        totaal += 1;
        if (story.voorstel) voorstel += 1;
      }
    }
  }
  return { totaal, perLane, voorstel };
}

/**
 * De onderzoeksvragen van een werkpakket als tickets voor het mini-bord: de
 * stand uit de eigen status, of uit de belegging van het werkpakket. Gedeeld
 * door het bord en het werkpakketpaneel, zodat beide hetzelfde laten zien.
 */
export function werkpakketTickets(data: WerkpakketData): MiniTicket[] {
  return onderzoeksvraagLijst(data.onderzoeksvragen).map((vraag) => {
    const { lane, eigen } = vraagLane(vraag, data);
    return {
      tekst: vraag.vraag,
      lane,
      eigen,
      bron: 'werkpakket',
      status: vraag.status,
      onder:
        [
          vraag.paper && `§ ${vraag.paper.nummer}`,
          vraag.deelvragen.length > 0 &&
            `${vraag.deelvragen.length} ${vraag.deelvragen.length === 1 ? 'deelvraag' : 'deelvragen'}`,
          vraag.stories.length > 0 && storiesTekst(vraag.stories.length),
        ]
          .filter(Boolean)
          .join(' · ') || undefined,
    };
  });
}

/** Eén klein ticket op een mini-bord onder een uitgeklapte rij. */
export interface MiniTicket {
  tekst: string;
  /** vrij, opgepakt of klaar; zie vraagLane() en deelvraagLane(). */
  lane: string;
  /** Of de lane uit de eigen status komt (true) of overgenomen is. */
  eigen: boolean;
  /** Waarvan overgenomen, voor het bijschrift: "werkpakket" of "vraag". */
  bron: string;
  /** De eigen status, als die er is; wordt een tag. */
  status: string;
  /** Een stille regel, bijvoorbeeld het §-nummer of het aantal deelvragen. */
  onder?: string;
}

/**
 * Alle bovenliggende onderzoeksvragen in de volgorde van het position paper:
 * eerst op sectie (§ 4.1, § 4.2, … zoals het paper ze nummert), daarbinnen
 * in de leesvolgorde van de matrix en dan de volgorde van het bestand. Vragen
 * zonder sectie komen achteraan, want het paper is de agenda en die lijst is
 * wat er nog niet aan hangt. Deelvragen staan in de tegel van hun ouder,
 * niet los in de lijst.
 */
export function vragenOpVolgorde(
  werkpakketten: { data: WerkpakketData }[],
): VraagMetWerkpakket[] {
  const sectieRang = new Map(paperSectieLijst.map((s, i) => [s.slug, i]));
  const uit: (VraagMetWerkpakket & { rang: number[] })[] = [];
  const gesorteerd = [...werkpakketten].sort((a, b) =>
    werkpakketVolgorde(a.data, b.data),
  );
  gesorteerd.forEach(({ data }, w) => {
    onderzoeksvraagLijst(data.onderzoeksvragen).forEach((vraag, i) => {
      uit.push({
        vraag,
        werkpakket: data,
        rang: [vraag.paper ? (sectieRang.get(vraag.paper.slug) ?? Infinity) : Infinity, w, i],
      });
    });
  });
  // Stabiel en op drie sleutels: een gelijke sectie valt terug op de
  // werkpakketvolgorde, en die op de plek in het bestand.
  uit.sort((a, b) => {
    for (let k = 0; k < 3; k++) {
      const d = a.rang[k] - b.rang[k];
      if (d) return d;
    }
    return 0;
  });
  return uit.map(({ rang: _rang, ...rest }) => rest);
}

/**
 * De tellingen boven het overzicht: hoeveel vragen er zijn en hoeveel er per
 * eigen status staan, hoeveel deelvragen daaronder hangen en hoeveel daarvan
 * beantwoord zijn, en daarnaast hoeveel werkpakketten er per `onderzoek`-stand
 * staan. Twee regels en niet één, omdat het twee dingen zijn: een vraag zonder
 * eigen status telt als "niet bepaald", ook als zijn werkpakket op "loopt"
 * staat.
 *
 * `perStatus` telt alleen de bovenliggende vragen, want dat is wat het
 * statusfilter op de pagina filtert: een deelvraag gaat met zijn ouder mee en
 * is niet los te tonen. Telden de deelvragen hier mee, dan zei de samenvatting
 * "3 beantwoord" terwijl het filter er nul liet zien.
 */
export function telStatussen(werkpakketten: { data: WerkpakketData }[]): {
  vragen: { totaal: number; perStatus: Record<string, number> };
  deelvragen: { totaal: number; beantwoord: number };
  werkpakketten: { totaal: number; perStatus: Record<string, number> };
} {
  const alle = alleOnderzoeksvragen(werkpakketten);
  const hoofd = alle.filter((v) => !v.ouder);
  const deel = alle.filter((v) => v.ouder);
  const tel = (waarden: string[]) => {
    const per: Record<string, number> = {};
    for (const w of waarden) per[w] = (per[w] ?? 0) + 1;
    return per;
  };
  return {
    vragen: {
      totaal: hoofd.length,
      perStatus: tel(hoofd.map((v) => vraagStatusWaarde(v.vraag.status))),
    },
    deelvragen: {
      totaal: deel.length,
      beantwoord: deel.filter((v) => v.vraag.status === 'beantwoord').length,
    },
    werkpakketten: {
      totaal: werkpakketten.length,
      // Hetzelfde vocabulaire als een vraagstatus, dus dezelfde afbeelding
      // van '' naar 'geen'; geschreven zonder de vraag-helper, want dit is
      // de stand van een werkpakket.
      perStatus: tel(werkpakketten.map((w) => w.data.onderzoek || GEEN_STATUS)),
    },
  };
}

/**
 * Fail the build on an onderzoeksvraag whose structure points nowhere.
 *
 * Three things, all about ids, because an id is the one thing another vraag
 * can hold on to: a duplicate id (the anchor `vraag-<id>` would then be two
 * elements on the overzicht, and a verwant would silently land on one of
 * them), a verwant to an id no (deel)vraag has, and a verwant on a vraag
 * without an id of its own (the other side could never point back, so the
 * relation would render one way round and read as if it were written that
 * way).
 *
 * Not checked, on purpose: an empty status, a missing doel, a vraag without
 * an id. That is the state of the work, not a defect; see the skill.
 */
export function assertOnderzoeksvragen(
  werkpakketten: { data: WerkpakketData }[],
): void {
  const alle = alleOnderzoeksvragen(werkpakketten);
  const problems: string[] = [];
  const waar = (v: VraagInWerkpakket) =>
    `werkpakket ${v.werkpakket.id} (${v.werkpakket.titel}): ${v.plek}`;
  const kort = (v: VraagInWerkpakket) => `"${v.vraag.vraag.slice(0, 60)}…"`;

  const gezien = new Map<string, VraagInWerkpakket>();
  for (const v of alle) {
    const id = v.vraag.id;
    if (!id) {
      if (v.vraag.verwant.length) {
        problems.push(
          `${waar(v)} ${kort(v)} heeft verwant maar geen eigen id; zonder id ` +
            'kan de andere kant niet terugwijzen',
        );
      }
      continue;
    }
    const eerder = gezien.get(id);
    if (eerder) {
      problems.push(
        `${waar(v)} heeft id "${id}", maar ${waar(eerder)} ook; een id moet ` +
          'uniek zijn over de hele roadmap, deelvragen meegerekend',
      );
    } else {
      gezien.set(id, v);
    }
  }

  /*
   * De ids van stories delen de ruimte van de vragen niet (hun anker is
   * `story-<id>`), maar wel die van elkaar: twee stories met hetzelfde id
   * zouden hetzelfde anker dragen, en de tweede is dan onvindbaar.
   */
  const storyIds = new Map<string, string>();
  for (const v of alle) {
    v.vraag.stories.forEach((story, i) => {
      if (!story.id) return;
      const hier = `${waar(v)}, story ${i + 1}`;
      const eerder = storyIds.get(story.id);
      if (eerder) {
        problems.push(
          `${hier} heeft id "${story.id}", maar ${eerder} ook; een story-id ` +
            'moet uniek zijn over de hele roadmap',
        );
      } else {
        storyIds.set(story.id, hier);
      }
    });
  }

  for (const v of alle) {
    for (const doel of v.vraag.verwant) {
      if (doel === v.vraag.id) {
        problems.push(`${waar(v)} ${kort(v)} noemt zichzelf in verwant`);
      } else if (!gezien.has(doel)) {
        problems.push(
          `${waar(v)} ${kort(v)} verwijst via verwant naar "${doel}", maar ` +
            'geen enkele (deel)vraag heeft dat id',
        );
      }
    }
  }

  if (problems.length) {
    throw new Error(`Onderzoeksvragen kloppen niet:\n  ${problems.join('\n  ')}`);
  }
}

/** An RFC a werkpakket points at, with the RFC's own implementation state. */
export interface RfcVerwijzing {
  /** Zero-padded id, e.g. "RFC-013". */
  id: string;
  /** The RFC's own English title; a name, not a label, so not translated. */
  title: string;
  /** The RFC's `implementation` value, in Dutch — this page is lang="nl". */
  implementation: string;
  link: string;
}

/*
 * The RFC's implementation state, in Dutch.
 *
 * The RFC pages are lang="en" and show the frontmatter value as it stands; a
 * werkpakket page is lang="nl", and an English string there is not only
 * inconsistent but read aloud with Dutch pronunciation rules. The RFC's own
 * title stays English: that is its name, and translating it would stop
 * matching the page the link goes to.
 *
 * An unknown value falls through unchanged rather than being dropped, so a new
 * state added to the RFC schema shows up as itself instead of disappearing.
 */
const IMPLEMENTATIE_NL: Record<string, string> = {
  Implemented: 'Gebouwd',
  'Partially implemented': 'Deels gebouwd',
  'Not implemented': 'Niet gebouwd',
};

/**
 * The RFCs a werkpakket points at, resolved against the RFC collection.
 *
 * The implementation state is read from the RFC and never copied into the
 * werkpakket: the RFC is the thing that gets built, so it owns that fact. A
 * copy would be a second truth that nobody updates.
 */
export function rfcVerwijzingen(nummers: number[]): RfcVerwijzing[] {
  const alle = new Map(getRfcs().map((r) => [r.num, r]));
  // Deduped: the same number twice used to render two identical rows, and the
  // build said nothing. Harmless to write by accident, invisible once shipped.
  return [...new Set(nummers)]
    .map((n) => alle.get(n))
    .filter((r): r is NonNullable<typeof r> => Boolean(r))
    .map((r) => ({
      id: r.id,
      title: r.title,
      implementation:
        IMPLEMENTATIE_NL[r.implementation ?? ''] ??
        r.implementation ??
        'Niet gebouwd',
      link: r.link,
    }));
}

/**
 * Fail the build on a werkpakket pointing at an RFC that does not exist.
 *
 * Same reason as assertPaperSections: check-links.mjs would catch the dead
 * link once it is in the HTML, but it reports the route, not the werkpakket
 * that wrote it.
 */
export function assertRfcReferences(
  werkpakketten: { data: WerkpakketData }[],
): void {
  const bestaande = new Set(getRfcs().map((r) => r.num));
  const problems: string[] = [];

  for (const { data } of werkpakketten) {
    for (const nummer of data.rfcs) {
      if (bestaande.has(nummer)) continue;
      problems.push(
        `werkpakket ${data.id} (${data.titel}): RFC ${nummer} bestaat niet`,
      );
    }
  }

  if (problems.length) {
    throw new Error(
      `Verwijzingen naar RFC's kloppen niet:\n  ${problems.join('\n  ')}`,
    );
  }
}

/**
 * Fail the build on a belegging that contradicts the rest of the werkpakket.
 *
 * The schema in content.config.ts already enforces what holds inside the
 * object itself (a date under 'opgepakt' and 'klaar', neither outside them).
 * What it cannot see from there is the sibling fields, and two combinations
 * are contradictions rather than incompleteness:
 *
 * - 'vrij' on a werkpakket whose question is answered and whose build is
 *   done. The card would invite someone to pick up work that is finished.
 *   A blank field counts as 'vrij' here, the same as everywhere else — see
 *   getBelegging(). Checking the raw string instead would let exactly the
 *   common case through: forty-six of the forty-nine leave the field blank,
 *   so a werkpakket that quietly finishes while nobody updates its belegging
 *   is precisely the one this rule is for.
 * - a `sinds` in the future. That is a typo (2062 for 2026), and sindsTekst()
 *   would render it as a negative age.
 *
 * Staleness is deliberately not checked here; see check-roadmap-belegging.mjs
 * for why that reports rather than blocks.
 */
export function assertBelegging(
  werkpakketten: { data: WerkpakketData }[],
  nu = new Date(),
): void {
  const vandaag = nu.toISOString().slice(0, 10);
  const problems: string[] = [];

  for (const { data } of werkpakketten) {
    const { sinds } = data.belegging;
    const stand = beleggingStand(data.belegging.stand);

    if (
      stand === 'vrij' &&
      data.onderzoek === 'beantwoord' &&
      data.bouw === 'wel'
    ) {
      problems.push(
        `werkpakket ${data.id} (${data.titel}): de belegging staat op 'vrij' ` +
          '(of is leeg, wat hetzelfde betekent) terwijl onderzoek beantwoord ' +
          'en bouw wel is; de kaart zou uitnodigen tot werk dat af is. Zet ' +
          "belegging.stand op 'klaar'.",
      );
    }

    if (sinds && sinds > vandaag) {
      problems.push(
        `werkpakket ${data.id} (${data.titel}): belegging.sinds (${sinds}) ` +
          `ligt na vandaag (${vandaag})`,
      );
    }
  }

  if (problems.length) {
    throw new Error(`De belegging klopt niet:\n  ${problems.join('\n  ')}`);
  }
}

/**
 * How long ago, in words.
 *
 * A frontmatter field cannot expire, so the page is what has to make its age
 * visible: `sinds 2024-03-11` reads as metadata, `sinds 18 maanden` reads as a
 * question. Computed at build time, which is accurate enough — the site
 * rebuilds on every merge.
 *
 * Months, not days: the unit of this roadmap is a quarter, and "sinds 3 dagen"
 * is noise. Anything under a month is "deze maand" rather than "0 maanden".
 */
export function sindsTekst(isoDatum: string, nu = new Date()): string {
  const toen = new Date(`${isoDatum}T00:00:00Z`);
  const maanden =
    (nu.getUTCFullYear() - toen.getUTCFullYear()) * 12 +
    (nu.getUTCMonth() - toen.getUTCMonth()) -
    (nu.getUTCDate() < toen.getUTCDate() ? 1 : 0);

  if (maanden < 1) return 'deze maand';
  if (maanden === 1) return '1 maand';
  return `${maanden} maanden`;
}

/**
 * The implemented RFCs no werkpakket points at.
 *
 * Reported, not enforced. An RFC that lands before anyone updates the roadmap
 * is a redactional gap, not a defect, and a gate that blocked the RFC on it
 * would put the roadmap in the way of the work it describes. Printing the
 * list at build time keeps it visible without that cost.
 */
export function ongekoppeldeRfcs(
  werkpakketten: { data: WerkpakketData }[],
): RfcVerwijzing[] {
  const gekoppeld = new Set(werkpakketten.flatMap((w) => w.data.rfcs));
  return getRfcs()
    .filter((r) => r.implementation === 'Implemented' && !gekoppeld.has(r.num))
    .map((r) => ({
      id: r.id,
      title: r.title,
      implementation: r.implementation ?? '',
      link: r.link,
    }));
}

/**
 * Fail the build on a research question pointing at a paper section that does
 * not exist.
 *
 * check-links.mjs does catch a dead anchor once the link is in the HTML, but
 * it reports the route and the anchor, not which werkpakket wrote it — with
 * some 150 questions that is a search. This names the file and the question
 * instead, and it is what keeps the mapping honest when the paper is revised:
 * drop a section and the build says which werkpakket pointed at it.
 *
 * Deelvragen count too: a deelvraag may carry its own `paper`.
 */
export function assertPaperSections(
  werkpakketten: { data: WerkpakketData; id?: string }[],
): void {
  const problems: string[] = [];

  for (const { data } of werkpakketten) {
    data.onderzoeksvragen.forEach((vraag, i) => {
      const items: { v: DeelvraagData; plek: string }[] = [
        { v: vraag, plek: `vraag ${i + 1}` },
        ...(typeof vraag === 'string'
          ? []
          : vraag.deelvragen.map((d, j) => ({
              v: d,
              plek: `vraag ${i + 1}, deelvraag ${j + 1}`,
            }))),
      ];
      for (const { v, plek } of items) {
        if (typeof v === 'string' || !v.paper || paperSecties.has(v.paper)) {
          continue;
        }
        problems.push(
          `werkpakket ${data.id} (${data.titel}): onbekende papersectie ` +
            `"${v.paper}" bij ${plek} "${v.vraag.slice(0, 60)}…"`,
        );
      }
    });
  }

  if (problems.length) {
    throw new Error(
      `Verwijzingen naar het position paper kloppen niet:\n  ${problems.join(
        '\n  ',
      )}`,
    );
  }
}

/**
 * Fail the build on a reference that goes nowhere.
 *
 * check-links.mjs only reads <a href>, and every link the roadmap renders sits
 * on an NLDD component attribute (nldd-card, nldd-list-item), which that gate
 * cannot see. Without this the samenhang links would be the one part of the
 * site where a dangling reference ships unnoticed — and the app relied on its
 * server to keep them consistent, which is exactly what we removed.
 *
 * zod cannot do this: a per-entry schema never sees its sibling entries, nor
 * the fase/discipline JSON.
 */
export function assertReferencesResolve(
  werkpakketten: { data: WerkpakketData; id?: string }[],
): void {
  const ids = new Set(werkpakketten.map((w) => w.data.id));
  const problems: string[] = [];

  /*
   * Two files carrying the same id would otherwise collapse into one Set
   * entry and pass unnoticed, while getStaticPaths emits the route twice:
   * Astro drops the second with a warning, the build still succeeds, and one
   * werkpakket renders a card that links to another one's page. No gate
   * catches it — check-links.mjs sees a link that resolves.
   *
   * This is the likelier mistake now that there is no write path: a new
   * werkpakket starts as a copy of an existing file, and the filename is the
   * part you remember to change.
   */
  const seen = new Set<string>();
  for (const { data, id: bestandsnaam } of werkpakketten) {
    if (seen.has(data.id)) {
      problems.push(`id "${data.id}" wordt door meer dan één bestand gebruikt`);
    }
    seen.add(data.id);
    // The filename is the werkpakket's id; keeping the two equal is what makes
    // the content directory navigable — with slugs that is the whole point of
    // it, since `ls` then reads as a list of werkpakketten.
    if (bestandsnaam !== undefined && bestandsnaam !== data.id) {
      problems.push(
        `bestand "${bestandsnaam}.md" bevat id "${data.id}"; die horen gelijk te zijn`,
      );
    }
  }

  for (const { data } of werkpakketten) {
    const waar = `werkpakket ${data.id} (${data.titel})`;
    if (!getFase(data.faseId)) {
      problems.push(`${waar}: onbekende faseId "${data.faseId}"`);
    }
    if (!getDiscipline(data.disciplineId)) {
      problems.push(`${waar}: onbekende disciplineId "${data.disciplineId}"`);
    }
    for (const samenhangId of data.samenhangIds) {
      if (!ids.has(samenhangId)) {
        problems.push(`${waar}: samenhangId "${samenhangId}" bestaat niet`);
      }
    }
    for (const afhankelijkheid of data.afhankelijkVan) {
      if (!ids.has(afhankelijkheid)) {
        problems.push(
          `${waar}: afhankelijkVan "${afhankelijkheid}" bestaat niet`,
        );
      }
      if (afhankelijkheid === data.id) {
        problems.push(`${waar}: afhankelijkVan wijst naar zichzelf`);
      }
    }
  }

  /*
   * A cycle means none of the werkpakketten in it can ever start, which is a
   * statement about the plan and not about the file it was written in. The
   * build says which ones, in the order it walked them, because the fix is a
   * judgement about which of those arrows is the wrong one.
   */
  const kleur = new Map<string, 'bezig' | 'klaar'>();
  const pad: string[] = [];
  const titel = new Map(werkpakketten.map(({ data }) => [data.id, data.titel]));
  const afhankelijkheden = new Map(
    werkpakketten.map(({ data }) => [data.id, data.afhankelijkVan]),
  );
  const loop = (id: string) => {
    if (kleur.get(id) === 'klaar') return;
    if (kleur.get(id) === 'bezig') {
      const kring = [...pad.slice(pad.indexOf(id)), id]
        .map((stap) => titel.get(stap) ?? stap)
        .join(' → ');
      problems.push(`afhankelijkheden lopen rond: ${kring}`);
      return;
    }
    kleur.set(id, 'bezig');
    pad.push(id);
    for (const volgende of afhankelijkheden.get(id) ?? []) {
      // Een zelfverwijzing is hierboven al gemeld, en met zijn eigen naam.
      // Hem hier nog eens als kring van één melden ("lopen rond: X → X") maakt
      // van één fout twee regels, waarvan de tweede minder zegt dan de eerste.
      if (volgende !== id && ids.has(volgende)) loop(volgende);
    }
    pad.pop();
    kleur.set(id, 'klaar');
  };
  for (const { data } of werkpakketten) loop(data.id);

  if (problems.length) {
    throw new Error(
      `Roadmap-verwijzingen kloppen niet:\n  ${problems.join('\n  ')}`,
    );
  }
}
