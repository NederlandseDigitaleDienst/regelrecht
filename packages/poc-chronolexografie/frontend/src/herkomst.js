/**
 * What filled in a line of the chain of a decision (the law, policy or a
 * reduction), and what a click on that line shows: the text of the article,
 * or the chronolexograms the reduction read. One sheet shows it
 * (HerkomstSheet.vue); a line opens it with `toonHerkomst`.
 *
 * Whether a regelwerk is law or policy follows from its directory in the
 * corpus (`regelwerken` of the case): policy sits under `uitvoeringsbeleid`, a
 * reduction with a cell. Everything else (wet, AMvB, regeling, verordening)
 * counts as law here. The text of an article comes from the bundle the cell
 * loads too (api.js).
 */
import { ref } from 'vue';
import { load } from 'js-yaml';
import { bestanden } from './api.js';
import { grammenVan } from './logboek.js';
import { state } from './state.js';

/** The kinds of origin, as a tag: text and colour. */
export const SOORTEN = Object.freeze({
  wet: { tekst: 'wet', kleur: 'lintblauw' },
  beleid: { tekst: 'beleid', kleur: 'paars' },
  reductie: { tekst: 'reductie', kleur: 'groen' },
  bevinding: { tekst: 'bestuursorgaan', kleur: 'neutral' },
});

const mapVan = (wet) => state.casus?.regelwerken?.find((r) => r.id === wet)?.map ?? '';

/** Whether the regelwerk `wet` is policy, a reduction or law. */
export function laag(wet) {
  const map = mapVan(wet);
  if (map.includes('/uitvoeringsbeleid/')) return 'beleid';
  if (map.startsWith('cells/')) return 'reductie';
  return 'wet';
}

/**
 * The origin of a step of the chain, or null when it has none (an outcome, a
 * default value): `{ soort, wet, artikel }` for an article, `{ soort:
 * 'reductie', antwoorden }` for the answers of cells, and `{ soort:
 * 'bevinding' }` for a finding of the bestuursorgaan.
 */
export function herkomstVan(stap) {
  if (stap.soort === 'artikel' || stap.soort === 'hook') {
    return stap.artikel ? { soort: laag(stap.wet), wet: stap.wet, artikel: stap.artikel } : null;
  }
  if (stap.soort !== 'begrip') return null;
  const by = stap.ingevuld_door;
  if (by === 'finding') return { soort: 'bevinding' };
  if (by?.regulation) {
    const { law_id: wet, article: artikel } = by.regulation;
    return { soort: laag(wet), wet, artikel };
  }
  if (by?.answer?.answers.length) return { soort: 'reductie', antwoorden: by.answer.answers };
  return null;
}

/** Whether a click on the origin has something to show. */
export const klikbaar = (h) => !!h && h.soort !== 'bevinding';

/** The origin the sheet shows, or null. */
export const getoond = ref(null);
export const toonHerkomst = (h) => {
  getoond.value = h;
};

/**
 * The chronolexograms a cell read for its answer (`AnswerProvenance`): its
 * chronicle up to and including the last one it read. The provenance carries
 * the count and the last id, not the list; a reading that skipped a gram
 * (one being decided again) shows that gram here anyway.
 */
export function gelezenGrammen(antwoord) {
  const kroniek = grammenVan(antwoord.answerer, state.grammen);
  const tot = kroniek.findIndex((g) => g.id === antwoord.last_record);
  return tot < 0 ? [] : kroniek.slice(0, tot + 1);
}

let bundel = null;

/**
 * The text of article `artikel` of the regelwerk `wet`, in the version valid
 * on the play date, or null when the bundle does not hold it.
 */
export async function artikelTekst(wet, artikel) {
  bundel ??= bestanden().catch((e) => {
    bundel = null;
    throw e;
  });
  const alle = await bundel;
  const map = mapVan(wet);
  if (!map) return null;
  const versies = Object.keys(alle)
    .filter((p) => p.startsWith(`corpus/${map}/`) && p.endsWith('.yaml'))
    .sort();
  const datum = (p) => p.split('/').at(-1).replace('.yaml', '');
  const pad = versies.filter((p) => datum(p) <= state.datum).at(-1) ?? versies.at(-1);
  if (!pad) return null;
  const gevonden = load(alle[pad])?.articles?.find((a) => String(a.number) === String(artikel));
  return gevonden?.text ?? null;
}

/**
 * The paragraphs of an article text. A literal block in the YAML separates
 * them with a blank line and wraps inside them; a folded one (`>-`) leaves a
 * single newline between them.
 */
export const leden = (tekst) =>
  (tekst.includes('\n\n') ? tekst.split(/\n\s*\n/) : tekst.split('\n'))
    .map((lid) => lid.replace(/\s*\n\s*/g, ' ').trim())
    .filter(Boolean);
