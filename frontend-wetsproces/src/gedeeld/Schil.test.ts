// @vitest-environment node
import { describe, expect, it } from 'vitest';
import type { RouteRecordNormalized } from 'vue-router';
import { isHuidig, tabsVan } from './schil';
import type { Tab } from './schil';

const tab: Tab = { naam: 'dossier', tekst: 'Mijn dossier', pad: '/burger', icoon: 'document', ook: ['besluit'] };
const record = (meta: RouteRecordNormalized['meta']) => ({ meta }) as RouteRecordNormalized;

describe('the tabs of a route', () => {
  it('come from the deepest route that declares them', () => {
    const binnen: Tab = { ...tab, naam: 'binnen' };
    expect(tabsVan({ matched: [record({ tabs: [tab] }), record({})] })).toEqual([tab]);
    expect(tabsVan({ matched: [record({ tabs: [tab] }), record({ tabs: [binnen] })] })).toEqual([binnen]);
    expect(tabsVan({ matched: [record({})] })).toEqual([]);
  });

  it('are current on their own route and on a route under them', () => {
    expect(isHuidig(tab, 'dossier')).toBe(true);
    expect(isHuidig(tab, 'besluit')).toBe(true);
    expect(isHuidig(tab, 'aanvraag')).toBe(false);
    expect(isHuidig(tab, undefined)).toBe(false);
  });
});
