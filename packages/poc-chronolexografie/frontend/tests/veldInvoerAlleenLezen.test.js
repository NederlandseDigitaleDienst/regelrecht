// @vitest-environment happy-dom
// A field the system fills (VeldInvoer.vue with `alleenLezen`): its value
// shows, and the applicant cannot change it. The nldd-* elements are not
// defined here, so the test reads their attributes.
import { afterEach, describe, expect, it } from 'vitest';
import { createApp, h, nextTick } from 'vue';
import VeldInvoer from '@regelrecht/frontend-shared/components/chronolex/VeldInvoer.vue';
import { BESTUUR, RIJEN } from './fixtureTabel.js';

let app = null;
let el = null;

async function mount(veld, modelValue, alleenLezen) {
  el = document.createElement('div');
  document.body.append(el);
  app = createApp({ render: () => h(VeldInvoer, { veld, modelValue, alleenLezen }) });
  app.mount(el);
  await nextTick();
}

afterEach(() => {
  app?.unmount();
  el?.remove();
});

describe('VeldInvoer alleen-lezen', () => {
  it('shows a text and a date the system fills, read-only', async () => {
    await mount({ naam: 'gevraagde_beschikking', type: 'string' }, 'r#107', true);
    const tekst = el.querySelector('nldd-text-field');
    expect(tekst.hasAttribute('readonly')).toBe(true);
    app.unmount();
    el.remove();
    await mount({ naam: 'dagtekening', type: 'date' }, '2028-03-06', true);
    expect(el.querySelector('nldd-date-field').hasAttribute('readonly')).toBe(true);
  });

  it('disables a switch the system fills', async () => {
    await mount({ naam: 'elektronisch', type: 'boolean' }, true, true);
    expect(el.querySelector('nldd-switch-field').hasAttribute('disabled')).toBe(true);
  });

  it('shows a table the system fills without its buttons', async () => {
    await mount(BESTUUR, RIJEN, true);
    expect(el.querySelectorAll('nldd-table-row:not([slot])').length).toBe(RIJEN.length);
    expect(el.querySelector('nldd-button[text="Rij toevoegen"]')).toBeNull();
    expect(el.querySelector('nldd-icon-button')).toBeNull();
  });

  it('leaves a field the applicant fills editable', async () => {
    await mount({ naam: 'naam', type: 'string' }, 'x', false);
    expect(el.querySelector('nldd-text-field').hasAttribute('readonly')).toBe(false);
  });
});
