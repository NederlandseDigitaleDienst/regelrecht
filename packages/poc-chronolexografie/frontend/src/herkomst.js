/**
 * What filled in a line of the chain of a decision (the law, policy or a
 * reduction), and what a click on that line shows: the text of the article,
 * or the chronolexograms the reduction read. One sheet shows it
 * (HerkomstSheet.vue); a line opens it with `toonHerkomst`.
 *
 * Whether a regelwerk is law or policy follows from its directory in the
 * corpus (`regelwerken` of the case): policy sits under `uitvoeringsbeleid`, a
 * reduction with a cell. Everything else (wet, AMvB, regeling, verordening)
 * counts as law here. The text of an article comes from the files the cell
 * loads too: the bundle, or the source of the case (api.js).
 *
 * A chronolexogram opens the same sheet with the article it rests on
 * (`herkomstVanBasis`, ArtikelLink.vue).
 */
import { ref } from 'vue';
import { load } from 'js-yaml';
import { bestanden, opgehaaldeBronnen } from './api.js';
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
 * 'reductie', begrip, waarde, antwoorden }` for the answers of cells, with
 * the term they filled in and its value, and `{ soort: 'bevinding' }` for a
 * finding of the bestuursorgaan.
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
  if (by?.answer?.answers.length) {
    return { soort: 'reductie', begrip: stap.naam, waarde: stap.waarde, antwoorden: by.answer.answers };
  }
  return null;
}

/**
 * The article a chronolexogram rests on, its `basis` (`<regelwerk>#<artikel>`),
 * as an origin: the article that establishes an application, or the one that
 * took a decision. Null for a basis that names no article.
 */
export function herkomstVanBasis(basis) {
  const [wet, artikel] = String(basis ?? '').split('#');
  return wet && artikel ? { soort: laag(wet), wet, artikel } : null;
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
 * The files the case runs on, by path: the bundle, or for a case from a
 * source the files of that source, which the cell loads too (api.js).
 */
async function bestandenVanCasus() {
  const bron = state.casus?.bron;
  if (bron) {
    const b = (await opgehaaldeBronnen()).find((x) => x.naam === bron);
    if (b?.fout) throw new Error(b.fout);
    return b?.bestanden ?? {};
  }
  bundel ??= bestanden().catch((e) => {
    bundel = null;
    throw e;
  });
  return bundel;
}

/**
 * Article `artikel` of the regelwerk `wet`, in the version valid on `op` (the
 * play date when not given): `{ tekst, machineReadable }`, its text and its
 * `machine_readable` as written in the YAML (`machineReadable`), either null
 * when the article has none. Null when the files of the case do not hold the
 * article. Before its first version, the first one. A case from a
 * source reads the files of that source, not the bundle, whose regelwerken
 * may differ. The `map` of a regelwerk is relative to the root of the corpus,
 * the directory that holds `casussen/<id>.yaml`; the bundle keys a file by
 * its path in the repository, a source by its path in the source.
 */
export async function artikelInhoud(wet, artikel, op = state.datum) {
  const alle = await bestandenVanCasus();
  const map = mapVan(wet);
  const casus = `casussen/${state.casus?.id}.yaml`;
  const wortel = Object.keys(alle).find((p) => p === casus || p.endsWith(`/${casus}`))?.slice(0, -casus.length);
  if (!map || wortel == null) return null;
  const versies = Object.keys(alle)
    .filter((p) => p.startsWith(`${wortel}${map}/`) && p.endsWith('.yaml'))
    .sort();
  const datum = (p) => p.split('/').at(-1).replace('.yaml', '');
  const pad = versies.filter((p) => datum(p) <= op).at(-1) ?? versies[0];
  if (!pad) return null;
  const yaml = alle[pad];
  const gevonden = load(yaml)?.articles?.find((a) => String(a.number) === String(artikel));
  if (!gevonden) return null;
  return { tekst: gevonden.text ?? null, machineReadable: machineReadableVan(yaml, artikel) };
}

/** The indentation of a line, or null for a blank line. */
const inspringing = (regel) => (regel.trim() ? regel.length - regel.trimStart().length : null);

/**
 * The `machine_readable` of article `artikel` as it stands in the YAML text
 * `yaml`, comments included, its key on the first line and dedented to it;
 * null when the article has none. The text is cut, not parsed: a parse
 * drops the comments. An article is an item of `articles` that opens with
 * its `number`, as every regelwerk of the corpus writes it; the comment
 * lines right above the key belong to it.
 */
export function machineReadableVan(yaml, artikel) {
  const regels = yaml.split('\n');
  const nummer = /^(\s*)- number:\s*(['"]?)(.*?)\2\s*(#.*)?$/;
  // The items of `articles` share the indentation of its first one; a
  // `- number:` deeper in an article is no article.
  const lijst = regels.findIndex((r) => /^articles:\s*(#.*)?$/.test(r));
  const eerste = regels.findIndex((r, i) => i > lijst && nummer.test(r));
  if (lijst < 0 || eerste < 0) return null;
  const item = nummer.exec(regels[eerste])[1].length;
  const begin = regels.findIndex(
    (r, i) => i >= eerste && nummer.exec(r)?.[1].length === item && nummer.exec(r)[3] === String(artikel),
  );
  if (begin < 0) return null;
  let eind = regels.findIndex((r, i) => i > begin && inspringing(r) !== null && inspringing(r) <= item);
  if (eind < 0) eind = regels.length;
  const sleutel = regels.findIndex(
    (r, i) => i > begin && i < eind && inspringing(r) === item + 2 && /^\s*machine_readable:\s*(#.*)?$/.test(r),
  );
  if (sleutel < 0) return null;
  let van = sleutel;
  while (van - 1 > begin && inspringing(regels[van - 1]) === item + 2 && regels[van - 1].trim().startsWith('#')) van -= 1;
  // A comment does not end the block, whatever its indentation; the comments
  // and blank lines right above the next key do not belong to it.
  const commentaar = (r) => r.trim().startsWith('#');
  let tot = regels.findIndex(
    (r, i) => i > sleutel && i < eind && inspringing(r) !== null && inspringing(r) <= item + 2 && !commentaar(r),
  );
  if (tot < 0) tot = eind;
  while (tot > sleutel + 1 && (!regels[tot - 1].trim() || commentaar(regels[tot - 1]))) tot -= 1;
  return regels
    .slice(van, tot)
    .map((r) => r.slice(Math.min(item + 2, inspringing(r) ?? item + 2)))
    .join('\n');
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
