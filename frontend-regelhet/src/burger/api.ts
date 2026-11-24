// The API client of the public side: everything a citizen's screens ask the
// server, and nothing of the back office. It says nothing of how a request
// proves who sends it (gedeeld/sessie.ts), so the same client runs behind
// another login. It calls only routes a citizen may use: the shared ones
// (`/api/casus`, `/api/formulier`, `/api/klok`, `/api/ik`) and the citizen's
// own (`/api/dossier`, `/api/dossier/...`, `/api/indieningen`).
import { gedeeldApi } from '../gedeeld/api';
import { haal, stuur } from '../gedeeld/http';
import type { Gram, Stand, Tijdlijn, VerzoekInDossier, Vervolgstand, Waarom } from '../gedeeld/types';

/** A submission as a citizen sends it: under `artikel` (the case's own application when left out), about the record `op`. */
export interface Indiening {
  artikel?: string;
  op?: string;
  velden: Record<string, unknown>;
}

export const burgerApi = {
  ...gedeeldApi,
  /**
   * The records about the citizen, what the kern says of them (`stand`), and
   * how far the acts after each of their grants are, and per request of the
   * policy the ids of its records in the dossier.
   */
  dossier: () =>
    haal<{
      grammen: Gram[];
      stand: Stand;
      vervolg?: Vervolgstand[];
      door_het_systeem?: Record<string, string>;
      verzoeken?: VerzoekInDossier[];
    }>('/api/dossier').then((d) => ({
      grammen: d.grammen,
      stand: d.stand,
      vervolg: d.vervolg ?? [],
      doorHetSysteem: d.door_het_systeem ?? {},
      verzoeken: d.verzoeken ?? [],
    })),
  waarom: (besluit: string) => haal<Waarom>(`/api/dossier/waarom/${encodeURIComponent(besluit)}`),
  /** The timeline of the application the record `id` of the dossier belongs to. */
  tijdlijn: (id: string) => haal<Tijdlijn>(`/api/dossier/tijdlijn/${encodeURIComponent(id)}`),
  indienen: (indiening: Indiening, sleutel: string) =>
    stuur<Gram>('POST', '/api/indieningen', indiening, sleutel),
};
