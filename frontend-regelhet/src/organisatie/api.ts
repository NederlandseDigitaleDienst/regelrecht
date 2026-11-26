// What the organisation's front tells its screens (packages/regelhet/src/
// organisatie/mod.rs): the organisation and its processes, and the address
// that opens a process with the front's hand-off. The front calls the
// processes itself; this module never names a process's own API.
import { haal } from '../gedeeld/http';

/** A process of the organisation, as the front sums it up. */
export interface Proces {
  /** `<namespace>/<name>` of the process. */
  id: string;
  naam: string;
  /** The title of its case, when the process answered and its case has one. */
  titel: string | null;
  /** Whether the process answered the front. */
  bereikbaar: boolean;
}

/** `GET /api/organisatie`. */
export interface Organisatie {
  naam: string;
  logo: string | null;
  /** The side of the processes the account reaches from here. */
  kant: 'burger' | 'backoffice';
  processen: Proces[];
}

export const organisatieApi = {
  overzicht: () => haal<Organisatie>('/api/organisatie'),
};

/**
 * The address that opens `naar` in the process `id`: the front signs a
 * statement and sends the browser on to the process, which makes its own
 * session (`GET /api/processen/{id}/openen`).
 */
export function openAdres(id: string, naar: string): string {
  return `/api/processen/${encodeURIComponent(id)}/openen?naar=${encodeURIComponent(naar)}`;
}

/** Where a tile of side `kant` leads in a process: the application form for a citizen, the work queue for an employee. */
export function bestemming(kant: Organisatie['kant']): string {
  return kant === 'burger' ? '/burger#nieuwe-aanvraag' : '/backoffice';
}

/** The name a tile shows: the title of the case, or else the process's own name. */
export const tegelNaam = (p: Proces): string => p.titel ?? p.naam;
