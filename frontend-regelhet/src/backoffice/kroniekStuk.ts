// A record of a chronicle as the employee's screens name it: by what the
// server says it is, by its number in the chronicle, and by the submission it
// is or answers to. Shared by the chronicle and the sheet that opens a record
// (KroniekView.vue, StukSheet.vue).
import { isBesluit, isIndiening, omschrijving, rolVan } from '../gedeeld/keten';
import type { Casus, Gram } from '../gedeeld/types';
import type { Kroniek } from './api';

/** Record `g` of chronicle `k` in words, by what the server says it is. */
export const omschrijfStuk = (g: Gram, casus: Casus | null | undefined, k: Kroniek | undefined): string =>
  casus && k ? omschrijving(g, casus, rolVan(g, k.stand)) : g.kind;

/** Record `id` of chronicle `k` in words, by its number in the chronicle. */
export function noemStuk(id: string, casus: Casus | null | undefined, k: Kroniek | undefined): string {
  const i = k?.grammen.findIndex((g) => g.id === id) ?? -1;
  const g = k?.grammen[i];
  if (!g) return 'een stuk in een andere kroniek';
  return casus ? `nr. ${i + 1}, ${omschrijfStuk(g, casus, k).toLowerCase()}` : `nr. ${i + 1}`;
}

/** The submission record `g` is or answers to, in chronicle `k`: what its panel in the work queue shows. */
export function indieningVan(g: Gram, k: Kroniek | undefined): Gram | undefined {
  if (!k) return undefined;
  if (isIndiening(g, k.stand)) return g;
  // A decision leads to the submission it answers; a recorded fact to nothing.
  if (!isBesluit(g, k.stand)) return undefined;
  const op = k.grammen.find((x) => x.id === g.refers_to);
  return op && isIndiening(op, k.stand) ? op : undefined;
}
