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
    // Not after an abbreviation such as "art. 13a" or "nr. 5".
    .split(/(?<!\b(?:art|nr|jo|bijv|o\.a|lid|onderdeel)\.)(?<=[.:;])\s+(?=[A-Z0-9])/);
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

/** `tekst` with a capital letter. */
const hoofd = (tekst) => (tekst ? tekst.charAt(0).toUpperCase() + tekst.slice(1) : '');

/** Sentences as one text: the empty ones left out, each ending in a full stop. */
const zinnen = (...delen) =>
  delen
    .filter(Boolean)
    .map((z) => (/[.!?…]$/.test(z) ? z : `${z}.`))
    .join(' ');

/** A parameter value as text: an article reference in words, another as written. */
const parameterWaarde = (v) => (String(v).includes('#') ? verwijzing(v) : String(v));

/**
 * A short heading for an article: a law or AMvB by its name, another layer
 * by the layer ("Verordening art. 2", "Beleid art. 5"), a reduction by what
 * it gives.
 */
function artikelKop(s) {
  if (s.laag === 'REDUCTIE') return `Reductie ${opsomming(s.leest ?? [])}`.trim();
  // The first word of a lower regelwerk's name tells two of one layer apart
  // when it names the kind of regulation ("Uitvoeringsbesluit art. 2",
  // "Parkeerverordening art. 2"); else the layer does ("Beleid art. 1").
  const eerste = s.naam?.split(/\s+/)[0] ?? '';
  const wet = ['WET', 'AMVB'].includes(s.laag)
    ? wetNaam(s.wet, s.naam)
    : /(besluit|verordening|regeling|beleid|regels?)$/i.test(eerste)
      ? eerste
      : hoofd(laagNaam(s.laag));
  return `${wet} art. ${s.artikel}`;
}

/** `dat` for one name, `die` for more: "geeft a, dat …", "geeft a en b, die …". */
const datDie = (namen) => (namen.length > 1 ? 'die' : 'dat');

/** The cells under a step that give `antwoord`, by name. */
const cellenVoor = (s, antwoord) =>
  (s.kinderen ?? []).filter((k) => k.soort === 'cel' && k.cel && k.antwoord === antwoord).map((k) => celNaam(k.cel));

/** The open terms a step leaves open itself, with who judges them. */
const eigenBegrippen = (s) => (s.kinderen ?? []).filter((k) => k.soort === 'begrip');

/**
 * What an article does in the chain, in two or three short sentences, by a
 * fixed pattern per kind: the article that decides the case, an article that
 * fills an open term, a reduction, or an article whose output a step above
 * reads. Only names and text from the YAML fill the pattern.
 */
function artikelUitleg(s, wortel) {
  const eigen = eigenBegrippen(s);
  if (wortel) {
    return zinnen(
      `Dit artikel beslist over de aanvraag en bepaalt ${opsomming(s.leest ?? [])}`,
      korteUitleg(s.tekst, 120),
      eigen.length && `Het laat ${eigen.length === 1 ? 'één open begrip' : `${eigen.length} open begrippen`} over aan andere regels: ${opsomming(eigen.map((b) => b.id))}`,
    );
  }
  if (s.laag === 'REDUCTIE') {
    const mee = Object.entries(s.parameters ?? {}).map(([k, v]) => `${k} = ${parameterWaarde(v)}`);
    return zinnen(
      `Deze reductie leest de kroniek van ${celNaam(s.cel)} en geeft ${opsomming(s.leest ?? [])}`,
      mee.length && `De stap erboven geeft ${opsomming(mee)} mee`,
    );
  }
  if (s.vult) {
    const antwoorden = (s.vraagt ?? []).map((a) => {
      const cellen = cellenVoor(s, a);
      return `het antwoord ${a}${cellen.length ? ` van ${opsomming(cellen)}` : ''}`;
    });
    // Its own open terms that no rule fills, grouped by who judges them.
    const perRechter = new Map();
    for (const b of eigen.filter((b) => b.ingevuld_door && !heeftKinderen(b))) {
      perRechter.set(b.ingevuld_door, [...(perRechter.get(b.ingevuld_door) ?? []), b.id]);
    }
    return zinnen(
      `Dit artikel vult het open begrip ${s.vult} van ${verwijzing(s.vult_in)}`,
      antwoorden.length ? `Het neemt daarvoor ${opsomming(antwoorden)}` : korteUitleg(s.tekst, 120),
      ...[...perRechter].map(
        ([wie, ids]) => `Daarnaast laat het ${opsomming(ids)} open, per geval in te vullen door ${wie}`,
      ),
    );
  }
  return zinnen(
    s.leest?.length && `Dit artikel geeft ${opsomming(s.leest)}, ${datDie(s.leest)} de stap erboven gebruikt`,
    korteUitleg(s.tekst, 120),
  );
}

