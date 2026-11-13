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
    const [status, type, tekst] = bronAntwoord('voorbeeld', { [lokaleEnv('voorbeeld')]: dir });
    rmSync(dir, { recursive: true });
    expect([status, type]).toEqual([200, 'application/json']);
    expect(JSON.parse(tekst)).toEqual({ naam: 'voorbeeld', versie: 'lokaal', bestanden: { 'casussen/c.yaml': 'cel: c' } });
  });

  it('maakt de bron uit een manifest, met de langste map per bestand', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    const schrijf = (pad, tekst) => {
      mkdirSync(join(dir, pad, '..'), { recursive: true });
      writeFileSync(join(dir, pad), tekst);
    };
    schrijf(
      'werk/m/bron.yaml',
      'repo_root: ../..\ndirectories:\n  regulation/nl/w: regulation/nl/w\n  cells/c: werk/m/cellen/c\n  cells/c/reductions: werk/m/cellen/c/reducties\n',
    );
    schrijf('regulation/nl/w/2026-01-01.yaml', '$id: w');
    schrijf('werk/m/cellen/c/cel.yaml', 'name: C');
    schrijf('werk/m/cellen/c/reducties/r/2026-01-01.yaml', '$id: r');
    schrijf('werk/groot/niet.yaml', 'niet genoemd');
    const [status, , tekst] = bronAntwoord('voorbeeld', { [lokaleEnv('voorbeeld')]: join(dir, 'werk/m/bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(200);
    expect(Object.keys(JSON.parse(tekst).bestanden).sort()).toEqual([
      'cells/c/cel.yaml',
      'cells/c/reductions/r/2026-01-01.yaml',
      'regulation/nl/w/2026-01-01.yaml',
    ]);
  });

  it('places the UI spec the manifest names at the root of the source, as the portal does', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    const schrijf = (pad, tekst) => {
      mkdirSync(join(dir, pad, '..'), { recursive: true });
      writeFileSync(join(dir, pad), tekst);
    };
    schrijf('werk/m/bron.yaml', 'repo_root: ../..\ndirectories:\n  casussen: werk/m/casussen\nui_spec: werk/ui-spec.yaml\n');
    schrijf('werk/m/casussen/k.yaml', 'cel: c');
    schrijf('werk/ui-spec.yaml', 'elementen: []');
    const env = { [lokaleEnv('voorbeeld')]: join(dir, 'werk/m/bron.yaml') };
    const [status, , tekst] = bronAntwoord('voorbeeld', env);
    expect(status).toBe(200);
    expect(JSON.parse(tekst).bestanden['ui_spec.yaml']).toBe('elementen: []');
    // A UI spec that is not there is named; one above the root is refused.
    rmSync(join(dir, 'werk/ui-spec.yaml'));
    const [zonder, , fout] = bronAntwoord('voorbeeld', env);
    expect(zonder).toBe(502);
    expect(fout).toContain("'werk/ui-spec.yaml' for its UI spec");
    schrijf('werk/m/bron.yaml', 'directories:\n  casussen: werk/m/casussen\nui_spec: ../u.yaml\n');
    const [boven, , waarom] = bronAntwoord('voorbeeld', env);
    expect(boven).toBe(502);
    expect(waarom).toContain('below the root');
    for (const [regel, verwacht] of [
      ['ui_spec: werk/u.json', 'no YAML file'],
      ['ui_spec: werk/m/casussen/u.yaml', "lies in 'werk/m/casussen'"],
    ]) {
      schrijf('werk/m/bron.yaml', `repo_root: ../..\ndirectories:\n  casussen: werk/m/casussen\n${regel}\n`);
      const [status, , fout] = bronAntwoord('voorbeeld', env);
      expect(status).toBe(502);
      expect(fout).toContain(verwacht);
    }
    rmSync(dir, { recursive: true });
  });

  it('rejects a manifest with a directory above the repository root, as the portal does', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    writeFileSync(join(dir, 'bron.yaml'), 'directories:\n  cells/c: ../elders\n');
    const [status, , tekst] = bronAntwoord('voorbeeld', { [lokaleEnv('voorbeeld')]: join(dir, 'bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(502);
    expect(tekst).toContain('no directory below the root');
  });

  it('weigert een manifest dat een map twee keer noemt, zoals het portaal', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    writeFileSync(join(dir, 'bron.yaml'), 'directories:\n  a: b\n  c: b/\n');
    const [status, , tekst] = bronAntwoord('voorbeeld', { [lokaleEnv('voorbeeld')]: join(dir, 'bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(502);
    expect(tekst).toContain('named twice');
  });

  it('still reads the old Dutch field names wortel and mappen', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    mkdirSync(join(dir, 'werk', 'w'), { recursive: true });
    writeFileSync(join(dir, 'werk', 'bron.yaml'), 'wortel: ..\nmappen:\n  regulation/nl/w: werk/w\n');
    writeFileSync(join(dir, 'werk', 'w', '2026-01-01.yaml'), '$id: w');
    const [status, , tekst] = bronAntwoord('voorbeeld', { [lokaleEnv('voorbeeld')]: join(dir, 'werk', 'bron.yaml') });
    rmSync(dir, { recursive: true });
    expect(status).toBe(200);
    expect(Object.keys(JSON.parse(tekst).bestanden)).toEqual(['regulation/nl/w/2026-01-01.yaml']);
  });

  it('rejects a manifest without directories, or with a field under both names, as the portal does', () => {
    const dir = mkdtempSync(join(tmpdir(), 'bron-dev-'));
    const antwoord = (yaml) => {
      writeFileSync(join(dir, 'bron.yaml'), yaml);
      return bronAntwoord('voorbeeld', { [lokaleEnv('voorbeeld')]: join(dir, 'bron.yaml') });
    };
    const zonder = antwoord('repo_root: ..\n');
    const dubbel = antwoord('repo_root: ..\nwortel: ..\ndirectories: {a: b}\n');
    rmSync(dir, { recursive: true });
    expect(zonder[0]).toBe(502);
    expect(zonder[2]).toContain('missing field `directories`');
    expect(dubbel[0]).toBe(502);
    expect(dubbel[2]).toContain('duplicate field `repo_root`');
  });

  it('zegt zonder POC_BRON_<NAAM> dat de bron niet beschikbaar is', () => {
    expect(lokaleEnv('mijn-bron')).toBe('POC_BRON_MIJN_BRON');
    const [status, , tekst] = bronAntwoord('voorbeeld', {});
    expect(status).toBe(503);
    expect(tekst).toBe('bron niet beschikbaar: POC_BRON_VOORBEELD is niet gezet');
  });
});
