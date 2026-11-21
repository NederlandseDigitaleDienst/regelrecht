// The API client of the back office: what an employee's screens ask the
// server. The server checks every call against the roles of the account; this
// client only names the routes.
import { gedeeldApi } from '../gedeeld/api';
import { haal, stuur } from '../gedeeld/http';
import type { Gram, Klok, Voorvertoning, Waarom } from '../gedeeld/types';

/** The chronicle of one cell. */
export interface Kroniek {
  cel: string;
  cel_naam: string;
  kop: number;
  grammen: Gram[];
}

/** A decision on the submission `op`, with what the authority finds. */
export interface Beslissing {
  op: string;
  na?: string;
  bevindingen: Record<string, unknown>;
}

/** An external party as the admin panel shows it: what waits for it. */
export interface Partij {
  cel: string;
  cel_naam: string;
  /** The submissions in its chronicle that wait for its decision. */
  open: Gram[];
  /** Its decisions of which no other cell received a copy yet. */
  toe_te_zenden: Gram[];
}

/** `GET /api/backoffice/beheer/partijen`: the executor, and per external party what waits for it. */
export interface Partijen {
  uitvoerder: { cel: string; cel_naam: string };
  partijen: Partij[];
}

/** A fact the cell `cel` records. */
export interface Vastlegging {
  cel: string;
  feit: string;
  velden: Record<string, unknown>;
  verwijst_naar?: string;
}

export const backofficeApi = {
  ...gedeeldApi,
  kroniek: (cel: string) => haal<Kroniek>(`/api/backoffice/cellen/${encodeURIComponent(cel)}/kroniek`),
  waarom: (besluit: string) => haal<Waarom>(`/api/backoffice/waarom/${encodeURIComponent(besluit)}`),
  voorvertoning: (b: Beslissing) => stuur<Voorvertoning>('POST', '/api/backoffice/voorvertoning', b),
  beslissen: (b: Beslissing, sleutel: string) =>
    stuur<{ besluit: Gram; waarom: Waarom }>('POST', '/api/backoffice/besluiten', b, sleutel),
  vastleggen: (v: Vastlegging, sleutel: string) => stuur<Gram>('POST', '/api/backoffice/vastleggingen', v, sleutel),
  klokZetten: (datum: string) => stuur<Klok>('PUT', '/api/backoffice/klok', { datum }),
  /** Empty the test setup, test mode only: every chronicle, the kept answers, the clock back on the start. */
  leegmaken: () => stuur<Klok>('POST', '/api/backoffice/testomgeving/leegmaken', {}),
};

/**
 * The admin panel: what an admin enters on behalf of an external party, such
 * as the GGD's advice or the court's judgment. The same steps as the
 * executor's, under routes that reach only the external parties' records.
 */
export const beheerApi = {
  partijen: () => haal<Partijen>('/api/backoffice/beheer/partijen'),
  voorvertoning: (b: Beslissing) => stuur<Voorvertoning>('POST', '/api/backoffice/beheer/voorvertoning', b),
  beslissen: (b: Beslissing, sleutel: string) =>
    stuur<{ besluit: Gram; waarom: Waarom }>('POST', '/api/backoffice/beheer/besluiten', b, sleutel),
  vastleggen: (v: Vastlegging, sleutel: string) => stuur<Gram>('POST', '/api/backoffice/beheer/vastleggingen', v, sleutel),
  toezenden: (besluit: string, sleutel: string) =>
    stuur<Gram>('POST', '/api/backoffice/beheer/toezendingen', { besluit }, sleutel),
};
