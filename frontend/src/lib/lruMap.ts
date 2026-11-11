export interface LruMapOptions<K, V> {
  onEvict?: (key: K, value: V) => void;
}

export interface LruMap<K, V> {
  has(key: K): boolean;
  get(key: K): V | undefined;
  peek(key: K): V | undefined;
  set(key: K, value: V): LruMap<K, V>;
  delete(key: K): boolean;
  clear(): void;
  keys(): MapIterator<K>;
  readonly size: number;
}

/**
 * createLruMap - a Map with least-recently-used eviction.
 *
 * Consolidates the copy-pasted "Map insertion order IS the LRU order"
 * caches (the corpus reader's caches in `src/corpus/reader.ts`,
 * loaded-scope tracking in `useEngine`): `set` and `get` move the key to
 * the most-recent position (delete + set), and `set` evicts the oldest
 * entries past `maxSize`.
 *
 * `onEvict(key, value)` lets a cache keep companion state in sync when
 * an entry falls out (drop a paired promise, unload a law from the WASM
 * engine, …). It only fires for capacity evictions - not for explicit
 * `delete`/`clear`, where the caller is already managing the entry.
 *
 * @template K, V
 * @param {number} maxSize
 * @param {{ onEvict?: (key: K, value: V) => void }} [options]
 */
export function createLruMap<K, V>(
  maxSize: number,
  { onEvict }: LruMapOptions<K, V> = {},
): LruMap<K, V> {
  /** @type {Map<K, V>} */
  const map = new Map<K, V>();

  function evictOverflow() {
    while (map.size > maxSize) {
      // The map is non-empty inside this loop, so the first key and its value exist.
      const oldestKey = map.keys().next().value as K;
      const oldestValue = map.get(oldestKey) as V;
      map.delete(oldestKey);
      onEvict?.(oldestKey, oldestValue);
    }
  }

  return {
    has(key: K) {
      return map.has(key);
    },
    /** Read + touch: marks the key most-recently-used. */
    get(key: K) {
      if (!map.has(key)) return undefined;
      // has() just confirmed the key, so the stored value is a V.
      const value = map.get(key) as V;
      map.delete(key);
      map.set(key, value);
      return value;
    },
    /** Read without touching the LRU order. */
    peek(key: K) {
      return map.get(key);
    },
    /** Insert/overwrite at the most-recent position, then evict overflow. */
    set(key: K, value: V) {
      map.delete(key);
      map.set(key, value);
      evictOverflow();
      return this;
    },
    delete(key: K) {
      return map.delete(key);
    },
    clear() {
      map.clear();
    },
    keys() {
      return map.keys();
    },
    get size() {
      return map.size;
    },
  };
}
