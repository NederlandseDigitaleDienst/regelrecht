/**
 * The edit form for one entry of `machine_readable.definitions`, and the way
 * back to YAML.
 *
 * A definition is a bare value (`naam: 123`) or the structured form
 * `{ value, type, type_spec }` that RFC-023 introduced so a constant can say
 * what kind of quantity it is. The form edits both without changing a shape on
 * its own: a bare value stays bare until the user picks a type, and keys the
 * form does not edit (`description`, `type_spec.precision`, ...) are kept.
 *
 * A unit is a label and never a conversion (RFC-023): the form shows a
 * `percentage` as the number it is and a `ratio` as the 0-1 number it is. The
 * one conversion is money, which the corpus stores in eurocent and the user
 * types in euros.
 */
import { centsToEuros, eurosToCents } from '../utils/currency';
import type { RawDefinition } from './lawFormat';

/** The types the form edits. The schema allows `object` and `decision` too; a
 *  constant of those is kept but not edited here. */
export type DefinitionType = 'number' | 'amount' | 'string' | 'boolean' | 'date' | 'array';

/** The units a `number` can carry. Money is `amount` with unit `eurocent`;
 *  `euro` is left out on purpose, because the corpus counts money in eurocent
 *  and a law in euro next to one in eurocent binds without anyone noticing. */
export type NumberUnit = 'percentage' | 'ratio' | 'years' | 'months' | 'weeks' | 'days';

/** What the value field shows, decided by the type or, without one, by the value. */
export type DefinitionKind = DefinitionType;

export const DEFINITION_TYPES: { value: DefinitionType | ''; label: string }[] = [
  { value: '', label: 'Geen type' },
  { value: 'number', label: 'Getal' },
  { value: 'amount', label: 'Bedrag' },
  { value: 'string', label: 'Tekst' },
  { value: 'boolean', label: 'Ja/nee' },
  { value: 'date', label: 'Datum' },
  { value: 'array', label: 'Lijst' },
];

export const NUMBER_UNITS: { value: NumberUnit | ''; label: string; suffix: string }[] = [
  { value: '', label: 'Geen eenheid', suffix: '' },
  { value: 'percentage', label: 'Percentage (0-100)', suffix: '%' },
  { value: 'ratio', label: 'Ratio (0-1)', suffix: '' },
  { value: 'years', label: 'Jaren', suffix: 'jaar' },
  { value: 'months', label: 'Maanden', suffix: 'maanden' },
  { value: 'weeks', label: 'Weken', suffix: 'weken' },
  { value: 'days', label: 'Dagen', suffix: 'dagen' },
];

const TYPES = new Set<string>(DEFINITION_TYPES.map((t) => t.value).filter(Boolean));
const UNITS = new Set<string>(NUMBER_UNITS.map((u) => u.value).filter(Boolean));

export interface DefinitionForm {
  /** The `type` it gets; '' writes none. */
  type: DefinitionType | '';
  /** The unit of a `number`; '' writes none. */
  unit: NumberUnit | '';
  /** What the value field shows: the type, or without one the kind of value
   *  the definition holds. */
  valueKind: DefinitionKind;
  /** A number, in euros for an amount. */
  number: number;
  text: string;
  bool: boolean;
  /** ISO, yyyy-mm-dd. */
  date: string;
  items: string[];
  /** Whether every item of the list was a number, so they go back as numbers. */
  numericItems: boolean;
  /** A `type` (or amount unit) the form does not edit, such as `object`. The
   *  definition is kept as it is; only its name can change. */
  unsupported: string | null;
}

function asRecord(raw: unknown): Record<string, unknown> | null {
  return raw !== null && typeof raw === 'object' && !Array.isArray(raw) ? (raw as Record<string, unknown>) : null;
}

function unitOf(obj: Record<string, unknown> | null): string | undefined {
  const spec = asRecord(obj?.type_spec);
  return typeof spec?.unit === 'string' ? spec.unit : undefined;
}

/** What the value field shows for a value without a type. */
function kindOfValue(value: unknown): DefinitionKind {
  if (typeof value === 'boolean') return 'boolean';
  if (typeof value === 'number') return 'number';
  if (Array.isArray(value)) return 'array';
  return 'string';
}

/** What the value field shows. */
export function kindOf(form: DefinitionForm): DefinitionKind {
  return form.type || form.valueKind;
}

