import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import type { Binding, BindingsDocument } from './loadCorpus';

/**
 * A `select_on` key has to be bound in the same law that does the lookup.
 *
 * `materialiseRecord` resolves a selector against that law's own parameters and
 * its own materialised inputs — nothing else. A law that selects rows on
 * `$partner_bsn` without binding `partner_bsn` therefore finds no key at all,
 * and every value behind it becomes unknown instead of absent.
 *
 * That is what blocked the huurtoeslag: twelve partner lookups in
 * `wet_inkomstenbelasting` selected on `$partner_bsn` while the binding for it
 * lived in `wet_brp`. A citizen whose own card said "Heeft partner: Nee" was
 * told the law could not decide because his partner's foreign income was
 * unknown — and the form could not ask for it either, because a register input
 * is not a question.
 */

const root = path.resolve(import.meta.dirname, '../../..');
const bindings = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/bindings.yaml'), 'utf8')) as BindingsDocument;

describe('select_on keys', () => {
  /** [lawId, inputName, key] for every selector that names another input. */
  const selectors = Object.entries(bindings).flatMap(([lawId, inputs]) =>
    inputs && typeof inputs === 'object'
      ? Object.entries(inputs).flatMap(([name, b]) =>
          (b?.select_on ?? [])
            .map((c) => c?.value)
            .filter((v) => typeof v === 'string' && v.startsWith('$'))
            .map((v) => [lawId, name, v.slice(1).split('.')[0]]),
        )
      : [],
  );

  it('exist, so this check is not vacuous', () => {
    expect(selectors.length).toBeGreaterThan(20);
  });

  // A key can also be a parameter of the law, or a cross-law input the engine
  // resolves (`$vestigingsadres` from the KVK law) — `resolveCriterion` has a
  // path for both. What cannot work is a key that is neither: nothing will ever
  // supply it, so the lookup silently yields unknown. `partner_bsn` is exactly
  // that kind of key: an ordinary register value, bound in wet_brp, that other
  // laws have to bind for themselves.
  it('bind partner_bsn in every law that selects on it', () => {
    const unbound = selectors
      .filter(([lawId, , key]) => key === 'partner_bsn' && !Object.hasOwn(bindings[lawId] ?? {}, key))
      .map(([lawId, name]) => `${lawId}.${name} selecteert op $partner_bsn zonder die te binden`);
    expect([...new Set(unbound)]).toEqual([]);
  });
});

describe('a lookup keyed on the partner', () => {
  const onPartner = Object.entries(bindings).flatMap(([lawId, inputs]) =>
    inputs && typeof inputs === 'object'
      ? Object.entries(inputs)
          .filter(([, b]) => (b?.select_on ?? []).some((c) => c?.value === '$partner_bsn'))
          .map(([name, b]): [string, string, Binding] => [lawId, name, b])
      : [],
  );

  it('exists, so this check is not vacuous', () => {
    expect(onPartner.length).toBeGreaterThan(0);
  });

  it('says what an absent partner means, rather than leaving it unknown', () => {
    // `absent: unknown` on a partner-keyed lookup is a contradiction: with no
    // partner the register is not silent, there is simply nobody to look up.
    // Known exception: wet_brp.partner_geboortedatum, where the BRP genuinely
    // cannot answer for a partner it does not hold.
    const unknowns = onPartner
      .filter(([, , b]) => b.absent === 'unknown' || b.absent === undefined)
      .map(([lawId, name, b]) => `${lawId}.${name} (absent: ${b.absent ?? 'niet gezet'})`);
    expect(unknowns).toEqual(['wet_brp.partner_geboortedatum (absent: unknown)']);
  });
});
