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

  it('zegt zonder POC_BRON_<NAAM> dat de bron niet beschikbaar is', () => {
    expect(lokaleEnv('mijn-bron')).toBe('POC_BRON_MIJN_BRON');
    const [status, , tekst] = bronAntwoord('napp', {});
    expect(status).toBe(503);
    expect(tekst).toBe('bron niet beschikbaar: POC_BRON_NAPP is niet gezet');
  });
});
