/**
 * Today's calendar date in the viewer's own time zone, as `YYYY-MM-DD`.
 *
 * Not `new Date().toISOString().slice(0, 10)`: that is the date in UTC, which
 * in the Netherlands is still yesterday until 01:00 (02:00 in summer). On the
 * first of January that picks last year's version of a law.
 *
 * `Temporal.Now.plainDateISO()` says this directly, but Safari has no Temporal
 * yet, and a polyfill is not worth it for one date.
 */
export function todayLocalIso(now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}
