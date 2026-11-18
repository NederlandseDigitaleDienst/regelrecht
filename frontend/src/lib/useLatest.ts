/**
 * useLatest - the "generation counter" guard against stale async writes.
 *
 * The editor re-implemented this pattern at every place where a slow
 * response could land after a newer request superseded it (law switch,
 * traject switch, document open, …):
 *
 *   let generation = 0;
 *   async function load() {
 *     const gen = ++generation;
 *     const res = await fetch(...);
 *     if (gen !== generation) return; // stale - discard
 *   }
 *
 * `useLatest()` captures that counter. Each call to the returned `claim`
 * starts a new generation and hands back an `isCurrent()` predicate that
 * stays `true` only until the next claim:
 *
 *   const claimLoad = useLatest();
 *   async function load() {
 *     const isCurrent = claimLoad();
 *     const res = await fetch(...);
 *     if (!isCurrent()) return; // a newer load() superseded this one
 *   }
 *
 * One `useLatest()` instance per *logical resource*: functions that must
 * supersede each other (e.g. `load()` and `switchLaw()` writing the same
 * refs) share one instance; independent resources get their own.
 *
 * @returns {() => () => boolean} claim - starts a new generation,
 *   returns its `isCurrent` predicate.
 */
export function useLatest() {
  let current = 0;
  return function claim() {
    const ticket = ++current;
    return () => ticket === current;
  };
}
