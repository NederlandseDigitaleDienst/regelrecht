/**
 * The simulated login step of a case (`state.vormen.aanvraag.inloggen`,
 * with `inlogstap`; src/inlogstap.rs). The applicant logs in on a screen of
 * its own (InlogScherm.vue), the service answers with an assertion, and that
 * goes into the session (sessie.js). The application form takes who is
 * logged in from the session, the same way tests/scenarios/speler.rs does:
 * the claim `sleutel` goes into the login field, and a claim named after a
 * field of the application fills that field.
 */
import { toon } from './logboek.js';
import { leesbaar } from './state.js';

/**
 * The fields of the application the login fills: the login field, and each
 * field named after a claim of `sessie`. `gevraagd` are the names of the
 * fields of the application.
 */
export function ingevuldDoorInloggen(inloggen, sessie, gevraagd) {
  if (!inloggen?.inlogstap) return new Set();
  const namen = Object.keys(sessie?.claims ?? {}).filter((n) => gevraagd.includes(n));
  return new Set([inloggen.veld, ...namen]);
}

/**
 * `waarden` of the application with what the session says. Without a
 * session the login field holds only what the scenario step names itself
 * (`uitStap`): a submission that did not come through the login, such as one
 * on paper. The example values and the applicant never fill it. Unchanged
 * for a case without a login step.
 */
export function metSessie(inloggen, sessie, waarden, gevraagd, uitStap = undefined) {
  if (!inloggen?.inlogstap) return waarden;
  const uit = { ...waarden };
  delete uit[inloggen.veld];
  if (!sessie) {
    if (uitStap !== undefined) uit[inloggen.veld] = uitStap;
    return uit;
  }
  for (const [naam, waarde] of Object.entries(sessie.claims ?? {})) {
    if (gevraagd.includes(naam)) uit[naam] = waarde;
  }
  if (inloggen.sleutel in (sessie.claims ?? {})) uit[inloggen.veld] = sessie.claims[inloggen.sleutel];
  return uit;
}

/**
 * Whether a refused login counts for the guide: only at a login step of the
 * scenario that expects the refusal (`stap`, the step of the YAML the
 * scenario is at). A mistyped name at a login that should succeed is no
 * step the guide can tick off.
 */
export const weigeringTelt = (stap) => stap?.stap === 'inloggen' && stap.uitkomst === 'geweigerd';

/** `Ingelogd als bakker`, for the login block at the top of the applicant's page. */
export const ingelogdAls = (sessie) => `Ingelogd als ${sessie.gebruikersnaam}`;

/**
 * The line of the login block before anyone logged in. The login holds for
 * a new application and for the ones made before alike, so it names both.
 */
export const nietIngelogdTekst = (aanbod) =>
  `Log in bij ${aanbod?.cel_naam} voor een nieuwe aanvraag en voor de aanvragen die je al deed. Het inloggen is gesimuleerd.`;

/**
 * The claims of the session in one line, the key first: on whose behalf,
 * and what else the service asserted.
 */
export function claimRegel(inloggen, sessie) {
  const claims = Object.entries(sessie?.claims ?? {});
  const eerst = claims.filter(([n]) => n === inloggen?.sleutel);
  const rest = claims.filter(([n]) => n !== inloggen?.sleutel);
  return [...eerst, ...rest].map(([n, v]) => `${leesbaar(n)} ${toon(v)}`).join(' · ');
}

/** Why the service refused a login, as the refusal screen says it. */
export function weigerTekst(reden) {
  if (reden === 'wachtwoord') {
    return 'Het wachtwoord klopt niet. Alle testaccounts hebben hetzelfde wachtwoord; het staat onder het formulier.';
  }
  return 'Voor deze gebruikersnaam staat bij de erkende dienst geen geldige machtiging voor deze dienst. De dienst heeft niets vastgelegd.';
}

/** The help text under the form: the test accounts and their password. */
export function accountTekst(aanbod) {
  const namen = aanbod?.gebruikersnamen ?? [];
  const accounts = namen.length ? `Testaccounts: ${namen.join(', ')}. ` : '';
  return `${accounts}Het wachtwoord is voor elk account ${aanbod?.wachtwoord}.`;
}
