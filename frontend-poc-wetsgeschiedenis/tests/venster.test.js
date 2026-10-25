// Het venster 'Wet ophalen' tegen een nagebootste dienst: zoeken, ophalen en de
// rij volgen, de diagnose, en een fout van het portaal die geen JSON is. En dat
// wat van buiten komt als tekst in beeld komt, nooit als opmaak.
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import OphaalVenster from '../src/OphaalVenster.vue';

const json = (x, status = 200) => ({ status, ok: status < 400, text: async () => JSON.stringify(x) });
let antwoorden;
let gevraagd;

beforeEach(() => {
  gevraagd = [];
  antwoorden = {
    'api/status': () => json({ ophalen: true, taak: null, rij: { huidig: { id: '2', wat: 'dossier', dossier: '36382', voor: 'BWBR0045754', status: 'bezig' }, wachtend: [] } }),
    'api/regeling/BWBR0045754': () => json({
      bwb: 'BWBR0045754', titel: 'Wet <img src=x onerror=alert(1)> open overheid', afkorting: 'Woo', keten: false,
      versies: [{ inwerking: '2023-04-01', betreft: 'Wijziging', staatsblad: 'stb-2024-391', dossiers: [{ nr: '36481', beschikbaar: false, rij: null }] }],
    }),
    'api/haal': () => json({ id: '1', wat: 'dossier', bwb: 'BWBR0045754', dossier: '36481', voor: 'BWBR0045754', status: 'wacht', plaats: 1, log: [] }, 202),
    'api/taak?id=1': () => json({ id: '1', wat: 'dossier', bwb: 'BWBR0045754', dossier: '36481', voor: 'BWBR0045754', status: 'klaar', uitkomst: '36481', log: ['Dossier 36481 ophalen', 'Klaar'] }),
    'api/diagnose?bwb=BWBR0045754': () => json({
      oordeel: 'ophalen faalt: gegevensmagazijn.tweedekamer.nl niet bereikbaar', curl: 'curl 8', proxy: {}, werker: { ok: true, python: '3.12' },
      data: { pad: '/data', schrijfbaar: true, vrij_mb: 900 }, dossiers: { in_het_image: 3, opgehaald: 0, max: 150 },
      hosts: { 'gegevensmagazijn.tweedekamer.nl': { ok: false, uitleg: 'de host is niet te vinden (DNS)', waarvoor: 'nodig' } },
    }),
    'api/log': () => json([{ t: '2026-10-21T10:00:00', soort: 'uit', host: 'gegevensmagazijn.tweedekamer.nl', pad: '/OData', code: 6, uitleg: 'DNS', ms: 12 }]),
  };
  vi.stubGlobal('fetch', vi.fn(async (url) => {
    const pad = url.replace(/^\//, '');
    gevraagd.push(pad);
    return (antwoorden[pad] ?? (() => json({ fout: 'onbekend' }, 404)))();
  }));
  document.body.innerHTML = '<span id="ophalen-plek" class="verborgen"><button id="ophalen">Wet ophalen</button></span>';
  HTMLDialogElement.prototype.showModal ??= function () { this.setAttribute('open', ''); };
});
afterEach(() => vi.unstubAllGlobals());

async function venster() {
  const w = mount(OphaalVenster, { attachTo: document.body });
  await flushPromises();
  return w;
}
const klik = async (w, tekst) => {
  const knop = w.findAll('nldd-button, button').find((b) => (b.attributes('text') || b.text()).includes(tekst));
  await knop.trigger('click');
  await flushPromises();
};

describe('het venster Wet ophalen', () => {
  test('toont de knop alleen als de dienst er is', async () => {
    await venster();
    expect(document.getElementById('ophalen-plek').classList.contains('verborgen')).toBe(false);
  });

  test('zoekt, haalt op en volgt de opdracht; de titel van buiten blijft tekst', async () => {
    const w = await venster();
    await w.find('input').setValue('bwbr0045754');
    await klik(w, 'Zoek');
    expect(w.find('h3').text()).toContain('Wet <img src=x onerror=alert(1)> open overheid (Woo)');
    expect(w.find('img').exists()).toBe(false);
    expect(w.findAll('button.ophaal-actie').map((b) => b.text())).toEqual(['Haal de wet op', 'Haal nu op: 36481']);
    await klik(w, 'Haal nu op: 36481');
    expect(w.find('h3').text()).toBe('Klaar: dossier 36481');
    expect(w.find('.ophaal-log').text()).toContain('Dossier 36481 ophalen');
    expect(w.text()).toContain('Op de achtergrond nog 1 Kamerdossier voor BWBR0045754: 36382 (nu)');
    w.unmount();
  });

  test('een fout BWB-id vraagt de dienst niets', async () => {
    const w = await venster();
    await w.find('input').setValue('BWB123');
    await klik(w, 'Zoek');
    expect(w.find('.ophaal-fout').text()).toBe('Een BWB-id ziet eruit als BWBR0007376.');
    expect(gevraagd.some((p) => p.startsWith('api/regeling'))).toBe(false);
    w.unmount();
  });

  test('de diagnose toont het oordeel, de hosts en het logboek', async () => {
    const w = await venster();
    await w.find('input').setValue('BWBR0045754');
    await klik(w, 'Diagnose');
    expect(w.find('h3').text()).toBe('ophalen faalt: gegevensmagazijn.tweedekamer.nl niet bereikbaar');
    expect(w.find('tbody tr').text()).toContain('nee: de host is niet te vinden (DNS)');
    expect(w.find('.ophaal-log').text()).toContain('gegevensmagazijn.tweedekamer.nl/OData  curl 6');
    w.unmount();
  });

  test('een 502 van het portaal, platte tekst, wordt een leesbare melding', async () => {
    antwoorden['api/regeling/BWBR0045754'] = () => ({ status: 502, ok: false, text: async () => 'De achterliggende omgeving antwoordde niet.' });
    const w = await venster();
    await w.find('input').setValue('BWBR0045754');
    await klik(w, 'Zoek');
    expect(w.find('.ophaal-fout').text()).toMatch(/^Het portaal kreeg geen antwoord van de dienst \(HTTP 502, na [\d.]+ s\): De achterliggende omgeving antwoordde niet\./);
    w.unmount();
  });
});