/** A blank form for a new definition: a number, structured from the start. */
export function newDefinitionForm(): DefinitionForm {
  return {
    type: 'number',
    unit: '',
    valueKind: 'number',
    number: 0,
    text: '',
    bool: false,
    date: '',
    items: [],
    numericItems: false,
    unsupported: null,
  };
}

/** The JS kind a declared type's value must have. */
const VALUE_CHECK: Record<DefinitionType, (v: unknown) => boolean> = {
  number: (v) => typeof v === 'number',
  amount: (v) => typeof v === 'number',
  string: (v) => typeof v === 'string',
  boolean: (v) => typeof v === 'boolean',
  date: (v) => typeof v === 'string',
  array: Array.isArray,
};

/** A list the token field can hold and give back unchanged: all texts or all numbers. */
function isPlainList(list: unknown[]): boolean {
  return list.every((v) => typeof v === 'string') || list.every((v) => typeof v === 'number');
}

/**
 * Why the form cannot edit this definition without changing it, or null when
 * it can. Anything it would have to guess at or coerce is kept as it is and
 * left to the YAML: a save that changes a value nobody touched is the one
 * thing this form must never do.
 */
function unsupportedReason(raw: RawDefinition | null | undefined): string | null {
  const obj = asRecord(raw);
  if (obj && !('value' in obj)) return 'samengestelde definitie zonder value';
  const value = obj ? obj.value : raw;
  if (value === null || value === undefined) return 'lege waarde';
  const declared = typeof obj?.type === 'string' ? obj.type : '';
  const unit = unitOf(obj);
  if (declared && !TYPES.has(declared)) return `type ${declared}`;
  if (declared === 'amount' && unit === undefined) return 'bedrag zonder eenheid';
  if (declared === 'amount' && unit !== 'eurocent') return `bedrag in ${unit}`;
  if (declared === 'number' && unit !== undefined && !UNITS.has(unit)) return `getal in ${unit}`;
  if (declared && !VALUE_CHECK[declared as DefinitionType](value)) {
    return `waarde die niet past bij type ${declared}`;
  }
  if (!declared && asRecord(value)) return 'samengestelde waarde';
  if (Array.isArray(value) && !isPlainList(value)) return 'lijst met gemengde waarden';
  return null;
}

/** The form for an existing definition. */
export function toForm(raw: RawDefinition | null | undefined): DefinitionForm {
  const obj = asRecord(raw);
  const value = obj ? obj.value : raw;
  const declared = typeof obj?.type === 'string' ? obj.type : '';
  const unit = unitOf(obj);
  const form: DefinitionForm = {
    ...newDefinitionForm(),
    type: '',
    valueKind: kindOfValue(value),
    unsupported: unsupportedReason(raw),
  };
  if (form.unsupported) return form;

  if (TYPES.has(declared)) form.type = declared as DefinitionType;
  if (declared === 'number' && unit && UNITS.has(unit)) form.unit = unit as NumberUnit;
  form.valueKind = kindOf(form);

  const kind = form.valueKind;
  if (kind === 'amount') {
    const euros = centsToEuros(value);
    form.number = euros === '' ? 0 : euros;
  } else if (kind === 'number') {
    form.number = value as number;
  } else if (kind === 'boolean') {
    form.bool = value as boolean;
  } else if (kind === 'array') {
    const list = value as unknown[];
    form.items = list.map((v) => String(v));
    form.numericItems = list.length > 0 && list.every((v) => typeof v === 'number');
  } else if (kind === 'date') {
    form.date = value as string;
  } else {
    form.text = String(value);
  }
  return form;
}

/**
 * The user picks another type. The stored number never changes by that:
 * between Getal and Bedrag only the display differs (an amount shows its
 * eurocent as euros), so 3971900 stays 3971900 whichever way it is labelled.
 * Across other kinds the value is carried over as text where it can be, so
 * switching a number to text keeps "13.7" rather than starting from nothing.
 */
