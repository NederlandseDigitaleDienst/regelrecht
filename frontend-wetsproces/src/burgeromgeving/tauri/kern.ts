// The app's session: every call of the citizen's screens goes to a command of
// the Rust core (packages/wetsproces-app), which holds the app key, the
// credential and the session of FSC voor Burgers. This module only knows the
// names of the commands and the shape of what they answer; it never sees a
// key, a token or the credential, and keeps nothing in storage.
import { ApiError } from '@regelrecht/frontend-shared';
import { reden } from '../../gedeeld/sessie';
import type { Sessie } from '../../gedeeld/sessie';
import type { Ik } from '../../gedeeld/types';

/** How the webview calls a command of the core: Tauri's `invoke`, or a stub in a test. */
export type Invoke = <T>(commando: string, args?: Record<string, unknown>) => Promise<T>;

/** Where the core keeps the app key (`KeyStorage` in keystore.rs). */
export type Opslag = { kind: 'memory' } | { kind: 'file'; path: string } | { kind: 'hardware'; platform: string };

/** What the core says of itself (`Status` in core.rs). */
export interface AppStatus {
  server: string | null;
  config_problem: string | null;
  holder: { bsn: string; name: string | null } | null;
  credential_expires_at: number | null;
  credential_expired: boolean;
  key_storage: Opslag;
  hardware_backed: boolean;
  test_bsns: string[];
}

/** An error of a command (`CommandError` in error.rs). */
export type KernFout =
  | { kind: 'not_configured'; detail: string }
  | { kind: 'not_activated' }
  | { kind: 'credential_expired' }
  | { kind: 'credential_refused' }
  | { kind: 'no_demo'; detail: string }
  | { kind: 'network'; detail: string }
  | { kind: 'service'; status: number; body: unknown }
  | { kind: 'protocol'; detail: string }
  | { kind: 'key_store'; detail: string }
  | { kind: 'input'; detail: string };

const isKernFout = (e: unknown): e is KernFout =>
  !!e && typeof e === 'object' && 'kind' in e && typeof (e as { kind: unknown }).kind === 'string';

/** The errors after which the citizen activates the app again. */
export const opnieuwActiveren = (f: KernFout): boolean =>
  f.kind === 'not_activated' || f.kind === 'credential_expired' || f.kind === 'credential_refused';

/** What a citizen reads for `f`. */
export function melding(f: KernFout): string {
  switch (f.kind) {
    case 'not_configured':
      return 'De app weet niet waar de server is. Zet WETSPROCES_APP_SERVER op het adres van de server en start de app opnieuw.';
    case 'not_activated':
      return 'Deze app is nog niet geactiveerd.';
    case 'credential_expired':
      return 'De activering van deze app is verlopen. Activeer de app opnieuw.';
    case 'credential_refused':
      return 'De server neemt de activering van deze app niet meer aan. Activeer de app opnieuw.';
    case 'no_demo':
      return 'Deze server biedt geen demo-Activatiedienst aan. De app kan alleen worden geactiveerd bij een server in de testmodus met de demo van FSC voor Burgers aan.';
    case 'network':
      return 'De server is niet bereikbaar. Controleer de verbinding en probeer het opnieuw.';
    case 'service':
      return reden(typeof f.body === 'string' ? f.body : JSON.stringify(f.body));
    case 'protocol':
      return `Het antwoord van de server klopt niet met het protocol: ${f.detail}`;
    case 'key_store':
      return `De app kan zijn sleutel niet lezen of bewaren: ${f.detail}`;
    case 'input':
      return f.detail;
  }
}

/** The error a screen gets for what a command threw. */
export function alsFout(e: unknown): Error {
  if (!isKernFout(e)) return e instanceof Error ? e : new Error(String(e));
  if (e.kind === 'service') {
    const body = typeof e.body === 'string' ? e.body : JSON.stringify(e.body);
    return new ApiError(reden(body), { status: e.status, body, contentType: 'application/json' });
  }
  // A 401, as the browser gets without a login: the screens already know it.
  if (opnieuwActiveren(e)) return new ApiError(melding(e), { status: 401 });
  return new Error(melding(e));
}

/** Who is logged in, as `GET /api/ik` says it, from the credential the core holds. */
export function ikUit(status: AppStatus): Ik | null {
  if (!status.holder) return null;
  return {
    sub: `bsn:${status.holder.bsn}`,
    naam: status.holder.name,
    burger: true,
    identiteit: status.holder.bsn,
    niveau: null,
    cellen: [],
    klok_zetten: false,
  };
}

const WAAROM = /^\/api\/dossier\/waarom\/([^/]+)$/;

/**
 * The session that sends the citizen's calls to the core. `bijOpnieuwActiveren`
 * runs when a command says the app must be activated again.
 */
export function kernSessie(invoke: Invoke, bijOpnieuwActiveren: (f: KernFout) => void = () => {}): Sessie {
  async function roep<T>(commando: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return await invoke<T>(commando, args);
    } catch (e) {
      if (isKernFout(e) && opnieuwActiveren(e)) bijOpnieuwActiveren(e);
      throw alsFout(e);
    }
  }

  return {
    async haal<T>(pad: string): Promise<T> {
      if (pad === '/api/ik') {
        const ik = ikUit(await roep<AppStatus>('status'));
        if (!ik) throw alsFout({ kind: 'not_activated' } satisfies KernFout);
        return ik as T;
      }
      if (pad === '/api/casus') return roep<T>('casus');
      if (pad === '/api/formulier') return roep<T>('formulier');
      if (pad === '/api/dossier') return roep<T>('dossier');
      const waarom = WAAROM.exec(pad);
      if (waarom?.[1]) return roep<T>('waarom', { besluit: decodeURIComponent(waarom[1]) });
      throw new Error(`De app biedt ${pad} niet aan.`);
    },
    async stuur<T>(methode: string, pad: string, body: unknown, sleutel?: string): Promise<T> {
      if (methode !== 'POST' || pad !== '/api/indieningen') throw new Error(`De app biedt ${methode} ${pad} niet aan.`);
      // Every submission of the app carries its key, inside the encryption.
      if (!sleutel) throw new Error('Een indiening vanuit de app heeft een idempotentiesleutel nodig.');
      return roep<T>('indienen', { indiening: body, sleutel });
    },
  };
}
