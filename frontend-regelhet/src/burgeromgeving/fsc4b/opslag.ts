// Where the app keeps, per RegelHet, its key and its credential: in the
// browser's IndexedDB, which can hold a `CryptoKey` as it is. The private
// half is not extractable, so what is kept is a handle the browser signs
// with, not the key's bytes. That is software all the same: whoever controls
// this browser profile can make the app sign. The screens say so
// (`opslagTekst` in ../tauri/status.ts).
//
// Nothing else is kept: the session lives in memory, and what the citizen
// types into a form never reaches storage.

/** What the app keeps for one RegelHet. */
export interface Bewaard {
  sleutel: CryptoKeyPair;
  /** The credential of the Activatiedienst for that key, once activated. */
  credential: string | null;
}

/** Where the app keeps its keys. In the browser IndexedDB; in a test, memory. */
export interface Sleutelopslag {
  lees(regelhet: string): Promise<Bewaard | null>;
  schrijf(regelhet: string, bewaard: Bewaard): Promise<void>;
  wis(regelhet: string): Promise<void>;
}

/** Keeps everything in memory, for a test or a browser without IndexedDB. */
export function geheugenOpslag(): Sleutelopslag {
  const alles = new Map<string, Bewaard>();
  return {
    lees: async (r) => alles.get(r) ?? null,
    schrijf: async (r, b) => void alles.set(r, b),
    wis: async (r) => void alles.delete(r),
  };
}

const DATABANK = 'regelhet-burgerapp';
const STORE = 'sleutels';

function verzoek<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((ok, fout) => {
    r.onsuccess = () => ok(r.result);
    r.onerror = () => fout(r.error ?? new Error('IndexedDB failed'));
  });
}

function open(): Promise<IDBDatabase> {
  const r = indexedDB.open(DATABANK, 1);
  r.onupgradeneeded = () => r.result.createObjectStore(STORE);
  return verzoek(r);
}

/** The browser's IndexedDB, one record per RegelHet under its id. */
export function indexedDbOpslag(): Sleutelopslag {
  let db: Promise<IDBDatabase> | null = null;
  async function winkel(modus: IDBTransactionMode): Promise<IDBObjectStore> {
    db ??= open().catch((e: unknown) => {
      db = null;
      throw e;
    });
    return (await db).transaction(STORE, modus).objectStore(STORE);
  }
  return {
    lees: async (r) => ((await verzoek((await winkel('readonly')).get(r))) as Bewaard | undefined) ?? null,
    schrijf: async (r, b) => void (await verzoek((await winkel('readwrite')).put(b, r))),
    wis: async (r) => void (await verzoek((await winkel('readwrite')).delete(r))),
  };
}
