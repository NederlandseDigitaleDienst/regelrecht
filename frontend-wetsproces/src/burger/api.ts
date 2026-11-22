// The API client of the public side: everything a citizen's screens ask the
// server, and nothing of the back office. It says nothing of how a request
// proves who sends it (gedeeld/sessie.ts), so the same client runs behind
// another login. It calls only routes a citizen may use: the shared ones
// (`/api/casus`, `/api/formulier`, `/api/klok`, `/api/ik`) and the citizen's
// own (`/api/dossier`, `/api/indieningen`).
import { gedeeldApi } from '../gedeeld/api';
import { haal, stuur } from '../gedeeld/http';
import type { Gram, Vervolgstand, Waarom } from '../gedeeld/types';

/** A submission as a citizen sends it: under `artikel` (the case's own application when left out), about the record `op`. */
export interface Indiening {
  artikel?: string;
  op?: string;
  velden: Record<string, unknown>;
}

export const burgerApi = {
  ...gedeeldApi,
  /** The records about the citizen, and how far the acts after each of their grants are. */
  dossier: () =>
    haal<{ grammen: Gram[]; vervolg?: Vervolgstand[] }>('/api/dossier').then((d) => ({
      grammen: d.grammen,
      vervolg: d.vervolg ?? [],
    })),
  waarom: (besluit: string) => haal<Waarom>(`/api/dossier/waarom/${encodeURIComponent(besluit)}`),
  indienen: (indiening: Indiening, sleutel: string) =>
    stuur<Gram>('POST', '/api/indieningen', indiening, sleutel),
};
