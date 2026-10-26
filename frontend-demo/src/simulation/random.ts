/**
 * Seeded pseudo-random numbers for the simulation, so the same seed always
 * yields the same population (a presenter can repeat a run, a test can pin
 * one). mulberry32: small, fast, good enough for demographics.
 */

/** A seeded random source; see `createRandom`. */
export interface Random {
  /** Uniform in [0, 1). */
  next(): number;
  /** Uniform in [min, max). */
  uniform(min: number, max: number): number;
  /** Integer in [min, max] (inclusive). */
  int(min: number, max: number): number;
  /** True with probability p. */
  chance(p: number): boolean;
  /** One element, uniformly. */
  pick<T>(items: readonly T[]): T;
  /** One element, by relative weight. */
  weighted<T>(items: readonly T[], weights: readonly number[]): T;
  /** Standard normal (Box-Muller). */
  gauss(mean?: number, sd?: number): number;
  /** Log-normal with the given parameters of the underlying normal. */
  lognormal(mu: number, sigma: number): number;
}

export function createRandom(seed: unknown): Random {
  let a = (Number(seed) >>> 0) || 1;
  const next = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const rng: Random = {
    /** Uniform in [0, 1). */
    next,
    /** Uniform in [min, max). */
    uniform: (min, max) => min + (max - min) * next(),
    /** Integer in [min, max] (inclusive). */
    int: (min, max) => Math.floor(min + (max - min + 1) * next()),
    /** True with probability p. */
    chance: (p) => next() < p,
    /** One element, uniformly. */
    // Callers pass a non-empty list, so the index is always in range.
    pick: (items) => items[Math.floor(next() * items.length)]!,
    /** One element, by relative weight. */
    weighted: (items, weights) => {
      const total = weights.reduce((s, w) => s + w, 0);
      // The items and weights lists are non-empty and of equal length at every call site.
      if (total <= 0) return items[0]!;
      let r = next() * total;
      for (let i = 0; i < items.length; i += 1) {
        r -= weights[i]!;
        if (r < 0) return items[i]!;
      }
      return items[items.length - 1]!;
    },
    /** Standard normal (Box-Muller). */
    gauss: (mean = 0, sd = 1) => {
      const u = 1 - next();
      const v = next();
      return mean + sd * Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
    },
    /** Log-normal with the given parameters of the underlying normal. */
    lognormal: (mu, sigma) => Math.exp(rng.gauss(mu, sigma)),
  };
  return rng;
}
