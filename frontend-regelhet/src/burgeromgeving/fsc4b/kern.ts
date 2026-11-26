// The core of the citizen's app in the browser, per RegelHet: the app key and
// the credential (opslag.ts), and the session of CreateSession in memory. It
// does what the Rust core of the desktop app does (packages/regelhet-app,
// core.rs), so the citizen's screens reach it through the same commands
// (`Invoke`, tauri/kern.ts) and the same mapping of their calls
// (`kernSessie`).
//
// A session lasts at most fifteen minutes on the server. When the service
// answers 401, the core starts a new session with the same credential and
// sends the request once more; a submission keeps its idempotency key inside
// the encryption, so it is recorded once.
import type { AppStatus, Invoke, KernFout } from '../tauri/kern';
import { ClientFout, TEST_CITIZENS, demoCredential, maakSessie, ontdek, roep } from './client';
import type { Adresboek, Haal, Sessie } from './client';
import { duimafdruk, nieuweSleutel, onverifieerd, publiekeJwk } from './jose';
import type { Sleutelopslag } from './opslag';

/** One RegelHet the app can talk to, from its configuration (config.ts). */
export interface Regelhet {
  /** A name of the app's own for it, a key in storage. */
  id: string;
  /** How the citizen knows it. */
  naam: string;
  /** Its public address, where the demo Stelsel and the services are. */
  adres: string;
  /** The OIN of its organization in the Stelseldocument. */
  oin: string;
}

/** How long before its end a credential counts as lapsed: a session started later would end before it is used. */
export const CREDENTIAL_MARGE_S = 60;

/** Who a credential names, until when, for which key. */
export interface Credential {
  bsn: string;
  naam: string | null;
  verlooptOp: number;
  /** The key it is bound to (`cnf.jwk`), as `sleutelnaam` writes it. */
  sleutel: string;
}

/** A P-256 key by its coordinates, to compare the key a credential names with the app key. */
const sleutelnaam = (jwk: { crv?: string; x?: string; y?: string }): string => `${jwk.crv}|${jwk.x}|${jwk.y}`;

/** The credential `token`, read without checking its signature: the server checks that on every session. */
export function leesCredential(token: string): Credential {
  const c = onverifieerd(token);
  const tekst = (n: string) => (typeof c[n] === 'string' && c[n] !== '' ? (c[n] as string) : null);
  const bsn = tekst('bsn');
  if (!bsn) throw fout({ kind: 'protocol', detail: 'the credential lacks `bsn`' });
  if (typeof c.exp !== 'number') throw fout({ kind: 'protocol', detail: 'the credential lacks `exp`' });
  const jwk = (c.cnf as { jwk?: Record<string, string> } | undefined)?.jwk;
  if (!jwk?.x || !jwk.y) throw fout({ kind: 'protocol', detail: 'the credential lacks `cnf.jwk`' });
  const naam = [tekst('given_name'), tekst('family_name')].filter(Boolean).join(' ') || null;
  return { bsn, naam, verlooptOp: c.exp, sleutel: sleutelnaam(jwk) };
}

const fout = (f: KernFout): KernFout => f;

/** The `KernFout` a step of the client throws as; on a step against the demo, a 404 means the server offers no demo. */
function alsKernFout(e: unknown, demo = false): unknown {
  if (!(e instanceof ClientFout)) return e;
  if (e.soort === 'netwerk') return fout({ kind: 'network', detail: e.message });
  if (e.soort === 'protocol') return fout({ kind: 'protocol', detail: e.message });
  if (demo && e.status === 404) return fout({ kind: 'no_demo', detail: e.body ?? '' });
  let body: unknown = e.body ?? '';
  try {
    body = JSON.parse(e.body ?? '');
  } catch {
    // Kept as text.
  }
  return fout({ kind: 'service', status: e.status ?? 0, body });
}

const nu = (): number => Math.floor(Date.now() / 1000);

/** The calls of the citizen's screens, as the commands of the core name them, and the route of the service each goes to. */
const ROUTE: Record<string, string> = {
  casus: 'casus',
  formulier: 'formulier',
  dossier: 'dossier',
  waarom: 'waarom',
  tijdlijn: 'tijdlijn',
  indienen: 'indieningen',
  gegevens: 'gegevens',
};

/** How the screens look before a RegelHet is chosen: the default, with no title or organisation (`Uiterlijk` in gedeeld/uiterlijk.ts). */
const GEEN_UITERLIJK = { accentkleur: null, titel: null, organisatie: null };

/** The app for every RegelHet of its configuration. */
export class Burgerapp {
  private readonly verbindingen = new Map<string, { boek?: Adresboek; sessie?: Sessie; bezig?: Promise<Sessie> }>();

  constructor(
    readonly regelhets: Regelhet[],
    private readonly opslag: Sleutelopslag,
    private readonly haal: Haal,
  ) {}

  regelhet(id: string): Regelhet {
    const r = this.regelhets.find((x) => x.id === id);
    if (!r) throw fout({ kind: 'not_configured', detail: `the app knows no RegelHet '${id}'` });
    return r;
  }

