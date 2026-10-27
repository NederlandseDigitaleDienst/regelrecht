/**
 * The chain of a case in words (tab Keten). The game builds the chain from
 * the regelwerken the case loads (`api.keten`, src/keten.rs); this file puts
 * each step into a line of text.
 *
 * A step has a `soort`: `artikel` (an article as the case runs it), `begrip`
 * (an open term with the articles that fill it), `cel` (the cell that gives a
 * policy its answer), `kroniek` (what a reduction reads from the chronicle of
 * a cell) or `eerder` (an article that stands higher up in the chain).
 */
import { leesbaar } from './state.js';
import { artikelNaam, celNaam, opsomming, wetNaam } from './logboek.js';

// The layers as a visitor reads them; another layer reads as its own name.
const LAGEN = {
  WET: 'wet',
  AMVB: 'AMvB',
  MINISTERIELE_REGELING: 'ministeriële regeling',
  GEMEENTELIJKE_VERORDENING: 'verordening',
  UITVOERINGSBELEID: 'beleid',
  REDUCTIE: 'reductie van een cel',
};

/** The layer of a regelwerk in words: `UITVOERINGSBELEID` → `beleid`. */
export const laagNaam = (laag) => LAGEN[laag] ?? leesbaar(String(laag).toLowerCase());

/** `wet#artikel` as `Wet artikel n`; a fact (`wet#artikel#feit`) with its name in front. */
export function verwijzing(ref) {
  const [wet, artikel, feit] = String(ref).split('#');
  if (!artikel) return ref;
  const naam = artikelNaam(wet, artikel);
  return feit ? `${leesbaar(feit)} (${naam})` : naam;
}

/**
 * The first sentence of an article's text that says what the article does:
 * without the sentence on what a fictional article is modeled on
 * ("FICTIEF, naar het model van …") and without the "Geen regelgeving." of a
 * reduction, since the step carries both as a label, and with only the text
 * of a markdown link or reference. A lid number in front goes; a long sentence
 * is cut at a word boundary.
 */
export function korteUitleg(tekst, max = 180) {
  if (!tekst) return '';
  const zinnen = String(tekst)
    // A markdown link or reference keeps only its text: `[artikel 11][ref1]`.
    .replace(/\[([^\]]+)\](\[[^\]]*\]|\([^)]*\))/g, '$1')
    .replace(/\s+/g, ' ')
    .trim()
    .split(/(?<=[.:;])\s+(?=[A-Z0-9])/);
  const zin = zinnen.find((z) => !/^FICTIEF\b/.test(z) && z !== 'Geen regelgeving.') ?? '';
  const kaal = zin.replace(/^\d+[a-z]?\s+/, '');
  if (kaal.length <= max) return kaal;
  const knip = kaal.slice(0, max);
  const spatie = knip.lastIndexOf(' ');
  return `${spatie > 0 ? knip.slice(0, spatie) : knip} …`;
}

/** A key for a step among its siblings that stays with the step when the tree changes. */
export const schakelSleutel = (s, i) =>
  [s.soort, s.wet, s.artikel, s.id, s.cel, s.antwoord, s.besluiten_onder, s.executogrammen_van, i]
    .filter((x) => x != null)
    .join('|');

/** Whether a step has steps under it to open. */
export const heeftKinderen = (s) => (s.kinderen?.length ?? 0) > 0;

/**
 * A step of the chain in words: a label above it (`overline`), a line of text
 * and an explanation.
 */
export function schakelTekst(s) {
  switch (s.soort) {
    case 'artikel': {
      const toelichting = [
        s.leest?.length ? `geeft ${opsomming(s.leest.map(leesbaar))}` : '',
        ...Object.entries(s.parameters ?? {}).map(([k, v]) => `${leesbaar(k)}: ${v}`),
      ].filter(Boolean);
      return {
        overline: laagNaam(s.laag),
        // The name of the version in force, which a later version may change.
        tekst: `${wetNaam(s.wet, s.naam)} artikel ${s.artikel}`,
        toelichting: toelichting.join(' · ') || korteUitleg(s.tekst),
      };
    }
    case 'begrip': {
      const n = s.kinderen.length;
      const toelichting = n
        ? `ingevuld door ${n === 1 ? 'het artikel' : `${n} artikelen`} hieronder`
        : s.ingevuld_door
          ? `wie beslist vult het in: ${s.ingevuld_door}`
          : 'geen regelwerk van deze casus vult het in';
      return { overline: 'open begrip', tekst: leesbaar(s.id), toelichting };
    }
    case 'cel':
      return s.cel
        ? {
            overline: 'antwoord van een cel',
            tekst: `${celNaam(s.cel)}: ${leesbaar(s.antwoord)}`,
            toelichting: `uit de reductie ${s.reductie} (cel.yaml, answers)`,
          }
        : {
            overline: 'antwoord van een cel',
            tekst: leesbaar(s.antwoord),
            toelichting: 'geen cel van deze casus geeft dit antwoord, dus het beleid krijgt een lege waarde',
          };
    case 'kroniek':
      return {
        overline: 'uit de kroniek',
        tekst: s.besluiten_onder
          ? `leest de besluiten onder ${verwijzing(s.besluiten_onder)}`
          : `leest de executogrammen van ${verwijzing(s.executogrammen_van)}`,
        toelichting: `uit de kroniek van ${celNaam(s.cel)}`,
      };
    case 'eerder':
      return {
        overline: 'zie hierboven',
        tekst: artikelNaam(s.wet, s.artikel),
        toelichting: 'dit artikel staat al hoger in de keten, met de stappen eronder',
      };
    default:
      return { overline: '', tekst: s.soort, toelichting: '' };
  }
}

/** The path of a file as the repository names it: under `corpus/` for a case of regelrecht itself. */
export const bestandPad = (bestand, bron) => (bestand ? (bron ? bestand : `corpus/${bestand}`) : '');
