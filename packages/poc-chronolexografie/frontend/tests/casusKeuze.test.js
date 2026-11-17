// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createApp, nextTick } from 'vue';

// One own case, one from an open source and one from a hidden source, which
// also failed to load: its notice must stay off the screen too.
const casus = (id, bron) => ({ id, titel: id, cel_naam: 'cel', rol: 'het college', besluit: 'wet#1', bron });
vi.mock('../src/api.js', () => ({
  api: {
    casussen: async () => ({
      casussen: [casus('eigen'), casus('open-casus', 'open'), casus('dichte-casus', 'dicht')],
      bronnen: [
        { naam: 'open', fouten: [], meldingen: [] },
        { naam: 'dicht', fouten: ['dicht.yaml laadt niet'], meldingen: [] },
      ],
    }),
  },
  bronLijst: async () => [
    { naam: 'open', verborgen: false },
    { naam: 'dicht', verborgen: true },
  ],
}));
vi.mock('../src/navigation.js', () => ({ navigate: vi.fn() }));

const { default: CasusKeuze } = await import('../src/components/CasusKeuze.vue');
const { useAlleCasussen } = await import('../src/verborgen.js');

let app = null;
async function mount() {
  const el = document.createElement('div');
  document.body.append(el);
  app = createApp(CasusKeuze);
  app.mount(el);
  for (let i = 0; i < 10; i++) await nextTick();
  await new Promise((r) => setTimeout(r));
  await nextTick();
  return {
    cases: [...el.querySelectorAll('nldd-card nldd-title[heading-level="2"]')].map((t) => t.getAttribute('text')),
    tags: [...el.querySelectorAll('nldd-card nldd-tag')].map((t) => t.getAttribute('text')),
    banners: [...el.querySelectorAll('nldd-banner')].map((b) => b.getAttribute('text')),
  };
}

afterEach(() => {
  app?.unmount();
  document.body.innerHTML = '';
  useAlleCasussen().alles.value = false;
});

describe('the choice screen', () => {
  it('leaves out the cases and the notices of a hidden source', async () => {
    const { cases, tags, banners } = await mount();
    expect(cases).toEqual(['eigen', 'open-casus']);
    expect(tags).toEqual([]);
    expect(banners).toEqual([]);
  });

  it('shows them, tagged, once this browser asks for all cases', async () => {
    useAlleCasussen().alles.value = true;
    const { cases, tags, banners } = await mount();
    expect(cases).toEqual(['eigen', 'open-casus', 'dichte-casus']);
    expect(tags).toEqual(['Verborgen casus']);
    expect(banners).toHaveLength(1);
  });
});
