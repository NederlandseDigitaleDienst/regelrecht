/**
 * A refusal of the cell in Dutch, for what a form can cause: a finding or a
 * value the decision needs and did not get, or one the law does not ask. The
 * cell and the engine report in English (packages/cel, packages/engine);
 * these patterns copy their messages, so whoever rewrites such a message
 * changes the pattern here too. Shared by the chronolexografie poc
 * (melding.js, which adds its own refusals) and the wetsprocessysteem.
 */

/** How an app names a field, and who decides (`casus.rol`). */
export interface Woorden {
  veld: (naam: string) => string;
  rol: string;
}

const hoofdletter = (tekst: string): string => tekst.charAt(0).toUpperCase() + tekst.slice(1);

const weigeringen: [RegExp, (w: Woorden, ...groepen: string[]) => string][] = [
  // A finding a policy leaves to the authority for this application, with an
  // open term of its own (kern::voorvertonen).
  [
    /the policy leaves '([^']+)' to the authority for this case/,
    (w, begrip = '') => `De regels laten "${w.veld(begrip)}" voor deze aanvraag aan ${w.rol} over. Geef die bevinding.`,
  ],
  // A finding the law leaves to the authority per case, not given yet (the engine).
  [
    /Required open term '([^']+)' on \S+ is decided per case by .*no judgement was given/,
    (w, begrip = '') =>
      `${hoofdletter(w.rol)} heeft "${w.veld(begrip)}" nog niet beoordeeld, en het besluit heeft die bevinding nodig.`,
  ],
  [
    /lacks '([^']+)', which the decision of \S+ needs \(Awb 4:5\)/,
    (w, veld = '') =>
      `De aanvraag mist "${w.veld(veld)}", en het besluit heeft dat nodig (Awb 4:5). Vul het aan in een nieuwe aanvraag.`,
  ],
  // A finding or a value the decision asks of the authority, such as the
  // motivation, not given yet (the cell).
  [
    /asks '([^']+)' of the deciding authority/,
    (w, veld = '') => `Het bestuursorgaan heeft "${w.veld(veld)}" nog niet gegeven, en het besluit heeft dat nodig.`,
  ],
  [/'([^']+)' is .*, which does not fit a/, (w, veld = '') => `De waarde van "${w.veld(veld)}" past niet bij wat de wet vraagt.`],
  [
    /the law does not ask '([^']+)' in the (\S+) of/,
    (w, veld = '', soort = '') =>
      `De wet vraagt niet om "${w.veld(veld)}" bij ${soort === 'BEZWAAR' ? 'het bezwaar' : 'de aanvraag'}.`,
  ],
];

/** The Dutch sentence for the refusal `fout`, or null when there is none. */
export function weigeringInWoorden(fout: unknown, w: Woorden): string | null {
  for (const [patroon, zin] of weigeringen) {
    const gevonden = String(fout).match(patroon);
    if (gevonden) return zin(w, ...gevonden.slice(1));
  }
  return null;
}
