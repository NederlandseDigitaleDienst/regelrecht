// The API client of the back office: what an employee's screens ask the
// server. The server checks every call against the roles of the account; this
// client only names the routes.
import { gedeeldApi } from '../gedeeld/api';
import { haal, stuur } from '../gedeeld/http';
import type { Gram, Klok, Kroniek, Tijdlijn, Vervolgstand, Voorvertoning, Wachtend, Waarom, Werkvoorraad } from '../gedeeld/types';

// The chronicle and the work queue are read by the organisation's front too
// (src/organisatie/), so their shapes live in gedeeld/types.ts.
export type { Kroniek, Wachtend, Werkvoorraad };

/** A decision on the submission `op`, with what the authority finds. */
export interface Beslissing {
  op: string;
  na?: string;
  bevindingen: Record<string, unknown>;
}

/**
 * A request of the executor that an external party received and did not
 * answer yet: its receipt in the party's chronicle, which holds for whom it
 * is, and the fact that answers it.
 */
export interface OpenVerzoek {
  verzoek: Gram;
  antwoordfeit: string;
}

/** An external party as the admin panel shows it: what waits for it. */
export interface Partij {
  cel: string;
  cel_naam: string;
  /** The submissions in its chronicle that wait for its decision, with the record to decide after (`na`) when one was revoked. */
  open: Wachtend[];
  /** Its decisions of which no other cell received a copy yet. */
  toe_te_zenden: Gram[];
  /** The requests of the executor it has not answered yet. */
  verzoeken: OpenVerzoek[];
}

/** `GET /api/backoffice/beheer/partijen`: the executor, and per external party what waits for it. */
export interface Partijen {
  uitvoerder: { cel: string; cel_naam: string };
  partijen: Partij[];
  /** The parties with a server of their own, linked with this one: nothing is entered for them here. */
  gekoppeld?: { cel: string; cel_naam: string }[];
  /** The facts by which a party answers a request, such as the GGD's advice: with a request open, it records them from that request. */
  antwoord_op_verzoek: string[];
}

/** A fact the cell `cel` records. */
export interface Vastlegging {
  cel: string;
  feit: string;
  velden: Record<string, unknown>;
  verwijst_naar?: string;
  /** The receipt of the request this fact answers; the fields it shares with the request come from it. */
  verzoek?: string;
}

/**
 * A request the executor's policy made when an application came in, as the
 * executor sees it: its own record, the party it went to, and the record of
 * the answer that came back to it. Never the party's own records.
 */
export interface EigenVerzoek {
  verzoek: Gram;
  aan: string;
  aan_naam: string;
  open: boolean;
  antwoord_ontvangen: Gram | null;
}

export const backofficeApi = {
  ...gedeeldApi,
  werkvoorraad: () => haal<Werkvoorraad>('/api/backoffice/werkvoorraad'),
  kroniek: (cel: string) => haal<Kroniek>(`/api/backoffice/cellen/${encodeURIComponent(cel)}/kroniek`),
  waarom: (besluit: string) => haal<Waarom>(`/api/backoffice/waarom/${encodeURIComponent(besluit)}`),
  /** The timeline of the application the record `id` belongs to: the executor's records. */
  tijdlijn: (id: string) => haal<Tijdlijn>(`/api/backoffice/tijdlijn/${encodeURIComponent(id)}`),
  voorvertoning: (b: Beslissing) => stuur<Voorvertoning>('POST', '/api/backoffice/voorvertoning', b),
  beslissen: (b: Beslissing, sleutel: string) =>
    stuur<{ besluit: Gram; waarom: Waarom }>('POST', '/api/backoffice/besluiten', b, sleutel),
  vastleggen: (v: Vastlegging, sleutel: string) => stuur<Gram>('POST', '/api/backoffice/vastleggingen', v, sleutel),
  /** The requests the executor's policy made, and whether the answer came back. */
  verzoeken: () => haal<{ verzoeken: EigenVerzoek[] }>('/api/backoffice/verzoeken'),
  /** Every grant of the executor and how far the acts that follow it are. */
  vervolg: () => haal<{ vervolg: Vervolgstand[] }>('/api/backoffice/vervolg').then((v) => v.vervolg),
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
