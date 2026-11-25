import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useErrorDetailsStore } from './errorDetails';

describe('errorDetails store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('opens with the error it is given', () => {
    const store = useErrorDetailsStore();
    store.show({ summary: 'Kort', details: 'lang' });
    expect(store.open).toBe(true);
    expect(store.current).toEqual({ summary: 'Kort', details: 'lang' });
  });

  it('keeps the error after closing, for the close animation', () => {
    const store = useErrorDetailsStore();
    store.show({ summary: 'Kort', details: 'lang' });
    store.close();
    expect(store.open).toBe(false);
    expect(store.current?.details).toBe('lang');
  });
});
