// The shell, laid out as the editor's: the tabs of the side and the account
// menu in the bar, the screen in the main pane. The nldd-* elements are not
// defined in this test, so it reads the attributes Vue sets on them.
// (One file for the component and its helpers in schil.ts: a second file
// named schil.test.ts collides with this one on a case-insensitive disk.)
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { RouteRecordNormalized } from 'vue-router';
import Schil from './Schil.vue';
import { isHuidig, tabsVan } from './schil';
import { organisatie } from './uiterlijk';
import type { Tab } from './schil';
import { zetUiterlijk } from './uiterlijk';

const tabs: Tab[] = [
  { naam: 'lijst', tekst: 'Lijst', pad: '/lijst', icoon: 'inbox', ook: ['detail'] },
  { naam: 'klok', tekst: 'Klok', pad: '/klok', icoon: 'calendar', recht: 'klok_zetten' },
];

async function toon(props: Record<string, unknown>, pad = '/lijst') {
  const leeg = { render: () => null };
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/lijst', name: 'lijst', component: leeg },
      { path: '/lijst/:id', name: 'detail', component: leeg },
      { path: '/klok', name: 'klok', component: leeg },
    ],
  });
  await router.push(pad);
  return mount(Schil, {
    props: { tabs, naam: null, ...props },
    slots: { default: '<p id="scherm">scherm</p>' },
    global: { plugins: [router] },
  });
}

const tabTeksten = (w: Awaited<ReturnType<typeof toon>>) =>
  w.findAll('nldd-tab-bar-item').map((t) => t.attributes('text'));

afterEach(() => zetUiterlijk({ accentkleur: null }, document.createElement('html')));

describe('the shell', () => {
  it('shows the screen in the main pane of a bar split view', async () => {
    const w = await toon({});
    expect(w.find('nldd-shell nldd-bar-split-view').exists()).toBe(true);
    expect(w.find('[slot="main"] #scherm').exists()).toBe(true);
  });

  it('shows a tab only to who holds the right it needs, in the bar and in the mobile bar', async () => {
    expect(tabTeksten(await toon({}))).toEqual(['Lijst', 'Lijst']);
    expect(tabTeksten(await toon({ rechten: ['klok_zetten'] }))).toEqual(['Lijst', 'Klok', 'Lijst', 'Klok']);
  });

  it('keeps the bar of a side without tabs on top on a narrow screen, with its action as an icon button', async () => {
    const actie = { tekst: 'Nieuwe aanvraag', hash: '#nieuw', icoon: 'add' };
    const w = await toon({ tabs: [], titel: 'Vergunningen', actie });
    // From md up the bar as it was, with the action as a button with its text.
    const breed = w.find('[slot="primary-bar"]');
    expect(breed.attributes('above')).toBe('md');
    expect(breed.find('nldd-button').attributes('text')).toBe('Nieuwe aanvraag');
    // On sm a toolbar of its own, also on top and of the same size.
    const smal = w.find('[slot="primary-bar-sm"]');
    expect(smal.attributes('only')).toBe('sm');
    expect(smal.find('nldd-toolbar').attributes('size')).toBe('md');
    expect(smal.find('nldd-toolbar-title').attributes('text')).toBe('Vergunningen');
    const knop = smal.find('nldd-icon-button[appearance="primary"]');
    expect(knop.attributes('text')).toBe('Nieuwe aanvraag');
    expect(knop.attributes('icon')).toBe('add');
    expect(w.find('[slot="mobile-bar"]').exists()).toBe(false);
    // A side with tabs keeps them in the bar under the screen.
    const metTabs = await toon({});
    expect(metTabs.find('[slot="primary-bar-sm"]').exists()).toBe(false);
    expect(metTabs.find('[slot="mobile-bar"]').exists()).toBe(true);
  });

  it('centers the organization in the bar of a side with tabs, and leaves it out of a side without', async () => {
    const midden = (w: Awaited<ReturnType<typeof toon>>) => w.findAll('nldd-toolbar-title[slot="center"]').map((t) => t.attributes('text'));
    expect(midden(await toon({ uitvoerder: 'Gemeente Voorbeeld' }))).toEqual(['Gemeente Voorbeeld']);
    expect(midden(await toon({ tabs: [], titel: 'Vergunningen', uitvoerder: 'Gemeente Voorbeeld' }))).toEqual([]);
  });

  it('gives the bar extra room on the left only when its title sits at the left edge', async () => {
    const links = (w: Awaited<ReturnType<typeof toon>>) => w.find('[slot="primary-bar"]').attributes('padding-left');
    expect(links(await toon({ titel: 'Vergunningen' }))).toBe('16');
    expect(links(await toon({}))).toBeUndefined();
    // A way back to the organisation's front comes before the title, so the title is not at the edge.
    organisatie.value = { naam: 'Gemeente Voorbeeld', url: 'https://voorbeeld.example' };
    try {
      expect(links(await toon({ titel: 'Vergunningen' }))).toBeUndefined();
    } finally {
      organisatie.value = null;
    }
  });

  it('carries the title the server names on a side without tabs, unless the route names its own', async () => {
    zetUiterlijk({ accentkleur: null, titel: 'IIT Groningen' }, document.createElement('html'));
    const titels = (w: Awaited<ReturnType<typeof toon>>) =>
      w.findAll('nldd-toolbar-title').map((t) => t.attributes('text'));
    expect(titels(await toon({ tabs: [] }))).toEqual(['IIT Groningen', 'IIT Groningen']);
    expect(titels(await toon({}))).toEqual([]);
    expect(titels(await toon({ tabs: [], titel: 'Eigen' }))).toEqual(['Eigen', 'Eigen']);
  });

  it('marks the tab of the screen as current, also on a screen under it', async () => {
    const w = await toon({}, '/lijst/7');
    expect(w.find('nldd-tab-bar-item').attributes('current')).toBeDefined();
  });

  it('puts who is in in the account menu, with a way out', async () => {
    const w = await toon({ naam: 'A. de Vries', afmeldTekst: 'Afmelden' });
    expect(w.find('nldd-identity').attributes('text')).toBe('A. de Vries');
    const uit = w.findAll('nldd-menu-item').filter((m) => m.attributes('text') === 'Afmelden');
    expect(uit.length).toBeGreaterThan(0);
    await uit[0]!.trigger('select');
    expect(w.emitted('afmelden')).toHaveLength(1);
  });

  it('offers no way out to nobody', async () => {
    const w = await toon({});
    expect(w.findAll('nldd-menu-item').some((m) => m.attributes('icon') === 'logout')).toBe(false);
  });
});

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
