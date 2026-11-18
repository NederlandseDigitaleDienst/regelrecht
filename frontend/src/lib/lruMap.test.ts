import { describe, it, expect, vi } from 'vitest';
import { createLruMap } from './lruMap';

describe('createLruMap', () => {
  it('stores and retrieves entries', () => {
    const lru = createLruMap(3);
    lru.set('a', 1);
    expect(lru.has('a')).toBe(true);
    expect(lru.get('a')).toBe(1);
    expect(lru.get('missing')).toBeUndefined();
    expect(lru.size).toBe(1);
  });

  it('evicts the least-recently-used entry past maxSize', () => {
    const lru = createLruMap(2);
    lru.set('a', 1);
    lru.set('b', 2);
    lru.set('c', 3);
    expect(lru.has('a')).toBe(false);
    expect(lru.has('b')).toBe(true);
    expect(lru.has('c')).toBe(true);
  });

  it('get touches: a read entry survives the next eviction', () => {
    const lru = createLruMap(2);
    lru.set('a', 1);
    lru.set('b', 2);
    lru.get('a'); // 'a' is now most-recent, 'b' is oldest
    lru.set('c', 3);
    expect(lru.has('a')).toBe(true);
    expect(lru.has('b')).toBe(false);
  });

  it('set on an existing key re-positions it as most-recent', () => {
    const lru = createLruMap(2);
    lru.set('a', 1);
    lru.set('b', 2);
    lru.set('a', 10); // refresh 'a'
    lru.set('c', 3);
    expect(lru.get('a')).toBe(10);
    expect(lru.has('b')).toBe(false);
  });

  it('peek reads without touching the LRU order', () => {
    const lru = createLruMap(2);
    lru.set('a', 1);
    lru.set('b', 2);
    expect(lru.peek('a')).toBe(1);
    lru.set('c', 3); // 'a' is still oldest - peek did not bump it
    expect(lru.has('a')).toBe(false);
  });

  it('calls onEvict for capacity evictions only', () => {
    const onEvict = vi.fn();
    const lru = createLruMap(1, { onEvict });
    lru.set('a', 1);
    lru.set('b', 2);
    expect(onEvict).toHaveBeenCalledTimes(1);
    expect(onEvict).toHaveBeenCalledWith('a', 1);

    lru.delete('b');
    lru.set('c', 3);
    lru.clear();
    expect(onEvict).toHaveBeenCalledTimes(1); // delete/clear do not fire it
  });

  it('exposes keys() in oldest-first order and supports clear()', () => {
    const lru = createLruMap(3);
    lru.set('a', 1);
    lru.set('b', 2);
    lru.get('a');
    expect([...lru.keys()]).toEqual(['b', 'a']);
    lru.clear();
    expect(lru.size).toBe(0);
  });
});