  /** The credential kept for `id`, read and checked against the kept key; null when there is none or it is bound to another key. */
  private async credential(id: string): Promise<{ paar: CryptoKeyPair; token: string; credential: Credential } | null> {
    const bewaard = await this.opslag.lees(id);
    if (!bewaard?.credential) return null;
    const credential = leesCredential(bewaard.credential);
    if (credential.sleutel !== sleutelnaam(await publiekeJwk(bewaard.sleutel))) return null;
    return { paar: bewaard.sleutel, token: bewaard.credential, credential };
  }

  /** What the screens may know of the app for `id`: never a key, a token or the credential itself. */
  async status(id: string): Promise<AppStatus> {
    const r = this.regelhet(id);
    const c = await this.credential(id).catch(() => null);
    const verlopen = !!c && c.credential.verlooptOp - CREDENTIAL_MARGE_S <= nu();
    const geldig = c && !verlopen ? c.credential : null;
    return {
      server: r.adres,
      config_problem: null,
      holder: geldig ? { bsn: geldig.bsn, name: geldig.naam } : null,
      credential_expires_at: geldig?.verlooptOp ?? null,
      credential_expired: verlopen,
      key_storage: { kind: 'browser' },
      hardware_backed: false,
      test_bsns: [...TEST_CITIZENS],
    };
  }

  private async adresboek(id: string, opnieuw = false): Promise<Adresboek> {
    const v = this.verbinding(id);
    if (v.boek && !opnieuw) return v.boek;
    const r = this.regelhet(id);
    try {
      v.boek = await ontdek(this.haal, r.adres, r.oin);
    } catch (e) {
      throw alsKernFout(e, true);
    }
    return v.boek;
  }

  private verbinding(id: string) {
    let v = this.verbindingen.get(id);
    if (!v) {
      v = {};
      this.verbindingen.set(id, v);
    }
    return v;
  }

  /**
   * Activate the app at `id` for the test citizen `bsn`: a new key, a
   * credential from the test Activatiedienst, the Stelseldocument checked,
   * and a first session. The old key and credential go first, so an
   * activation that fails leaves the app not activated.
   */
  async activeren(id: string, bsn: string): Promise<AppStatus> {
    if (!(TEST_CITIZENS as readonly string[]).includes(bsn)) {
      throw fout({ kind: 'input', detail: `the demo activates with one of ${TEST_CITIZENS.join(', ')} only` });
    }
    const r = this.regelhet(id);
    this.verbindingen.delete(id);
    await this.opslag.wis(id);
    const paar = await nieuweSleutel();
    await this.opslag.schrijf(id, { sleutel: paar, credential: null });
    // The Stelsel first: a server without a demo is told before a key is asked a credential.
    await this.adresboek(id);
    let token: string;
    try {
      token = await demoCredential(this.haal, r.adres, paar, bsn);
    } catch (e) {
      throw alsKernFout(e, true);
    }
    const credential = leesCredential(token);
    if (credential.sleutel !== sleutelnaam(await publiekeJwk(paar))) {
      throw fout({ kind: 'protocol', detail: 'the credential is bound to another key than the app key' });
    }
    await this.opslag.schrijf(id, { sleutel: paar, credential: token });
    await this.nieuweSessie(id);
    return this.status(id);
  }

  /** The organization of `id` as the checked Stelseldocument names it: its OIN and its key. */
  async organisatie(id: string): Promise<Pick<Adresboek, 'oin' | 'organisatiesleutel'>> {
    const { oin, organisatiesleutel } = await this.adresboek(id);
    return { oin, organisatiesleutel };
  }

  /**
   * The regelwerken, cells and case of `id` (`GET /fsc4b/regelwerken`), by
   * their path in the source. No session: it is the case, not the citizen's
   * data.
   */
  async regelwerken(id: string): Promise<Record<string, string>> {
    const bestanden = await this.publiek(id, '/fsc4b/regelwerken', 'the regelwerken');
    if (!bestanden || typeof bestanden !== 'object' || Object.values(bestanden).some((t) => typeof t !== 'string')) {
      throw fout({ kind: 'protocol', detail: 'the regelwerken are no object of path and text' });
    }
    return bestanden as Record<string, string>;
  }

  /**
   * How the screens of `id` look (`GET /fsc4b/uiterlijk`): its accent
   * colour, title and organisation, which its web front shows anyone. No
   * session: it is public.
   */
  uiterlijk(id: string): Promise<unknown> {
    return this.publiek(id, '/fsc4b/uiterlijk', 'the appearance');
  }

  /** The JSON at `pad` of `id`, a route without a session; `wat` names it in an error. */
  private async publiek(id: string, pad: string, wat: string): Promise<unknown> {
    const url = `${this.regelhet(id).adres}${pad}`;
    let antwoord: Response;
    try {
      antwoord = await this.haal(url);
    } catch (e) {
      throw fout({ kind: 'network', detail: `${url}: ${e instanceof Error ? e.message : String(e)}` });
    }
    const tekst = await antwoord.text();
    if (!antwoord.ok) throw alsKernFout(new ClientFout('status', url, antwoord.status, tekst));
    try {
      return JSON.parse(tekst) as unknown;
    } catch {
      throw fout({ kind: 'protocol', detail: `${wat} is no JSON` });
    }
  }

