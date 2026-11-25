// The shell, laid out as the editor's: the tabs of the side and the account
// menu in the bar, the screen in the main pane. The nldd-* elements are not
// defined in this test, so it reads the attributes Vue sets on them.
// (One file for the component and its helpers in schil.ts: a second file
// named schil.test.ts collides with this one on a case-insensitive disk.)
import { flushPromises, mount } from '@vue/test-utils';
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

  it('shows a tab only to who holds the right it needs, in the bars and in the mobile bar', async () => {
    // The bar on md, the one from lg, and the mobile bar.
    expect(tabTeksten(await toon({}))).toEqual(['Lijst', 'Lijst', 'Lijst']);
    expect(tabTeksten(await toon({ rechten: ['klok_zetten'] }))).toEqual(['Lijst', 'Klok', 'Lijst', 'Klok', 'Lijst', 'Klok']);
  });

  it("puts a screen's menu of actions beside the account, and each option does its own thing", async () => {
    const gekozen: string[] = [];
    const menu = { tekst: 'Vastleggen', icoon: 'add', opties: ['Verzonden bericht', 'Doorgestuurd bericht'].map((tekst) => ({ tekst, kies: () => gekozen.push(tekst) })) };
    const w = await toon({ menu });
    const breed = w.find('[slot="primary-bar"]');
    const items = breed.findAll('nldd-toolbar-item[slot="end"]');
    // The menu, then the account.
    const knop = items.at(-2)!.find('nldd-button');
    expect(knop.attributes('text')).toBe('Vastleggen');
    expect(knop.attributes('expandable')).toBeDefined();
    expect(items.at(-1)!.find('nldd-icon-button').attributes('text')).toBe('Account');
    const opties = knop.findAll('nldd-menu-item');
    expect(opties.map((o) => o.attributes('text'))).toEqual(['Verzonden bericht', 'Doorgestuurd bericht']);
    await opties[1]!.trigger('select');
    expect(gekozen).toEqual(['Doorgestuurd bericht']);
    // On md an icon button with the same menu.
    const md = w.find('[slot="primary-bar-md"] nldd-icon-button[text="Vastleggen"]');
    expect(md.attributes('expandable')).toBeDefined();
    expect(md.findAll('nldd-menu-item').map((o) => o.attributes('text'))).toEqual(['Verzonden bericht', 'Doorgestuurd bericht']);
    // On sm, beside the account in the bar under the screen.
    expect(w.find('[slot="mobile-bar"] nldd-icon-button[text="Vastleggen"]').exists()).toBe(true);
    expect((await toon({})).find('[text="Vastleggen"]').exists()).toBe(false);
  });

  it('puts the tools of the test setup in the account menu, each opening its hash', async () => {
    const gereedschap = [
      { tekst: 'Klok instellen', icoon: 'calendar', hash: '#klok' },
      { tekst: 'Testomgeving legen', icoon: 'delete', destructief: true, hash: '#testomgeving-legen' },
    ];
    const w = await toon({ gereedschap, naam: 'Beheerder' });
    const account = w.find('[slot="primary-bar"] nldd-icon-button[text="Account"]');
    const items = account.findAll('nldd-menu-item').map((i) => i.attributes('text'));
    expect(items).toEqual(expect.arrayContaining(['Klok instellen', 'Testomgeving legen']));
    // Before leaving, which stays last.
    expect(items.at(-1)).toBe('Uitloggen');
    // Emptying cannot be undone, and the item says so in red.
    expect(account.find('nldd-menu-item[text="Testomgeving legen"]').attributes('destructive')).toBeDefined();
    expect(account.find('nldd-menu-item[text="Klok instellen"]').attributes('destructive')).toBeUndefined();
    await account.find('nldd-menu-item[text="Testomgeving legen"]').trigger('select');
    await flushPromises();
    expect(w.vm.$route.hash).toBe('#testomgeving-legen');
    expect((await toon({})).find('nldd-menu-item[text="Klok instellen"]').exists()).toBe(false);
  });

  it('puts the options of a group under its title, after those without one', async () => {
    const optie = (tekst: string, groep?: string) => ({ tekst, groep, kies: () => {} });
    const menu = { tekst: 'Vastleggen', opties: [optie('Advies', 'GGD Voorbeeld'), optie('Eigen'), optie('Uitspraak', 'Rechtbank Voorbeeld')] };
    const w = await toon({ menu });
    const popup = w.find('[slot="primary-bar"] nldd-button[text="Vastleggen"] nldd-menu');
    expect(popup.findAll(':scope > nldd-menu-item').map((o) => o.attributes('text'))).toEqual(['Eigen']);
    const groepen = popup.findAll('nldd-menu-group');
    expect(groepen.map((g) => g.attributes('text'))).toEqual(['GGD Voorbeeld', 'Rechtbank Voorbeeld']);
    expect(groepen.map((g) => g.findAll('nldd-menu-item').map((o) => o.attributes('text')))).toEqual([['Advies'], ['Uitspraak']]);
  });

  it('keeps the bar of a side without tabs on top on a narrow screen, with its action as an icon button', async () => {
    const actie = { tekst: 'Nieuwe aanvraag', hash: '#nieuw', icoon: 'add' };
    const w = await toon({ tabs: [], titel: 'Vergunningen', actie });
    // From md up the bar as it was, with the action as a button with its text.
    expect(w.find('[slot="primary-bar-md"]').attributes('only')).toBe('md');
    const breed = w.find('[slot="primary-bar"]');
    expect(breed.attributes('above')).toBe('lg');
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
    expect(midden(await toon({ uitvoerder: 'Gemeente Voorbeeld' }))).toEqual(['Gemeente Voorbeeld', 'Gemeente Voorbeeld']);
    expect(midden(await toon({ tabs: [], titel: 'Vergunningen', uitvoerder: 'Gemeente Voorbeeld' }))).toEqual([]);
  });

  it('names the organization under the title of a side without tabs', async () => {
    const onder = (w: Awaited<ReturnType<typeof toon>>) => w.findAll('nldd-toolbar-title').map((t) => t.attributes('supporting-text'));
    expect(onder(await toon({ tabs: [], titel: 'Vergunningen', uitvoerder: 'Gemeente Voorbeeld' }))).toEqual(['Gemeente Voorbeeld', 'Gemeente Voorbeeld', 'Gemeente Voorbeeld']);
    // Not when the title is the organization itself.
    expect(onder(await toon({ tabs: [], titel: 'Gemeente Voorbeeld', uitvoerder: 'Gemeente Voorbeeld' }))).toEqual([undefined, undefined, undefined]);
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
    expect(titels(await toon({ tabs: [] }))).toEqual(['IIT Groningen', 'IIT Groningen', 'IIT Groningen']);
    expect(titels(await toon({}))).toEqual([]);
    expect(titels(await toon({ tabs: [], titel: 'Eigen' }))).toEqual(['Eigen', 'Eigen', 'Eigen']);
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
