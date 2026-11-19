// @vitest-environment happy-dom
// A field of the decision form with `metArtikel` (VeldInvoer.vue): the article
// it rests on shows as a link that opens the article in the sheet
// (ArtikelLink.vue, herkomst.js). The nldd-* elements are not defined here, so
// the test reads their attributes.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createApp, h, nextTick } from 'vue';
import VeldInvoer from '../src/components/VeldInvoer.vue';
import { getoond } from '../src/herkomst.js';
import { state } from '../src/state.js';

let app = null;
let el = null;

async function mount(veld, metArtikel) {
  el = document.createElement('div');
  document.body.append(el);
  app = createApp({ render: () => h(VeldInvoer, { veld, metArtikel }) });
  app.mount(el);
  await nextTick();
}

// FICTIEF: a law with one deciding article.
beforeEach(() => {
  state.casus = { regelwerken: [{ id: 'voorbeeldwet', naam: 'Voorbeeldwet', map: 'wet/voorbeeldwet' }] };
  getoond.value = null;
});

afterEach(() => {
  app?.unmount();
  el?.remove();
});

describe('VeldInvoer met artikel', () => {
  it('links a switch to the article it rests on', async () => {
    await mount({ naam: 'is_ingezetene', type: 'boolean', basis: 'voorbeeldwet#2' }, true);
    const link = el.querySelector('nldd-link');
    expect(link.getAttribute('text')).toBe('Voorbeeldwet artikel 2');
    link.dispatchEvent(new Event('click'));
    expect(getoond.value).toMatchObject({ soort: 'wet', wet: 'voorbeeldwet', artikel: '2' });
  });

  it('links a text field to its article in place of the bare basis', async () => {
    await mount({ naam: 'reden', type: 'string', basis: 'voorbeeldwet#2' }, true);
    expect(el.querySelector('nldd-form-field').hasAttribute('supporting-label')).toBe(false);
    expect(el.querySelector('nldd-form-field-help-text nldd-link').getAttribute('text')).toBe(
      'Voorbeeldwet artikel 2',
    );
  });

  it('shows no link without metArtikel', async () => {
    await mount({ naam: 'is_ingezetene', type: 'boolean', basis: 'voorbeeldwet#2' }, false);
    expect(el.querySelector('nldd-link')).toBeNull();
    app.unmount();
    el.remove();
    await mount({ naam: 'reden', type: 'string', basis: 'voorbeeldwet#2' }, false);
    expect(el.querySelector('nldd-link')).toBeNull();
    expect(el.querySelector('nldd-form-field').getAttribute('supporting-label')).toBe('voorbeeldwet#2');
  });
});