/**
 * A step of the chain in words: a label above it (`overline`), a short
 * heading (`tekst`), the names that lead to the next step (`toelichting`)
 * and an explanation of two or three sentences (`uitleg`). The names stay as
 * the YAML writes them, so the eye finds them in the panels below.
 */
export function schakelTekst(s, { wortel = false } = {}) {
  switch (s.soort) {
    case 'artikel': {
      const namen = [
        s.vult ? `vult ${s.vult}` : '',
        s.vraagt?.length ? `vraagt antwoord ${opsomming(s.vraagt)}` : '',
        s.leest?.length ? `geeft ${opsomming(s.leest)}` : '',
        ...Object.entries(s.parameters ?? {}).map(([k, v]) => `${leesbaar(k)}: ${parameterWaarde(v)}`),
      ].filter(Boolean);
      return {
        overline: wortel ? 'besluit op de aanvraag' : laagNaam(s.laag),
        tekst: artikelKop(s),
        toelichting: namen.join(' · '),
        uitleg: artikelUitleg(s, wortel),
      };
    }
    case 'begrip': {
      const n = s.kinderen.length;
      return {
        overline: 'open begrip',
        tekst: `Open begrip ${s.id}`,
        toelichting: n ? `ingevuld door ${n === 1 ? 'het artikel' : `${n} artikelen`} hieronder` : '',
        uitleg: zinnen(
          `${artikelNaam(s.wet, s.artikel)} laat dit begrip open`,
          s.beschrijving,
          n
            ? `${n === 1 ? 'Het artikel' : 'De artikelen'} hieronder ${n === 1 ? 'vult' : 'vullen'} het in`
            : s.ingevuld_door
              ? `Geen regel vult het in; dat gebeurt per geval door ${s.ingevuld_door}`
              : 'Geen regelwerk van deze casus vult het in',
        ),
      };
    }
    case 'cel':
      return s.cel
        ? {
            overline: 'antwoord van een cel',
            tekst: `Antwoord van ${celNaam(s.cel)}`,
            toelichting: `geeft ${s.antwoord}`,
            uitleg: zinnen(
              `${hoofd(celNaam(s.cel))} geeft het antwoord ${s.antwoord} uit haar reductie ${s.reductie}`,
              'In haar cel.yaml staat welke reductie dit antwoord geeft',
            ),
          }
        : {
            overline: 'antwoord van een cel',
            tekst: `Geen cel voor ${s.antwoord}`,
            toelichting: '',
            uitleg: zinnen(
              `Geen cel van deze casus geeft het antwoord ${s.antwoord}`,
              'Zonder dit antwoord blijft de invoer leeg, of kan de beslissende cel het besluit niet nemen',
            ),
          };
    case 'kroniek':
      return {
        overline: 'uit de kroniek',
        tekst: `Kroniek van ${celNaam(s.cel)}`,
        toelichting: '',
        uitleg: s.besluiten_onder
          ? zinnen(`De reductie leest de besluiten onder ${verwijzing(s.besluiten_onder)} uit de kroniek van ${celNaam(s.cel)}`)
          : zinnen(`De reductie leest de vastgelegde feiten ${verwijzing(s.executogrammen_van)} uit de kroniek van ${celNaam(s.cel)}`),
      };
    case 'eerder':
      return {
        overline: 'zie eerder',
        tekst: artikelKop(s),
        toelichting: '',
        uitleg: 'Dit artikel staat al eerder in de keten. Daar staan ook de stappen eronder.',
      };
    default:
      return { overline: '', tekst: s.soort, toelichting: '', uitleg: '' };
  }
}

/** The path of a file as the repository names it: under `corpus/` for a case of regelrecht itself. */
export const bestandPad = (bestand, bron) => (bestand ? (bron ? bestand : `corpus/${bestand}`) : '');
