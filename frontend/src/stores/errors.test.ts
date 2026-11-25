import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useErrorsStore } from './errors';

const SAVE = { summary: 'Niet opgeslagen', details: 'HTTP 500' };
const ENGINE = { summary: 'Fout in artikel 2', details: 'article 2: ...' };

describe('errors store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('shows what the sources report, under their titles', () => {
    const store = useErrorsStore();
    store.report('save', 'Opslaan mislukt', SAVE);
    store.report('engine', 'Kan niet worden uitgevoerd', ENGINE);
    expect(store.visible).toEqual([
      { title: 'Opslaan mislukt', ...SAVE },
      { title: 'Kan niet worden uitgevoerd', ...ENGINE },
    ]);
  });

  it('replaces the error of a source instead of adding to it', () => {
    const store = useErrorsStore();
    store.report('save', 'Opslaan mislukt', SAVE);
    store.report('save', 'Opslaan mislukt', { summary: 'Anders', details: null });
    expect(store.visible.map((e) => e.summary)).toEqual(['Anders']);
  });

  it('drops an error when its source clears it', () => {
    const store = useErrorsStore();
    store.report('save', 'Opslaan mislukt', SAVE);
    store.clear('save');
    expect(store.visible).toEqual([]);
  });

  it('keeps a dismissed error away until the source has a new one', () => {
    const store = useErrorsStore();
    const onDismiss = vi.fn();
    store.report('save', 'Opslaan mislukt', SAVE, onDismiss);
    store.dismissVisible();
    expect(store.visible).toEqual([]);
    expect(onDismiss).toHaveBeenCalledOnce();

    store.report('save', 'Opslaan mislukt', SAVE);
    expect(store.visible).toEqual([]);

    // The same text from a new attempt is a new failure.
    store.report('save', 'Opslaan mislukt', { ...SAVE });
    expect(store.visible).toHaveLength(1);
  });

  it('opens the details with the errors it is given, and keeps them after closing', () => {
    const store = useErrorsStore();
    store.showDetails([{ title: 'Opslaan mislukt', ...SAVE }]);
    expect(store.detailsOpen).toBe(true);
    store.closeDetails();
    expect(store.detailsOpen).toBe(false);
    expect(store.detailsOf[0]?.details).toBe('HTTP 500');
  });
});
