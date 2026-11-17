import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import {
  loadSavedTabs,
  saveTabs,
  loadSavedActiveTab,
  saveActiveTab,
} from './openTabsStorage';

// openTabsStorage keeps the editor's open law tabs per traject: each traject
// ref gets its own `regelrecht-open-tabs:<ref>` / `regelrecht-active-tab:<ref>`
// key. These pin the per-traject isolation and the try/catch safe defaults.

const TAB_A = { kind: 'article', lawId: 'wet_op_de_zorgtoeslag', articleNumber: '1' } as const;
const TAB_B = { kind: 'article', lawId: 'algemene_wet_inkomensafhankelijke_regelingen', articleNumber: '7' } as const;

beforeEach(() => {
  localStorage.clear();
});

describe('openTabsStorage', () => {
  it('persists and reloads tabs under a per-traject key', () => {
    saveTabs('traject-alpha', [TAB_A, TAB_B]);
    expect(loadSavedTabs('traject-alpha')).toEqual([TAB_A, TAB_B]);
    expect(localStorage.getItem('regelrecht-open-tabs:traject-alpha')).toBe(
      JSON.stringify([TAB_A, TAB_B]),
    );
  });

  it('isolates tabs between trajects', () => {
    saveTabs('traject-alpha', [TAB_A]);
    saveTabs('traject-beta', [TAB_B]);
    expect(loadSavedTabs('traject-alpha')).toEqual([TAB_A]);
    expect(loadSavedTabs('traject-beta')).toEqual([TAB_B]);
  });

  it('returns [] for an unknown traject', () => {
    expect(loadSavedTabs('nope')).toEqual([]);
  });

  it('returns [] when the stored value is not an array', () => {
    localStorage.setItem('regelrecht-open-tabs:x', JSON.stringify({ not: 'an array' }));
    expect(loadSavedTabs('x')).toEqual([]);
  });

  it('returns [] on malformed JSON instead of throwing', () => {
    localStorage.setItem('regelrecht-open-tabs:x', '{broken');
    expect(loadSavedTabs('x')).toEqual([]);
  });

  it('persists and reloads the active tab per traject', () => {
    saveActiveTab('traject-alpha', TAB_A);
    saveActiveTab('traject-beta', TAB_B);
    expect(loadSavedActiveTab('traject-alpha')).toEqual(TAB_A);
    expect(loadSavedActiveTab('traject-beta')).toEqual(TAB_B);
  });

  it('clears the active tab when saving a falsy value', () => {
    saveActiveTab('traject-alpha', TAB_A);
    saveActiveTab('traject-alpha', null);
    expect(loadSavedActiveTab('traject-alpha')).toBeNull();
    expect(localStorage.getItem('regelrecht-active-tab:traject-alpha')).toBeNull();
  });

  it('returns null for an unknown active tab', () => {
    expect(loadSavedActiveTab('nope')).toBeNull();
  });

  describe('sanitises stored content on read', () => {
    it('drops entries missing lawId or articleNumber', () => {
      localStorage.setItem(
        'regelrecht-open-tabs:x',
        JSON.stringify([
          TAB_A,
          { articleNumber: '3' }, // no lawId
          { lawId: 'some_law' }, // no articleNumber
          null,
          'not-an-object',
          TAB_B,
        ]),
      );
      expect(loadSavedTabs('x')).toEqual([TAB_A, TAB_B]);
    });

    it('normalises a numeric articleNumber to a string', () => {
      localStorage.setItem(
        'regelrecht-open-tabs:x',
        JSON.stringify([{ lawId: 'some_law', articleNumber: 5 }]),
      );
      expect(loadSavedTabs('x')).toEqual([{ kind: 'article', lawId: 'some_law', articleNumber: '5' }]);
    });

    it('de-duplicates identical tabs (numeric vs string article collapse)', () => {
      localStorage.setItem(
        'regelrecht-open-tabs:x',
        JSON.stringify([
          { kind: 'article', lawId: 'some_law', articleNumber: 5 },
          { kind: 'article', lawId: 'some_law', articleNumber: '5' },
        ]),
      );
      expect(loadSavedTabs('x')).toEqual([{ kind: 'article', lawId: 'some_law', articleNumber: '5' }]);
    });

    it('caps the stored set at MAX_TABS, keeping the newest', () => {
      const many = Array.from({ length: 25 }, (_, i) => ({
        lawId: `law_${i}`,
        articleNumber: '1',
      }));
      localStorage.setItem('regelrecht-open-tabs:x', JSON.stringify(many));
      const loaded = loadSavedTabs('x');
      expect(loaded).toHaveLength(20);
      expect(loaded[0]).toEqual({ kind: 'article', lawId: 'law_5', articleNumber: '1' });
      expect(loaded[19]).toEqual({ kind: 'article', lawId: 'law_24', articleNumber: '1' });
    });

    it('drops a malformed active tab', () => {
      localStorage.setItem('regelrecht-active-tab:x', JSON.stringify({ articleNumber: '3' }));
      expect(loadSavedActiveTab('x')).toBeNull();
    });

    it('normalises a numeric articleNumber on the active tab', () => {
      localStorage.setItem(
        'regelrecht-active-tab:x',
        JSON.stringify({ lawId: 'some_law', articleNumber: 5 }),
      );
      expect(loadSavedActiveTab('x')).toEqual({ kind: 'article', lawId: 'some_law', articleNumber: '5' });
    });
  });

  describe('safe defaults when storage is unavailable', () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    it('reads a tab saved before scenario tabs existed as an article tab', () => {
      localStorage.setItem('regelrecht-open-tabs:x', JSON.stringify([{ lawId: 'some_law', articleNumber: '2' }]));
      expect(loadSavedTabs('x')).toEqual([{ kind: 'article', lawId: 'some_law', articleNumber: '2' }]);
    });

    it('keeps a scenario tab, and drops one without a file name', () => {
      localStorage.setItem(
        'regelrecht-open-tabs:x',
        JSON.stringify([
          { kind: 'scenario', lawId: 'some_law', filename: 'basis.feature' },
          { kind: 'scenario', lawId: 'some_law' },
          { kind: 'scenario', lawId: 'some_law', filename: '' },
        ]),
      );
      expect(loadSavedTabs('x')).toEqual([{ kind: 'scenario', lawId: 'some_law', filename: 'basis.feature' }]);
    });

    it('keeps a scenario tab next to an article tab of the same law', () => {
      const tabs = [
        { kind: 'article', lawId: 'some_law', articleNumber: '1' },
        { kind: 'scenario', lawId: 'some_law', filename: '1' },
      ] as const;
      saveTabs('x', tabs);
      expect(loadSavedTabs('x')).toEqual(tabs);
    });

    it('swallows write failures (e.g. quota full / disabled)', () => {
      vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('QuotaExceededError');
      });
      expect(() => saveTabs('t', [TAB_A])).not.toThrow();
      expect(() => saveActiveTab('t', TAB_A)).not.toThrow();
    });

    it('returns safe defaults when reads throw', () => {
      vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
        throw new Error('SecurityError');
      });
      expect(loadSavedTabs('t')).toEqual([]);
      expect(loadSavedActiveTab('t')).toBeNull();
    });
  });
});
