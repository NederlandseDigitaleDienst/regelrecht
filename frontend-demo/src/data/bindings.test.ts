/**
 * The bindings sidecar and the laws must agree on absence (RFC-036, schema
 * v0.5.8 `nullable`).
 *
 * `bindings.yaml` says per register input what a missing row means (`absent:`),
 * and the materialiser writes exactly that into the record: `null` for a
 * register that is authoritative for non-existence, `0` for a register that
 * counts nothing, nothing at all (unknown) otherwise. The law says per input
 * whether `null` is a legitimate value (`nullable: true`). The engine rejects a
 * `null` delivered to a non-nullable input at the boundary, so the two files
 * have to coincide:
 *
 *  - `absent: null` needs `nullable: true` on the input;
 *  - a lookup whose `select_on` refers to a nullable parameter or input is
 *    `null` whatever `absent` says (there is nobody to look up), so that
 *    input is nullable too;
 *  - every other scalar table input (`absent: 0`, `absent: unknown`, no
 *    `absent`) never receives `null` from the data layer and must not be
 *    declared nullable, or the declaration claims an absence no register
 *    produces;
 *  - an array input is the list of matching rows, empty when there are none,
 *    and carries neither `absent` nor `nullable`.
 *
 * Claims and the sourceless kinds are unknown until supplied and put no
 * constraint on the declaration. The test reads the real corpus, so a change
 * to either file that breaks the agreement fails `npm test -w frontend-demo`.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';
import { describe, expect, it } from 'vitest';
import type { BindingsDocument, LawDocument } from './loadCorpus';

const here = dirname(fileURLToPath(import.meta.url));
const demoDir = resolve(here, '..', '..', '..', 'corpus', 'demo');

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir).sort()) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (entry.endsWith('.yaml')) out.push(full);
  }
  return out;
}

/** What the test knows about one declared field. */
interface FieldInfo {
  kind: 'parameter' | 'input' | 'output';
  type: string;
  nullable: boolean;
}

/** field key (`<kind>:<name>`) -> field */
type LawFields = Record<string, FieldInfo>;

/** Per law id, per field name: { kind, type, nullable } over every article of every version. */
function loadFields(): Record<string, LawFields> {
  const laws: Record<string, LawFields> = {};
  for (const file of walk(join(demoDir, 'regulation', 'nl'))) {
    const doc = yaml.load(readFileSync(file, 'utf8')) as LawDocument | null | undefined;
    if (!doc?.$id) continue;
    const fields = (laws[doc.$id] ??= {});
    for (const article of doc.articles ?? []) {
      const execution = article.machine_readable?.execution;
      if (!execution) continue;
      for (const [kind, key] of [['parameter', 'parameters'], ['input', 'input'], ['output', 'output']] as const) {
        for (const f of execution[key] ?? []) {
          const prev = fields[`${kind}:${f.name}`];
          fields[`${kind}:${f.name}`] = {
            kind,
            type: f.type ?? 'string',
            // Nullable in any version counts: the materialiser serves them all.
            nullable: Boolean(prev?.nullable) || f.nullable === true,
          };
        }
      }
    }
  }
  return laws;
}

function isNullableVariable(fields: LawFields, ref: unknown): boolean {
  if (typeof ref !== 'string' || !ref.startsWith('$')) return false;
  const head = ref.slice(1).split('.')[0];
  return Boolean(fields[`parameter:${head}`]?.nullable || fields[`input:${head}`]?.nullable);
}

const bindings = yaml.load(readFileSync(join(demoDir, 'bindings.yaml'), 'utf8')) as BindingsDocument;
const laws = loadFields();

describe('bindings.yaml and the demo laws agree on absence', () => {
  it('binds only inputs the law declares', () => {
    const unknown: string[] = [];
    for (const [lawId, lawBindings] of Object.entries(bindings)) {
      if (!laws[lawId]) {
        unknown.push(`${lawId}: no such law`);
        continue;
      }
      for (const name of Object.keys(lawBindings)) {
        if (!laws[lawId][`input:${name}`]) unknown.push(`${lawId}.${name}: bound but not an input of the law`);
      }
    }
    expect(unknown).toEqual([]);
  });

  it('declares nullable exactly the table inputs the data layer can deliver null for', () => {
    const mismatches: string[] = [];
    for (const [lawId, lawBindings] of Object.entries(bindings)) {
      const fields = laws[lawId];
      if (!fields) continue;
      for (const [name, binding] of Object.entries(lawBindings)) {
        const field = fields[`input:${name}`];
        if (!field || binding.kind !== 'table') continue;
        const hasAbsent = Object.hasOwn(binding, 'absent');
        if (field.type === 'array') {
          if (hasAbsent) mismatches.push(`${lawId}.${name}: an array input is the list of rows and carries no absent`);
          if (field.nullable) mismatches.push(`${lawId}.${name}: an array input is never null (empty when no rows), drop nullable`);
          continue;
        }
        const selectorNullable = (binding.select_on ?? []).some((c) => isNullableVariable(fields, c.value));
        const expected = (hasAbsent && binding.absent === null) || selectorNullable;
        if (expected && !field.nullable) {
          const why = hasAbsent && binding.absent === null ? 'absent: null' : 'selects on a nullable variable';
          mismatches.push(`${lawId}.${name}: ${why}, so the law must declare nullable: true`);
        } else if (!expected && field.nullable) {
          const absent = hasAbsent ? String(binding.absent) : 'unknown (no absent key)';
          mismatches.push(`${lawId}.${name}: nullable: true, but the binding never delivers null (absent: ${absent})`);
        }
      }
    }
    expect(mismatches).toEqual([]);
  });
});
