import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import type { Binding, BindingsDocument } from './loadCorpus';

/**
 * Every `kind: table` binding has to name a table and a column that the persona
 * data actually has. Nothing checked that before, and a typo is invisible:
 * `materialize` finds no row, the input resolves to unknown, and the law
 * reports a missing fact for a register that was never queried. The citizen
 * then sees "De wet kan nog geen uitkomst geven" for a value nobody can supply
 * — the form does not ask for it either, because a register input is not a
 * question.
 *
 * That is how `partner_buitenlands_inkomen` pointed at table `buitenlands`
 * (the table is `buitenlands_inkomen`) with column `inkomen` (the column is
 * `bedrag`): a citizen without a partner was told the huurtoeslag could not be
 * decided because his partner's foreign income was unknown.
 */

const root = path.resolve(import.meta.dirname, '../../..');
const bindings = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/bindings.yaml'), 'utf8')) as BindingsDocument;
const profiles = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/profiles.yaml'), 'utf8'));

/** Table name -> the set of columns it carries, as the persona data has them. */
function tablesIn(data: unknown, out = new Map<string, Set<string>>()): Map<string, Set<string>> {
  if (!data || typeof data !== 'object') return out;
  for (const [key, value] of Object.entries(data)) {
    const isRowList = Array.isArray(value) && value.length && value.every((r) => r && typeof r === 'object' && !Array.isArray(r));
    if (isRowList) {
      const columns = out.get(key) ?? new Set<string>();
      for (const row of value) for (const column of Object.keys(row)) columns.add(column);
      out.set(key, columns);
    } else {
      tablesIn(value, out);
    }
  }
  return out;
}

describe('table bindings', () => {
  const tables = tablesIn(profiles);

  /** [lawId, inputName, binding] for every binding that reads a register table. */
  const tableBindings = Object.entries(bindings).flatMap(([lawId, inputs]) =>
    inputs && typeof inputs === 'object'
      ? Object.entries(inputs)
          .filter(([, b]) => b?.kind === 'table' && b.table)
          // The filter above leaves only bindings with a table.
          .map(([name, b]): [string, string, Binding & { table: string }] => [lawId, name, b as Binding & { table: string }])
      : [],
  );

  // A table no persona carries is legitimate: not everyone has a pension file.
  // A table that exists under a *near* name is a typo, and that is what this
  // catches — the binding misses every row and the input silently goes unknown.
  it('does not read a table that exists under a different name', () => {
    const known = [...tables.keys()];
    const typos = tableBindings
      .filter(([, , b]) => !tables.has(b.table))
      .map(([lawId, name, b]) => {
        const near = known.filter((t) => t.startsWith(b.table) || b.table.startsWith(t));
        return near.length ? `${lawId}.${name} → ${b.table} (bedoeld: ${near.join(', ')})` : null;
      })
      .filter(Boolean);
    expect(typos).toEqual([]);
  });

  // A column no row carries is not by itself a defect: the personas are
  // deliberately incomplete, and a register that has no answer for someone is
  // exactly what `absent:` is for. Only a column that no row anywhere has,
  // while the binding filters rows *on* it, cannot work at all — a select_on
  // column that is nowhere means the lookup can never match.
  it('never selects rows on a column that exists in no row of that table', () => {
    const impossible = tableBindings
      .filter(([, , b]) => tables.has(b.table))
      .flatMap(([lawId, name, b]) =>
        (b.select_on ?? [])
          // Filtered on `tables.has(b.table)` above.
          .filter((c) => c?.name && !tables.get(b.table)!.has(c.name))
          .map((c) => `${lawId}.${name} → ${b.table}.${c.name}`),
      );
    // Known and not fixed here: the Awb bestuursorgaan law and one APV input
    // filter `organisaties`/`vergunningen_historie` on a key those tables do
    // not carry (`organisatie_id` where the rows have `kvk_nummer`). Same class
    // of defect as the buitenlands_inkomen one, but it belongs to a different
    // question — whether those laws should read a persona table at all — so it
    // is recorded here rather than silently repaired.
    const KNOWN = impossible.filter((x) => /organisaties\.organisatie_id|vergunningen_historie\.kvk_nummer/.test(x));
    expect(impossible.filter((x) => !KNOWN.includes(x))).toEqual([]);
    expect(KNOWN.length).toBeGreaterThan(0);
  });

  it('covers the bindings it means to cover', () => {
    // A guard against the check quietly matching nothing.
    expect(tableBindings.length).toBeGreaterThan(50);
    expect(tables.size).toBeGreaterThan(10);
  });
});
