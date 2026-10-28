import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { describe, expect, it } from 'vitest';
import { bronAntwoord, lokaleEnv } from '../scripts/bron-dev.js';

describe('bronAntwoord', () => {
  it('serveert de YAML-bestanden van de lokale map, per pad eronder', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    mkdirSync(join(dir, 'casussen'));
    writeFileSync(join(dir, 'casussen', 'c.yaml'), 'cel: c');
    writeFileSync(join(dir, 'notities.txt'), 'nee');
    const [status, type, tekst] = bronAntwoord('napp', { [lokaleEnv('napp')]: dir });
    rmSync(dir, { recursive: true });
    expect([status, type]).toEqual([200, 'application/json']);
    expect(JSON.parse(tekst)).toEqual({ naam: 'napp', versie: 'lokaal', bestanden: { 'casussen/c.yaml': 'cel: c' } });
  });

  it('maakt de bron uit een manifest, met de langste map per bestand', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    const schrijf = (pad, tekst) => {
      mkdirSync(join(dir, pad, '..'), { recursive: true });
      writeFileSync(join(dir, pad), tekst);
    };
    schrijf(
      'werk/m/bron.yaml',
      'wortel: ../..\nmappen:\n  regulation/nl/w: regulation/nl/w\n  cells/c: werk/m/cellen/c\n  cells/c/reductions: werk/m/cellen/c/reducties\n',
    );
    schrijf('regulation/nl/w/2026-01-01.yaml', '$id: w');
    schrijf('werk/m/cellen/c/cel.yaml', 'name: C');
    schrijf('werk/m/cellen/c/reducties/r/2026-01-01.yaml', '$id: r');
    schrijf('werk/groot/niet.yaml', 'niet genoemd');
    const [status, , tekst] = bronAntwoord('napp', { [lokaleEnv('napp')]: join(dir, 'werk/m/bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(200);
    expect(Object.keys(JSON.parse(tekst).bestanden).sort()).toEqual([
      'cells/c/cel.yaml',
      'cells/c/reductions/r/2026-01-01.yaml',
      'regulation/nl/w/2026-01-01.yaml',
    ]);
  });

  it('weigert een manifest met een map boven de wortel, zoals het portaal', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    writeFileSync(join(dir, 'bron.yaml'), 'mappen:\n  cells/c: ../elders\n');
    const [status, , tekst] = bronAntwoord('napp', { [lokaleEnv('napp')]: join(dir, 'bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(502);
    expect(tekst).toContain('no directory below the root');
  });

  it('weigert een manifest dat een map twee keer noemt, zoals het portaal', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    writeFileSync(join(dir, 'bron.yaml'), 'mappen:\n  a: b\n  c: b/\n');
    const [status, , tekst] = bronAntwoord('napp', { [lokaleEnv('napp')]: join(dir, 'bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(502);
    expect(tekst).toContain('named twice');
  });

  it('zegt zonder POC_BRON_<NAAM> dat de bron niet beschikbaar is', () => {
    expect(lokaleEnv('mijn-bron')).toBe('POC_BRON_MIJN_BRON');
    const [status, , tekst] = bronAntwoord('napp', {});
    expect(status).toBe(503);
    expect(tekst).toBe('bron niet beschikbaar: POC_BRON_NAPP is niet gezet');
  });
});
