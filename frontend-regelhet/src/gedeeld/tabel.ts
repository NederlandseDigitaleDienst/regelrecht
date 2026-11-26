// A table from the chronicle (TabelUitKroniek.vue): which columns it shows,
// what a cell says, and where a row comes from, in words.
import { hoofdletter, leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from './datum';
import { waardeTekst } from './keten';
import type { Tabel, Tabelbron, Tabelkolom, Tabelrij } from './types';

/**
 * A value as rows of a table, when it is a non-empty list of records; a row
 * holds no sources then: it is what a form or a record holds itself.
 */
export function alsRijen(waarde: unknown): Tabelrij[] | undefined {
  if (!Array.isArray(waarde) || !waarde.length) return undefined;
  if (!waarde.every((r) => r !== null && typeof r === 'object' && !Array.isArray(r))) return undefined;
  return waarde.map((r) => ({ waarden: r as Record<string, unknown>, herkomst: [] }));
}

/** The columns of `tabel`: those the UI spec names, or else every key of the rows, in the order they come. */
export function kolommen(tabel: Tabel, rijen: Tabelrij[]): Tabelkolom[] {
  if (tabel.kolommen.length) return tabel.kolommen;
  const namen: string[] = [];
  for (const r of rijen) for (const k of Object.keys(r.waarden)) if (!namen.includes(k)) namen.push(k);
  return namen.map((name) => ({ name }));
}

/** The heading of a column: its label in the UI spec, or else its key in words. */
export const kop = (k: Tabelkolom) => k.label ?? hoofdletter(leesbaar(k.name));

/** What a cell says: empty for a value the row does not hold. */
export const cel = (v: unknown) => (v === null || v === undefined ? '' : waardeTekst(v));

/** The fact or article a source records, in words: the last part of `<regelwerk>#<artikel>#<feit>`. */
export function watTekst(wat: string): string {
  const delen = wat.split('#');
  return hoofdletter(leesbaar(delen[delen.length - 1] ?? wat));
}

/** Where a row comes from, in a line: `Telbureau · telling ontvangen · 2 februari 2026`. */
export const bronTekst = (b: Tabelbron, celNaam: (cel: string) => string) =>
  `${celNaam(b.cel)} · ${watTekst(b.wat).toLowerCase()} · ${longDate(b.op)}`;
