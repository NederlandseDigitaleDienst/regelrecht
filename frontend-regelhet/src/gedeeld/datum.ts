/**
 * A date as a screen writes it: `2026-10-14` → `14 oktober 2026`. A date
 * without a time is midnight UTC, so it is shown in UTC; west of Greenwich it
 * would otherwise become the day before.
 */
export const longDate = (iso: string): string =>
  new Date(iso).toLocaleDateString('nl-NL', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });
