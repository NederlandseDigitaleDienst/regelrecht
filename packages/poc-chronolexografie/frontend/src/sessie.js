/**
 * Who is logged in through the login step of a case (`inloggen.inlogstap`,
 * src/inlogstap.rs): the session, as a cookie would hold it. The login
 * screen (InlogScherm.vue) puts the assertion the accredited service gave
 * here, the application form reads it, and Uitloggen clears it. The
 * chronicle of the service records that an authentication took place; who
 * is logged in now is no part of it, so nothing reads it from there.
 *
 * One session per case, in sessionStorage, so a reload keeps it and another
 * tab has its own. Without storage (a private window) the session lasts as
 * long as the page.
 */
import { reactive, ref } from 'vue';

const SLEUTEL = 'poc-sessies';

function gelezen() {
  try {
    return JSON.parse(sessionStorage.getItem(SLEUTEL) ?? '{}') ?? {};
  } catch {
    return {};
  }
}

/** Per case id, the assertion of whoever is logged in. */
const sessies = reactive(gelezen());

function bewaar() {
  try {
    sessionStorage.setItem(SLEUTEL, JSON.stringify(sessies));
  } catch {
    // No storage: the session lasts as long as the page.
  }
}

/** The assertion of whoever is logged in for case `casusId`, or null. */
export const sessieVan = (casusId) => sessies[casusId] ?? null;

/** Log in for case `casusId` with `assertie`, as the service gave it. */
export function zetSessie(casusId, assertie) {
  sessies[casusId] = assertie;
  bewaar();
}

/** Log out of case `casusId`: only the session goes, no chronicle changes. */
export function wisSessie(casusId) {
  delete sessies[casusId];
  bewaar();
}

/**
 * Opens a refusal of a login that a scenario keeps (`state.refusals`), so
 * the guide can see a login step the service refused (scenarioGuides.js).
 */
export const LOGIN_REFUSED = 'login refused';

/**
 * Whether the login screen is open. It takes the place of the app, as a
 * redirect to the service would, and returns to the application.
 */
export const inlogScherm = ref(false);
