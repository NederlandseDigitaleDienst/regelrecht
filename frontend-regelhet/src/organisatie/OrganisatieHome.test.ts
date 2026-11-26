// The page of an organisation's front: a tile per process, which opens the
// process through the front's hand-off, and a process that did not answer
// keeps its tile without a link. The nldd-* elements are not defined in this
// test, so it reads the attributes Vue sets on them.
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import OrganisatieHome from './OrganisatieHome.vue';
import { bestemming, openAdres } from './api';

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const organisatie = {
  naam: 'Gemeente Wilgendaal',
  logo: null,
  kant: 'burger',
  processen: [
    { id: 'tnt-p/parkeerkaart', naam: 'parkeerkaart', titel: 'Gehandicaptenparkeerkaart', bereikbaar: true },
    { id: 'tnt-b/bedrijfsparkeren', naam: 'bedrijfsparkeren', titel: null, bereikbaar: false },
  ],
};

function antwoord(body: unknown, status = 200) {
  const fetch = vi.fn(
    async () => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } }),
  );
  vi.stubGlobal('fetch', fetch);
  return fetch;
}

async function toon(kant: 'burger' | 'backoffice') {
  const w = mount(OrganisatieHome, { props: { kant } });
  await flushPromises();
  return w;
}

describe('the tiles of an organisation', () => {
  it('asks the front, never a process', async () => {
    const fetch = antwoord(organisatie);
    await toon('burger');
    expect(fetch.mock.calls.map((c) => String((c as unknown[])[0]))).toEqual(['/api/organisatie']);
  });

  it('gives a citizen a tile per process that opens its application form', async () => {
    antwoord(organisatie);
    const w = await toon('burger');
    expect(w.find('nldd-title#organisatie-titel').attributes('text')).toBe('Wat wilt u aanvragen?');
    const kaarten = w.findAll('nldd-card');
    expect(kaarten).toHaveLength(2);
    expect(kaarten[0]?.find('nldd-title').attributes('text')).toBe('Gehandicaptenparkeerkaart');
    expect(kaarten[0]?.attributes('href')).toBe(
      '/api/processen/tnt-p%2Fparkeerkaart/openen?naar=%2Fburger%23nieuwe-aanvraag',
    );
  });

  it('keeps the tile of a process that did not answer, without a link and under its own name', async () => {
    antwoord(organisatie);
    const w = await toon('burger');
    const stil = w.findAll('nldd-card')[1];
    expect(stil?.attributes('href')).toBeUndefined();
    expect(stil?.find('nldd-title').attributes('text')).toBe('bedrijfsparkeren');
    expect(stil?.find('nldd-tag').attributes('text')).toBe('Nu niet bereikbaar');
  });

  it("opens an employee's tile on the process's work queue", () => {
    expect(bestemming('backoffice')).toBe('/backoffice');
    expect(openAdres('tnt-p/parkeerkaart', bestemming('backoffice'))).toBe(
      '/api/processen/tnt-p%2Fparkeerkaart/openen?naar=%2Fbackoffice',
    );
  });

  it('says so when the organisation has no process yet, or the front fails', async () => {
    antwoord({ ...organisatie, processen: [] });
    let w = await toon('backoffice');
    expect(w.find('nldd-inline-dialog').attributes('text')).toBe('Er is hier nog niets');
    vi.spyOn(console, 'error').mockImplementation(() => {});
    antwoord({ fout: 'you may not do this' }, 403);
    w = await toon('backoffice');
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toBe('you may not do this');
  });
});
