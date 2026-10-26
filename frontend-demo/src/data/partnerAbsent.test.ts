import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import { materialiseRecord, tablesFromProfiles } from './materialize.js';
import type { LawShape } from './materialize.js';
import type { BindingsDocument, ProfilesDocument } from './loadCorpus';

/**
 * A citizen without a partner must get "there is none" for every value that is
 * looked up on the partner's BSN — not "nobody knows".
 *
 * The huurtoeslag reported "De wet kan nog geen uitkomst geven — er ontbreekt:
 * partner buitenlands inkomen" for a persona whose own card said "Heeft
 * partner: Nee". The form cannot ask for it either (a register input is not a
 * question), so the application could not be completed at all.
 */

const root = path.resolve(import.meta.dirname, '../../..');
const bindings = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/bindings.yaml'), 'utf8')) as BindingsDocument;
const profiles = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/profiles.yaml'), 'utf8')) as ProfilesDocument;

/** The persona of the report: no partner, and a foreign-income row of his own. */
const BSN = '999100001';

const rowsFor = tablesFromProfiles(profiles);

/** The BRP row of the persona, to prove the premise: he has no partner. */
function brpRow(bsn: string): Record<string, unknown> | null {
  return rowsFor('RvIG', 'personen').find((r: Record<string, unknown>) => r.bsn === bsn) ?? null;
}

describe('a partner-keyed register value for someone without a partner', () => {
  it('finds the persona of the report, who has no partner', () => {
    const person = brpRow(BSN);
    expect(person).toBeTruthy();
    expect(person?.partner_bsn ?? null).toBeNull();
  });

  it('resolves partner_buitenlands_inkomen to absent, not to unknown', () => {
    const lawBindings = bindings['wet_inkomstenbelasting'];
    expect(lawBindings?.partner_buitenlands_inkomen).toBeTruthy();

    const { record } = materialiseRecord(
      // No parameters: the record is keyed on the bsn passed below.
      { id: 'wet_inkomstenbelasting', inputTypes: { partner_buitenlands_inkomen: 'amount' } } as Omit<LawShape, 'parameters'> as LawShape,
      // Asserted to exist just above.
      lawBindings!,
      { bsn: BSN },
      rowsFor,
      {},
    );

    // Present as a key with value null = "the register says there is none".
    // A key left out entirely = unknown, which is what blocked the huurtoeslag.
    expect(Object.hasOwn(record, 'partner_buitenlands_inkomen')).toBe(true);
    expect(record.partner_buitenlands_inkomen).toBeNull();
  });
});