export function changeType(form: DefinitionForm, next: DefinitionType | ''): void {
  const from = kindOf(form);
  // Without a type the field follows what is stored: an amount is a number.
  const to: DefinitionKind = next || (from === 'amount' ? 'number' : from);
  form.type = next;
  if (next !== 'number') form.unit = '';
  if (from === to) return;
  form.valueKind = to;
  if (from === 'number' && to === 'amount') {
    const euros = centsToEuros(form.number);
    form.number = euros === '' ? 0 : euros;
    return;
  }
  if (from === 'amount' && to === 'number') {
    const cents = eurosToCents(form.number);
    form.number = cents === '' ? 0 : cents;
    return;
  }
  const asText =
    from === 'number' || from === 'amount'
      ? String(from === 'amount' ? eurosToCents(form.number) : form.number)
      : from === 'boolean'
        ? String(form.bool)
        : from === 'date'
          ? form.date
          : from === 'array'
            ? form.items.join(', ')
            : form.text;
  if (to === 'number' || to === 'amount') {
    const n = Number(asText.replace(',', '.'));
    const stored = Number.isFinite(n) ? n : 0;
    form.number = to === 'amount' ? (centsToEuros(stored) as number) : stored;
  } else if (to === 'boolean') {
    form.bool = asText === 'true';
  } else if (to === 'date') {
    form.date = /^\d{4}-\d{2}-\d{2}$/.test(asText) ? asText : '';
  } else if (to === 'array') {
    form.items = asText ? [asText] : [];
    form.numericItems = false;
  } else {
    form.text = asText;
  }
}

/** The value the form holds, in the shape the YAML stores. */
function valueOf(form: DefinitionForm): unknown {
  const kind = kindOf(form);
  if (kind === 'amount') return eurosToCents(form.number) === '' ? 0 : eurosToCents(form.number);
  if (kind === 'number') return form.number;
  if (kind === 'boolean') return form.bool;
  if (kind === 'date') return form.date;
  if (kind === 'array') {
    if (!form.numericItems) return [...form.items];
    // Items that were numbers go back as numbers, as long as they still are.
    return form.items.map((item) => {
      const n = Number(item);
      return item.trim() !== '' && Number.isFinite(n) ? n : item;
    });
  }
  return form.text;
}

/**
 * The definition to write: bare when it was bare and still has no type,
 * otherwise the structured form, keeping every key the form does not edit.
 */
export function fromForm(
  form: DefinitionForm,
  raw: RawDefinition | null | undefined,
): unknown {
  if (form.unsupported) return raw;
  const obj = asRecord(raw);
  const value = valueOf(form);
  const wasBare = raw != null && obj === null;
  if (wasBare && !form.type) return value;

  const out: Record<string, unknown> = { ...(obj ?? {}), value };
  const hadType = obj?.type !== undefined;
  if (form.type) out.type = form.type;
  else delete out.type;

  const spec: Record<string, unknown> = { ...(asRecord(out.type_spec) ?? {}) };
  if (form.type === 'amount') spec.unit = 'eurocent';
  else if (form.type === 'number' && form.unit) spec.unit = form.unit;
  else if (form.type) delete spec.unit;
  // The user cleared the type: the unit that went with it goes too. A
  // definition that never had a type keeps its unit, which the form never
  // showed and so did not touch.
  else if (hadType) delete spec.unit;
  if (Object.keys(spec).length) out.type_spec = spec;
  else delete out.type_spec;
  return out;
}

/** The text after the value field: what the number means. */
export function valueSuffix(form: DefinitionForm): string {
  if (kindOf(form) !== 'number' || form.type !== 'number') return '';
  if (form.unit === 'ratio') {
    // The ratio stays the number it is; this only says what it amounts to,
    // so 0.137 against 0.0137 is visible before it is saved.
    const pct = form.number * 100;
    return `= ${pct.toLocaleString('nl-NL', { maximumFractionDigits: 6 })}%`;
  }
  return NUMBER_UNITS.find((u) => u.value === form.unit)?.suffix ?? '';
}

/**
 * One definition for reading, as the machine pane lists it: money in euros, a
 * percentage with its sign, a ratio with the percentage it amounts to, a
 * duration with its unit. Without a unit the value is shown as it is; the old
 * guess that read every number between 0 and 1 as a percentage is gone,
 * because nothing in the YAML says so.
 */
export function formatDefinitionValue(value: unknown, unit: string | undefined): string {
  if (typeof value === 'number') {
    const nl = (n: number) => n.toLocaleString('nl-NL', { maximumFractionDigits: 6 });
    if (unit === 'eurocent') return (value / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR' });
    if (unit === 'percentage') return `${nl(value)}%`;
    if (unit === 'ratio') return `${nl(value)} (${nl(value * 100)}%)`;
    const suffix = NUMBER_UNITS.find((u) => u.value === unit)?.suffix;
    if (suffix) return `${nl(value)} ${suffix}`;
  }
  // A list joins with comma + space so it can wrap.
  if (Array.isArray(value)) return value.join(', ');
  if (value == null) return '';
  return String(value);
}
