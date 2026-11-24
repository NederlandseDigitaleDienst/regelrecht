// The routes both sides use: the case, its forms, the clock and who is
// logged in. The server answers them for a citizen and an employee alike.
// burger/api.ts and backoffice/api.ts each add their own routes to these.
import { haal } from './http';
import type { Casus, Ik, Klok, Vormen } from './types';

export const gedeeldApi = {
  ik: () => haal<Ik>('/api/ik'),
  casus: () => haal<Casus>('/api/casus'),
  formulier: () => haal<Vormen>('/api/formulier'),
  klok: () => haal<Klok>('/api/klok'),
};
