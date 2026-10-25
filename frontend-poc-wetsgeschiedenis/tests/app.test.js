// De app als geheel: Vue zet de opmaak van de bekijker neer, en pagina.js
// tekent er een dossier in, met de gegevens onder de basis van de bundel.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import App from '../src/App.vue';

const gegevens = resolve(__dirname, '..', '..', 'corpus-poc', 'wetsgeschiedenis', 'gegevens');

afterEach(() => vi.unstubAllGlobals());

test('de bekijker tekent een dossier binnen de Vue-app', async () => {
  const gevraagd = [];
  vi.stubGlobal('fetch', vi.fn(async (url) => {
    gevraagd.push(url);
    const m = /^\/gegevens\/([\w-]+\.json)$/.exec(url);
    if (m) return { ok: true, status: 200, json: async () => JSON.parse(readFileSync(resolve(gegevens, m[1]), 'utf8')), text: async () => readFileSync(resolve(gegevens, m[1]), 'utf8') };
    return { ok: false, status: 404, json: async () => ({}), text: async () => '{}' };
  }));
  location.hash = 'd=35968&t=0';
  const w = mount(App, { attachTo: document.body });
  for (let i = 0; i < 100 && !document.querySelector('#artlijst li[data-art]'); i++) {
    await flushPromises();
    await new Promise((r) => setTimeout(r, 20));
  }
  expect(gevraagd).toContain('/gegevens/dossiers.json');
  expect(gevraagd).toContain('/gegevens/35968.json');
  expect(document.querySelectorAll('#dossierkeuze option').length).toBeGreaterThan(0);
  expect(document.querySelectorAll('#artlijst li[data-art]').length).toBeGreaterThan(50);
  // Zonder dienst (api/status geeft 404) blijft de knop 'Wet ophalen' weg.
  expect(document.getElementById('ophalen-plek').classList.contains('verborgen')).toBe(true);
  w.unmount();
});
