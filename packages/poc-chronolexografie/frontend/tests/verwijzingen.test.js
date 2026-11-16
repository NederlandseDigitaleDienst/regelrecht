// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createApp, h, nextTick } from 'vue';

// The cell is not loaded here: the chronicles are set on `state` directly.
vi.mock('../src/api.js', () => ({ api: {} }));

const { state } = await import('../src/state.js');
const { doel, verwezenDoor, verwijzingen, zoekGram } = await import('../src/verwijzingen.js');
const { default: GramCode } = await import('../src/components/GramCode.vue');

const id = (n) => `01a1e3bc-2019-7606-acbc-${String(n).padStart(12, '0')}`;
const gram = (n, rest) => ({
  id: id(n),
  basis: 'wet#1',
  recording_actor: 'school',
  recorded_on: '2026-10-14',
  fields: {},
  ...rest,
});

const aanvraag = gram(1, { kind: 'AANVRAAG' });
const besluit = gram(2, { kind: 'BESCHIKKING', refers_to: id(1) });
// The cell fills the contested decision in a field too: named once.
const bezwaar = gram(3, { kind: 'BEZWAAR', refers_to: id(2), fields: { bestreden_besluit: id(2) } });
// In the chronicle of the court, holding a copy of the decision.
const beroep = gram(4, { kind: 'BEROEP', recording_actor: 'rechtbank', received: { from: 'school', id: id(2) } });
// Refers to a gram no chronicle of the app holds.
const los = gram(5, { kind: 'EXECUTOGRAM', fact: 'ontvangst', refers_to: id(99) });

beforeEach(() => {
  Object.assign(state, {
    casus: { cel: 'school', cel_naam: 'OBS De Klimop', andere_cellen: [{ cel: 'rechtbank', cel_naam: 'Rechtbank Wilgendaal' }] },
    grammen: [aanvraag, besluit, bezwaar, los],
    andere: [{ cel: 'rechtbank', cel_naam: 'Rechtbank Wilgendaal', grammen: [beroep] }],
    vormen: null,
    inKroniek: null,
  });
});

describe('references between chronolexograms', () => {
  it('finds a gram in any chronicle of the app', () => {
    expect(zoekGram(id(4))).toEqual({ gram: beroep, cel: 'rechtbank' });
    expect(zoekGram(id(99))).toBeNull();
  });

  it('names where an id leads by kind, short id and basis or fact', () => {
    expect(doel(id(2), 'school')).toEqual({
      id: id(2),
      gevonden: true,
      tekst: 'BESCHIKKING 00000002 · wet#1',
      kroniek: null,
    });
    expect(doel(id(2), 'rechtbank').kroniek).toBe('OBS De Klimop');
    expect(doel(id(99), 'school')).toMatchObject({ gevonden: false, tekst: 'chronolexogram 00000099' });
  });

  it('names each id once, under the first field that holds it', () => {
    expect(verwijzingen(bezwaar).map((r) => [r.soort, r.id])).toEqual([['Verwijst naar', id(2)]]);
    const metVeld = gram(6, { kind: 'BEZWAAR', fields: { bestreden_besluit: id(2) } });
    expect(verwijzingen(metVeld).map((r) => r.soort)).toEqual(['Bestreden besluit']);
  });

  it('follows a copy received from another cell into its chronicle', () => {
    const [r] = verwijzingen(beroep);
    expect(r.soort).toBe('Ontvangen van OBS De Klimop');
    expect(r.doel).toMatchObject({ gevonden: true, kroniek: 'OBS De Klimop' });
  });

  it('lists what refers to a gram, from every chronicle', () => {
    expect(verwezenDoor(besluit).map((d) => d.id)).toEqual([id(3), id(4)]);
    expect(verwezenDoor(besluit)[1].kroniek).toBe('Rechtbank Wilgendaal');
    expect(verwezenDoor(bezwaar)).toEqual([]);
  });
});

describe('a gram with its references in view', () => {
  let app = null;
  afterEach(() => {
    app?.unmount();
    document.body.innerHTML = '';
  });
  async function mount(g) {
    const el = document.createElement('div');
    document.body.append(el);
    app = createApp({ render: () => h(GramCode, { gram: g }) });
    app.config.compilerOptions.isCustomElement = (tag) => tag.startsWith('nldd-');
    app.mount(el);
    await nextTick();
    return el;
  }
  const regels = (el) => [...el.querySelectorAll('nldd-text')].map((t) => t.textContent.replace(/\s+/g, ' ').trim());

  it('links what it refers to and what refers to it', async () => {
    const el = await mount(besluit);
    expect(regels(el)).toEqual([
      'Verwijst naar:',
      'Verwezen door: , (in de kroniek van Rechtbank Wilgendaal)',
    ]);
    const links = [...el.querySelectorAll('nldd-link')];
    expect(links.map((l) => [l.getAttribute('text'), l.getAttribute('href')])).toEqual([
      ['AANVRAAG 00000001 · wet#1', `#gram-${id(1)}`],
      ['BEZWAAR 00000003 · wet#1', `#gram-${id(3)}`],
      ['BEROEP 00000004 · wet#1', `#gram-${id(4)}`],
    ]);
  });

  it('asks the Kroniek tab to show the gram a link names', async () => {
    const el = await mount(besluit);
    el.querySelector('nldd-link').dispatchEvent(new Event('click', { cancelable: true }));
    expect(state.inKroniek).toBe(id(1));
  });

  it('says so, without a link, when no chronicle holds the gram', async () => {
    const el = await mount(los);
    expect(el.querySelector('nldd-link')).toBeNull();
    expect(regels(el)).toEqual([
      'Verwijst naar: chronolexogram 00000099 (staat niet in een kroniek die de app heeft)',
    ]);
  });
});
