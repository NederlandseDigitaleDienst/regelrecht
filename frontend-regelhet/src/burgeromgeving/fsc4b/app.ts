// The one app of this browser tab: its list of RegelHets, its keys in
// IndexedDB, and which RegelHet the citizen's screens talk to now
// (`gekozen`). That choice is kept for the tab only (sessionStorage), so a
// reload stays where it was; it is no key and no data of the citizen.
import { ref } from 'vue';
import { laadUiterlijk } from '../../gedeeld/uiterlijk';
import { laadConfig } from './config';
import { Burgerapp } from './kern';
import { indexedDbOpslag } from './opslag';
import { laadWasm } from './rekenen';
import type { MaakPoc } from './rekenen';

const KEUZE = 'regelhet-burgerapp-gekozen';
const basis = import.meta.env.BASE_URL;

function leesKeuze(): string | null {
  try {
    return sessionStorage.getItem(KEUZE);
  } catch {
    return null;
  }
}

/** The RegelHet the citizen's screens talk to, or null on the list. */
export const gekozen = ref<string | null>(leesKeuze());

/** Talk to `id` from now on, or to none. */
export function kies(id: string | null): void {
  gekozen.value = id;
  try {
    if (id) sessionStorage.setItem(KEUZE, id);
    else sessionStorage.removeItem(KEUZE);
  } catch {
    // Without storage the choice lasts until the page reloads.
  }
  // The screens take the look of the chosen RegelHet, or the default.
  void laadUiterlijk();
}

let app: Promise<Burgerapp> | null = null;

/** The app, made once its list has loaded. A list that does not load is tried again on the next call. */
export function burgerapp(): Promise<Burgerapp> {
  app ??= laadConfig(basis)
    .then((regelhets) => new Burgerapp(regelhets, indexedDbOpslag(), (url, init) => fetch(url, init)))
    .catch((e: unknown) => {
      app = null;
      throw e;
    });
  return app;
}

let wasm: Promise<MaakPoc> | null = null;

/** The kern as WebAssembly, loaded once. */
export function rekenkern(): Promise<MaakPoc> {
  wasm ??= laadWasm(basis).catch((e: unknown) => {
    wasm = null;
    throw e;
  });
  return wasm;
}
