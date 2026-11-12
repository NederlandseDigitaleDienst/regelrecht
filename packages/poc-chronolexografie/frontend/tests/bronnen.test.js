import { describe, expect, it } from 'vitest';
import { bronMeldingen, haalBron, haalBronnen, herkomst, leesBronLijst } from '../src/bronnen.js';

const antwoord = (status, body) => ({
  ok: status >= 200 && status < 300,
  status,
  statusText: '',
  text: async () => (typeof body === 'string' ? body : JSON.stringify(body)),
  json: async () => body,
});

describe('haalBron', () => {
  it('geeft de bestanden en de versie van een bron', async () => {
    const haal = async () => antwoord(200, { naam: 'voorbeeld', versie: 'abc1234', bestanden: { 'a.yaml': 'a: 1' } });
    expect(await haalBron('voorbeeld', '/bron/voorbeeld', haal)).toEqual({
      naam: 'voorbeeld',
      versie: 'abc1234',
      bestanden: { 'a.yaml': 'a: 1' },
      melding: null,
    });
  });

  it('neemt de reden van het portaal over als de bron er niet is', async () => {
    const haal = async () => antwoord(503, 'bron niet beschikbaar: VOORBEELD_TOKEN is niet gezet');
    expect(await haalBron('voorbeeld', '/bron/voorbeeld', haal)).toEqual({
      naam: 'voorbeeld',
      fout: 'bron niet beschikbaar: VOORBEELD_TOKEN is niet gezet',
    });
  });

  it('toont bij een verlopen cookie geen inlogscherm maar een korte reden', async () => {
    const haal = async () => antwoord(401, '<!doctype html><html>…</html>');
    expect((await haalBron('voorbeeld', '/x', haal)).fout).toBe('bron niet beschikbaar: log opnieuw in op deze poc');
  });

  it('maakt van een kale 404 of een netwerkfout ook "bron niet beschikbaar"', async () => {
    expect((await haalBron('voorbeeld', '/x', async () => antwoord(404, 'Niet gevonden'))).fout).toBe(
      'bron niet beschikbaar: 404 Niet gevonden',
    );
    const stuk = async () => {
      throw new Error('offline');
    };
    expect((await haalBron('voorbeeld', '/x', stuk)).fout).toBe('bron niet beschikbaar: offline');
  });
});

describe('leesBronLijst', () => {
  it('leest per bron of zij verborgen is, en zonder lijst geen bron', async () => {
    const lijst = [{ naam: 'open' }, { naam: 'dicht', verborgen: true }];
    expect(await leesBronLijst('/bronnen.json', async () => antwoord(200, lijst))).toEqual([
      { naam: 'open', verborgen: false },
      { naam: 'dicht', verborgen: true },
    ]);
    expect(await leesBronLijst('/bronnen.json', async () => antwoord(404, ''))).toEqual([]);
  });

  it('slaat een onleesbare regel over in plaats van te stoppen', async () => {
    expect(await leesBronLijst('/bronnen.json', async () => antwoord(200, [null, 3, { naam: 'open' }]))).toEqual([
      { naam: 'open', verborgen: false },
    ]);
  });

  it('neemt een kale naam van een oudere build als een bron die niet verborgen is', async () => {
    expect(await leesBronLijst('/bronnen.json', async () => antwoord(200, ['voorbeeld']))).toEqual([
      { naam: 'voorbeeld', verborgen: false },
    ]);
  });
});

describe('haalBronnen', () => {
  it('haalt elke bron uit de lijst op, ook een verborgen bron', async () => {
    const haal = async () => antwoord(200, { versie: 'v', bestanden: {} });
    const lijst = [
      { naam: 'voorbeeld', verborgen: false },
      { naam: 'verborgen', verborgen: true },
    ];
    const bronnen = await haalBronnen(lijst, (n) => `/bron/${n}`, haal);
    expect(bronnen.map((b) => b.naam)).toEqual(['voorbeeld', 'verborgen']);
    // The cell refuses an unknown field (src/bundel.rs, `Bron`).
    expect(bronnen.every((b) => !('verborgen' in b))).toBe(true);
    expect(await haalBronnen([], (n) => n, haal)).toEqual([]);
  });
});

describe('bronMeldingen', () => {
  it('meldt een bron die er niet is als waarschuwing en een kapotte als kritiek', () => {
    const m = bronMeldingen([
      { naam: 'voorbeeld', fout: 'bron niet beschikbaar: geen token' },
      { naam: 'x', fout: 'bron/x/regulation/nl/w/2026.yaml: expected a mapping' },
      { naam: 'gh', fout: 'bron kon niet worden opgehaald: GitHub: 502' },
      { naam: 'stil', versie: 'v', fouten: [], meldingen: [] },
    ]);
    expect(m.map((x) => [x.naam, x.variant, x.tekst])).toEqual([
      ['voorbeeld', 'warning', 'Bron voorbeeld niet beschikbaar'],
      ['x', 'critical', 'Bron x laadt niet'],
      ['gh', 'warning', 'Bron gh niet beschikbaar'],
    ]);
    expect(m[1].regels[0]).toContain('bron/x/regulation/nl/w/2026.yaml');
  });

  it('telt de meldingen van een bron die wel laadt', () => {
    const [m] = bronMeldingen([{ naam: 'voorbeeld', versie: 'abc', fouten: ["case 'a': kapot"], meldingen: ['dubbel'] }]);
    expect(m).toMatchObject({ variant: 'critical', tekst: 'Bron voorbeeld (abc): 2 meldingen' });
  });
});

describe('herkomst', () => {
  it('noemt de bron en haar versie bij een casus uit een bron', () => {
    expect(herkomst({ bron: 'voorbeeld' }, [{ naam: 'voorbeeld', versie: 'abc1234' }])).toBe(' · uit bron voorbeeld (abc1234)');
    expect(herkomst({ bron: null }, [])).toBe('');
  });
});
