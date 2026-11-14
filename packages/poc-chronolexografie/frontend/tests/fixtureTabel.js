import { readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { load } from 'js-yaml';

/**
 * The table of the test source for prefilling (tests/bron-voorinvullen): the
 * field `bestuur` of the application, with the columns its article declares,
 * as the form gets it (kern.rs `veld`), and the rows the register recorded
 * in the example case.
 */
// A path, not a URL: happy-dom replaces the global URL, which fs refuses.
const bron = join(dirname(fileURLToPath(import.meta.url)), '../../tests/bron-voorinvullen');
const lees = (pad) => load(readFileSync(join(bron, pad), 'utf8'));

const artikel = lees('regulation/nl/uitvoeringsbeleid/voorbeeld_inschrijving/2026-01-01.yaml').articles.find(
  (a) => a.number === '1',
);
const parameter = artikel.machine_readable.execution.parameters.find((p) => p.name === 'bestuur');

export const BESTUUR = Object.freeze({
  naam: parameter.name,
  type: parameter.type,
  mag_leeg: parameter.nullable,
  basis: 'voorbeeld_inschrijving#1',
  kolommen: parameter.columns.map((c) => ({ naam: c.name, type: c.type })),
});

export const RIJEN = lees('casussen/voorbeeld_inschrijving.yaml').andere_cellen[0].begin.executogrammen.find(
  (e) => e.feit === 'voorbeeld_register#2#bestuur_vastgesteld',
).velden.bestuur;
