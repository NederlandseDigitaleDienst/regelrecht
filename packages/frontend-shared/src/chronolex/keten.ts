/**
 * The chain of a decision in words: the trace of the engine as the
 * chronolexografie kern condenses it (`kern::Stap`, in `waarom.keten`), as a
 * tree of steps. Shared by the poc (packages/poc-chronolexografie/frontend)
 * and the wetsprocessysteem (frontend-wetsproces); each app says how it names
 * a regelwerk and a field, and where a regelwerk sits in its corpus.
 *
 * A step has a `soort`: `artikel` (an article runs), `hook` (an article that
 * runs at a stage of the decision), `uitkomst` (an output of the article) or
 * `begrip` (an open term, with who filled it in).
 */
import { leesbaar, toon } from './waarden';

/** Who filled in an open term (`FilledBy` of the engine). */
export type IngevuldDoor =
  | 'finding'
  | { regulation: { law_id: string; article: string } }
  | { answer: { law_id: string; article: string; answers: unknown[] } }
  | null
  | undefined;

/** One step of the chain (`kern::Stap`). */
export interface Stap {
  soort: 'artikel' | 'hook' | 'uitkomst' | 'begrip' | string;
  wet?: string;
  artikel?: string;
  uitvoer?: string;
  naam?: string;
  waarde?: unknown;
  ontbreekt?: string[];
  ingevuld_door?: IngevuldDoor;
  stappen?: Stap[];
}

/** A step with a key (`nummer`, `1.2.3`) and the steps under it. */
export interface Knoop {
  nummer: string;
  stap: Stap;
  kinderen: Knoop[];
}

/**
 * The chain as a tree. Each step gets a `nummer` (`1`, `1.2`, `1.2.3`) that
 * serves as its key; the chain does not show it.
 */
export function genummerd(stappen: Stap[], prefix = ''): Knoop[] {
  return stappen.map((stap, i) => {
    const nummer = prefix ? `${prefix}.${i + 1}` : `${i + 1}`;
    return { nummer, stap, kinderen: genummerd(stap.stappen ?? [], nummer) };
  });
}

/** How an app names what the chain names. */
export interface Namen {
  /** A regelwerk by its `$id`. */
  wet: (wet: string) => string;
  /** An article of a regelwerk. */
  artikel: (wet: string, artikel: string) => string;
  /** An output, a term or a fact by its name; by default readable (`leesbaar`). */
  veld?: (naam: string) => string;
}

/** A step of the chain in words: a line of text and an explanation. */
export function stapTekst(stap: Stap, namen: Namen): { tekst: string; toelichting?: string } {
  const veld = namen.veld ?? leesbaar;
  switch (stap.soort) {
    case 'artikel':
      return stap.uitvoer
        ? { tekst: `${veld(stap.uitvoer)} uit ${namen.wet(stap.wet ?? '')}` }
        : { tekst: `${namen.artikel(stap.wet ?? '', stap.artikel ?? '')} wordt uitgevoerd` };
    case 'hook':
      return { tekst: `Ook van toepassing: ${namen.artikel(stap.wet ?? '', stap.artikel ?? '')}` };
    case 'uitkomst':
      return stap.ontbreekt?.length
        ? {
            tekst: `${veld(stap.naam ?? '')}: niet bepaald`,
            toelichting: `ontbreekt: ${stap.ontbreekt.map(veld).join(', ')}`,
          }
        : { tekst: `${veld(stap.naam ?? '')}: ${toon(stap.waarde)}` };
    // Who filled the term in shows as a tag (`herkomstVan`), not as a sentence.
    case 'begrip':
      return { tekst: `${veld(stap.naam ?? '')}: ${toon(stap.waarde)}` };
    default:
      return { tekst: stap.soort };
  }
}

/** Where a regelwerk sits: law, policy, or a reduction of a cell. */
export type Laag = 'wet' | 'beleid' | 'reductie';

/** The kinds of origin of a step, as a tag: text and colour. */
export const SOORTEN = Object.freeze({
  wet: { tekst: 'wet', kleur: 'lintblauw' },
  beleid: { tekst: 'beleid', kleur: 'paars' },
  reductie: { tekst: 'reductie', kleur: 'groen' },
  bevinding: { tekst: 'bestuursorgaan', kleur: 'neutral' },
});

/**
 * Whether the regelwerk in directory `map` of the corpus is policy, a
 * reduction or law: policy sits under `uitvoeringsbeleid`, a reduction with
 * a cell. Everything else (wet, AMvB, regeling, verordening) counts as law.
 */
export function laagVanMap(map: string): Laag {
  if (map.includes('/uitvoeringsbeleid/')) return 'beleid';
  if (map.startsWith('cells/')) return 'reductie';
  return 'wet';
}

/** What filled in a step: an article, the answers of cells, or a finding. */
export type Herkomst =
  | { soort: Laag; wet: string; artikel: string }
  | { soort: 'reductie'; begrip: string; waarde: unknown; antwoorden: unknown[] }
  | { soort: 'bevinding' };

/**
 * The origin of a step of the chain, or null when it has none (an outcome, a
 * default value): an article, the answers of cells with the term they filled
 * in and its value, or a finding of the bestuursorgaan. `laag` says where a
 * regelwerk sits.
 */
export function herkomstVan(stap: Stap, laag: (wet: string) => Laag): Herkomst | null {
  if (stap.soort === 'artikel' || stap.soort === 'hook') {
    return stap.artikel && stap.wet ? { soort: laag(stap.wet), wet: stap.wet, artikel: stap.artikel } : null;
  }
  if (stap.soort !== 'begrip') return null;
  const by = stap.ingevuld_door;
  if (by === 'finding') return { soort: 'bevinding' };
  if (by && typeof by === 'object' && 'regulation' in by) {
    const { law_id: wet, article: artikel } = by.regulation;
    return { soort: laag(wet), wet, artikel };
  }
  if (by && typeof by === 'object' && 'answer' in by && by.answer.answers.length) {
    return { soort: 'reductie', begrip: stap.naam ?? '', waarde: stap.waarde, antwoorden: by.answer.answers };
  }
  return null;
}

/** Whether a click on the origin has something to show. */
export const klikbaar = (h: Herkomst | null): boolean => !!h && h.soort !== 'bevinding';
