// De bron: hoe de app bestanden vindt, en wat hij doet als de bron er niet is.
import { mkdtempSync, mkdirSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { describe, expect, it } from 'vitest';
import { bestand, json, laadBron, zetBron } from './bron.js';
import { b } from './eerste/basePad.js';
import { bronAntwoord, lokaleEnv } from '../scripts/bron-dev.js';

describe('bestand', () => {
  it('vindt een YAML-bestand op zijn naam en een ander bestand met .yaml erachter', () => {
    zetBron({ bestanden: { 'regels/a.yaml': '$id: a', 'demo.json.yaml': '{"x": 1}' } });
    expect(bestand('regels/a.yaml')).toBe('$id: a');
    expect(json('demo.json')).toEqual({ x: 1 });
    expect(json('./demo.json')).toEqual({ x: 1 });
  });

  it('noemt het pad als een bestand ontbreekt', () => {
    zetBron({ bestanden: {} });
    expect(() => bestand('data/testpunten.json')).toThrow('niet in de bron: data/testpunten.json');
  });

  it('weigert zolang de bron niet geladen is', () => {
    zetBron(null);
    expect(() => bestand('demo.json')).toThrow('nog niet geladen');
  });
});

describe('laadBron', () => {
  it('geeft de reden van het portaal door als de bron er niet is', async () => {
    zetBron(null);
    const haal = async () => new Response('bron niet beschikbaar: SPOORZONE_CORPUS_TOKEN is niet gezet', { status: 503 });
    await expect(laadBron(haal)).rejects.toThrow('SPOORZONE_CORPUS_TOKEN is niet gezet');
  });

  it('haalt de bron op onder de basis van de app en laadt de bestanden uit het antwoord', async () => {
    zetBron(null);
    const gevraagd = [];
    const haal = async (url) => {
      gevraagd.push(url);
      return Response.json({ naam: 'spoorzone', versie: 'abc1234', bestanden: { 'a.yaml': 'x: 1' } });
    };
    expect((await laadBron(haal)).versie).toBe('abc1234');
    expect(gevraagd).toEqual([b('bron/spoorzone')]);
    expect(bestand('a.yaml')).toBe('x: 1');
  });

  it('zegt het als de bron geen JSON geeft, en onthoudt dan niets', async () => {
    zetBron(null);
    const haal = async () => new Response('<!doctype html>', { status: 200 });
    await expect(laadBron(haal)).rejects.toThrow('de bron gaf geen JSON');
    expect(() => bestand('a.yaml')).toThrow('nog niet geladen');
  });
});

describe('de bron op de dev-server', () => {
  it('antwoordt 503 zonder POC_BRON_SPOORZONE, zoals het portaal zonder token', () => {
    expect(lokaleEnv('spoorzone')).toBe('POC_BRON_SPOORZONE');
    expect(bronAntwoord('spoorzone', {})[0]).toBe(503);
  });

  it('serveert alleen de YAML-bestanden uit de map, per pad', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-'));
    mkdirSync(join(dir, 'regels'));
    writeFileSync(join(dir, 'regels', 'a.yaml'), '$id: a');
    writeFileSync(join(dir, 'demo.json.yaml'), '{}');
    writeFileSync(join(dir, 'leesmij.txt'), 'niet mee');
    const [status, , tekst] = bronAntwoord('spoorzone', { POC_BRON_SPOORZONE: dir });
    expect(status).toBe(200);
    expect(Object.keys(JSON.parse(tekst).bestanden).sort()).toEqual(['demo.json.yaml', 'regels/a.yaml']);
  });
});