  /** Forget the key, the credential and the session of `id`. */
  async intrekken(id: string): Promise<AppStatus> {
    this.verbindingen.delete(id);
    await this.opslag.wis(id);
    return this.status(id);
  }

  /** The thumbprint of the app key at `id`, as the server names the app; null before activation. */
  async duimafdruk(id: string): Promise<string | null> {
    const bewaard = await this.opslag.lees(id);
    return bewaard ? duimafdruk(await publiekeJwk(bewaard.sleutel)) : null;
  }

  /**
   * A new session for `id`. A first attempt that fails other than on the
   * network or with a server error is made once more with the Stelseldocument
   * read anew: a demo server makes new keys at every start. Only a 401 on
   * the second attempt counts as a refused credential.
   */
  private nieuweSessie(id: string): Promise<Sessie> {
    const v = this.verbinding(id);
    // Calls that find no live session at the same moment share one.
    v.bezig ??= (async () => {
      try {
        const c = await this.credential(id);
        if (!c) throw fout({ kind: 'not_activated' });
        if (c.credential.verlooptOp - CREDENTIAL_MARGE_S <= nu()) throw fout({ kind: 'credential_expired' });
        let sessie: Sessie;
        try {
          sessie = await maakSessie(this.haal, await this.adresboek(id), c.paar, c.token);
        } catch (e) {
          if (e instanceof ClientFout && (e.soort === 'netwerk' || (e.status ?? 0) >= 500)) throw alsKernFout(e);
          try {
            sessie = await maakSessie(this.haal, await this.adresboek(id, true), c.paar, c.token);
          } catch (e2) {
            if (e2 instanceof ClientFout && e2.status === 401) throw await this.geweigerd(id);
            throw alsKernFout(e2);
          }
        }
        v.sessie = sessie;
        return sessie;
      } finally {
        v.bezig = undefined;
      }
    })();
    return v.bezig;
  }

  /**
   * The RegelHet `id` no longer takes the credential: it is forgotten, the
   * key kept, so the app shows as not activated there and the citizen
   * activates again instead of being sent back to a refusal.
   */
  private async geweigerd(id: string): Promise<KernFout> {
    this.verbindingen.delete(id);
    const bewaard = await this.opslag.lees(id);
    if (bewaard) await this.opslag.schrijf(id, { sleutel: bewaard.sleutel, credential: null });
    return fout({ kind: 'credential_refused' });
  }

  /** `payload` to the citizen's route `op` at `id`; the answer, after one new session when the first was refused. */
  async roep(id: string, op: string, payload: Record<string, unknown>, sleutel?: string): Promise<unknown> {
    const v = this.verbinding(id);
    let sessie = v.sessie ?? (await this.nieuweSessie(id));
    let antwoord = await roep(this.haal, sessie, op, payload, sleutel).catch((e: unknown) => {
      throw alsKernFout(e);
    });
    if (antwoord.status === 401) {
      // Another call may have renewed the session meanwhile; use that one.
      const verlopen = sessie;
      if (v.sessie === verlopen) v.sessie = undefined;
      sessie = v.sessie && v.sessie !== verlopen ? v.sessie : await this.nieuweSessie(id);
      antwoord = await roep(this.haal, sessie, op, payload, sleutel).catch((e: unknown) => {
        throw alsKernFout(e);
      });
    }
    if (antwoord.status === 401) throw await this.geweigerd(id);
    if (antwoord.status >= 200 && antwoord.status < 300) return antwoord.waarde;
    throw fout({ kind: 'service', status: antwoord.status, body: antwoord.waarde });
  }

  /**
   * The commands of the core for the RegelHet `gekozen()` names, as the
   * citizen's screens call them through `kernSessie` (tauri/kern.ts).
   */
  invoke(gekozen: () => string | null): Invoke {
    return async <T>(commando: string, args?: Record<string, unknown>): Promise<T> => {
      const id = gekozen();
      if (commando === 'uiterlijk') return (id ? await this.uiterlijk(id) : GEEN_UITERLIJK) as T;
      if (!id) throw fout({ kind: 'not_activated' });
      if (commando === 'status') return (await this.status(id)) as T;
      const route = ROUTE[commando];
      if (!route) throw fout({ kind: 'input', detail: `the app has no command '${commando}'` });
      if (commando === 'waarom') return (await this.roep(id, route, { id: args?.besluit })) as T;
      if (commando === 'tijdlijn') return (await this.roep(id, route, { id: args?.id })) as T;
      if (commando === 'indienen') {
        const indiening = (args?.indiening ?? {}) as Record<string, unknown>;
        return (await this.roep(id, route, indiening, args?.sleutel as string | undefined)) as T;
      }
      return (await this.roep(id, route, {})) as T;
    };
  }
}
