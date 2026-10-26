<script setup lang="ts">
import { computed, nextTick, ref, shallowRef, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import OrgLogo from './OrgLogo.vue';
import { type DeclaredField, fieldSpec, formatValue, humanize, isUnknown } from '../data/format';
import { type EditKind, type TableRow, columnKind as columnKindOf, decimalsFor, editKind, emptyRow, parseCell, parseDutchNumber, stepFor, tableColumns, unitLabel, valueKind } from '../data/editKinds';
import { useI18n } from '../i18n/index.js';
import { type Claimant, type DemoClaim, useDemo } from '../store/demoStore';

// The citizen corrects one register value. The correction becomes a claim on
// the law that owns the input; once approved (by the caseworker, or at once
// for a profile with AUTO_APPROVE_CLAIMS) the engine uses it instead of the
// register value. Opens as a sheet, mirrors `open` onto show()/hide().

/**
 * The value being corrected, as far as this sheet reads it. A `LineageNode`
 * from `data/lineage` fits.
 */
export interface EditValueNode {
  /** The law that owns the input. */
  law: string;
  /** The input name. */
  name: string;
  value: unknown;
  /** The organisation whose register holds the value. */
  service?: string | null;
  keyField?: string;
  keyValue?: string;
  /** The value shown already comes from an approved claim. */
  corrected?: boolean;
}

const props = withDefaults(defineProps<{
  open?: boolean;
  /** Lineage value node: { law, name, value, service, keyField, keyValue } */
  node?: EditValueNode | null;
  /** The tile's law id, so the case for that law knows about this claim. */
  tileLawId?: string | null;
  /** A value only the citizen can know (no register): applies at once. */
  selfDeclared?: boolean;
  /** Who corrects: the citizen from the portal, or the caseworker from a case. */
  claimant?: Claimant;
  /** The person the correction is about; null = the active profile. */
  bsn?: string | null;
  /** The case the caseworker corrects from; null from the portal. */
  caseId?: string | null;
}>(), {
  open: false,
  node: null,
  tileLawId: null,
  selfDeclared: false,
  claimant: 'BURGER',
  bsn: null,
  caseId: null,
});
const emit = defineEmits<{
  close: [];
  submitted: [claim: DemoClaim];
}>();
const { t } = useI18n();
const { corpus, submitClaim, profile, features } = useDemo();

// The hardship clauses the POC offered (web/templates/partials/edit_form.html).
//
// `value` is what lands on the claim and travels to the case file, so it stays
// Dutch in both languages: it names an article of a Dutch act, and a stored
// value that changed with the menu language would make two spellings of the
// same clause. Only the label on screen follows the language.
const HARDSHIP_CLAUSES = [
  { value: '', labelKey: 'sheet.edit.hardship.none' },
  { value: 'Awb Art. 4:84', labelKey: 'sheet.edit.hardship.awb_4_84' },
  { value: 'Participatiewet Art. 18', labelKey: 'sheet.edit.hardship.participatiewet_18' },
  { value: 'Wmo 2015 Art. 2.3.5', labelKey: 'sheet.edit.hardship.wmo_2_3_5' },
  { value: 'Belastingwet Art. 63', labelKey: 'sheet.edit.hardship.belastingwet_63' },
  { value: 'Jeugdwet Art. 2.3', labelKey: 'sheet.edit.hardship.jeugdwet_2_3' },
  { value: 'Anders', labelKey: 'sheet.edit.hardship.other' },
];
/** Above this the file is not kept in localStorage; only its name, type and size are. */
const EVIDENCE_INLINE_LIMIT = 400 * 1024;

/** The chosen document; `dataUrl` stays null for a file too large to keep. */
interface EvidenceDraft {
  name: string;
  type: string;
  size: number;
  dataUrl: string | null;
}

const sheet = ref<NLDDSheet | null>(null);
const newValue = ref('');
/** Working copies for the structured kinds: a table, a list, a single record. */
const rows = ref<TableRow[]>([]);
const list = ref<unknown[]>([]);
const record = ref<Record<string, unknown>>({});
const reason = ref('');
const hardship = ref('');
// Shallow so the identity check in onEvidenceChange sees the object it stored.
const evidence = shallowRef<EvidenceDraft | null>(null);
const error = ref('');
const caseworker = computed(() => props.claimant === 'BEHANDELAAR');

// `fieldSpec` returns the field as the law declares it, so its description is there too.
const spec = computed((): (DeclaredField & { description?: string }) | null => (props.node ? fieldSpec(corpus.value?.lawById(props.node.law)?.doc, props.node.name) : null));
const law = computed(() => (props.node ? corpus.value?.lawById(props.node.law) : null));
// One definition of which editor a value deserves, shared with the tests
// (src/data/editKinds.ts): the declared type decides, the value fills in.
const kind = computed((): EditKind => editKind(props.node?.value, spec.value));
// What the law says about this number: how it is counted, and how precise.
const unit = computed(() => unitLabel(spec.value));
const step = computed(() => stepFor(spec.value));

/** The columns of a table-valued correction, in the order they appear. */
const columns = computed(() => tableColumns(rows.value));
function columnKind(col: string) {
  return columnKindOf(rows.value, col);
}
function setCell(i: number, col: string, raw: unknown, kindOfCol: string) {
  const next = rows.value.map((r) => ({ ...r }));
  // `i` is the index of a rendered row.
  next[i]![col] = parseCell(raw, kindOfCol);
  rows.value = next;
}
function addRow() {
  rows.value = [...rows.value, emptyRow(columns.value)];
}
function removeRow(i: number) {
  rows.value = rows.value.filter((_, j) => j !== i);
}

/** Fields of a single record, in the order they appear. */
const recordKeys = computed(() => Object.keys(record.value ?? {}));
function setField(key: string, raw: unknown, k: string) {
  record.value = { ...record.value, [key]: parseCell(raw, k) };
}
function setListItem(i: number, raw: unknown, k: string) {
  const next = [...list.value];
  next[i] = parseCell(raw, k);
  list.value = next;
}
function addListItem() {
  // A new item copies the kind of the ones already there, empty.
  list.value = [...list.value, list.value.length && typeof list.value[0] === 'number' ? null : ''];
}
function removeListItem(i: number) {
  list.value = list.value.filter((_, j) => j !== i);
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheet.value?.hide?.();
      return;
    }
    error.value = '';
    reason.value = '';
    hardship.value = '';
    evidence.value = null;
    // An unknown value (RFC-036) carries no value to edit: the engine does not
    // have the fact. It arrives as `{__unknown: true, missing: [...]}`, so
    // without this the box would open on the literal text "[object Object]".
    // The field then starts from nothing, like an absent value does, and the
    // citizen fills in what the register lacks. (A number field renders that
    // as 0, which is the design system's own empty state.)
    const raw = props.node?.value;
    const v = isUnknown(raw) ? null : raw;
    if (kind.value === 'amount' && typeof v === 'number') newValue.value = (v / 100).toFixed(2).replace('.', ',');
    else if (kind.value === 'boolean') newValue.value = v ? 'true' : 'false';
    // `editKind` picked these kinds from the value's shape (or the declared type).
    else if (kind.value === 'rows') rows.value = ((v as TableRow[] | null | undefined) ?? []).map((r) => ({ ...r }));
    else if (kind.value === 'list') list.value = [...((v as unknown[] | null | undefined) ?? [])];
    else if (kind.value === 'record') record.value = { ...((v as Record<string, unknown> | null | undefined) ?? {}) };
    else newValue.value = v === null || v === undefined ? '' : String(v);
    await nextTick();
    sheet.value?.show?.();
  },
  { immediate: true },
);

function parse() {
  const raw = String(newValue.value).trim();
  switch (kind.value) {
    case 'amount': {
      const n = parseDutchNumber(raw);
      if (n === null) throw new Error(t('sheet.edit.error.amount'));
      // Round in cents: 0.1 + 0.2 in euros does not land on a whole cent.
      return Math.round(n * 100);
    }
    case 'number': {
      const n = parseDutchNumber(raw);
      if (n === null) throw new Error(t('sheet.edit.error.number'));
      const decimals = decimalsFor(spec.value);
      // The law says how precise this number is; a citizen typing more
      // decimals than it admits would otherwise submit a value the engine
      // silently rounds.
      return decimals === null ? n : Math.round(n * 10 ** decimals) / 10 ** decimals;
    }
    case 'boolean':
      return raw === 'true';
    case 'date':
      if (!/^\d{4}-\d{2}-\d{2}$/.test(raw)) throw new Error(t('sheet.edit.error.date'));
      return raw;
    case 'rows':
      return rows.value;
    case 'list':
      return list.value;
    case 'record':
      return record.value;
    default:
      return raw === '' ? null : raw;
  }
}

// The chosen document. Small files travel along as a data URL so the caseworker
// can open them; larger ones keep only name, type and size (localStorage budget).
function onEvidenceChange(e: Event) {
  const file = (e as CustomEvent<{ files?: File[] } | null | undefined>).detail?.files?.[0] ?? null;
  if (!file) {
    evidence.value = null;
    return;
  }
  const meta: EvidenceDraft = { name: file.name, type: file.type, size: file.size, dataUrl: null };
  evidence.value = meta;
  if (file.size > EVIDENCE_INLINE_LIMIT) return;
  const reader = new FileReader();
  reader.onload = () => {
    // readAsDataURL yields a string.
    if (evidence.value === meta) evidence.value = { ...meta, dataUrl: reader.result as string };
  };
  reader.readAsDataURL(file);
}

/** The value of a native `<select>` after a change. */
function selectValue(e: Event): string {
  return (e.target as HTMLSelectElement).value;
}
/** What an nldd field's event carries: the value in `detail`, or on the field itself. */
function inputValue(e: Event): string {
  return (e as CustomEvent<{ value?: string } | null | undefined>).detail?.value ?? (e.target as EventTarget & { value: string }).value;
}
/** The value in an nldd field's `detail`, if any. */
function detailValue(e: Event): unknown {
  return (e as CustomEvent<{ value?: unknown } | null | undefined>).detail?.value;
}

function submit() {
  if (!props.node) return;
  let value;
  try {
    value = parse();
  } catch (e) {
    // parse() only throws the Errors it creates.
    error.value = (e as Error).message;
    return;
  }
  if (!reason.value.trim() && !props.selfDeclared) {
    error.value = caseworker.value ? t('sheet.edit.error.reason.officer') : t('sheet.edit.error.reason.citizen');
    return;
  }
  const claim = submitClaim({
    lawId: props.node.law,
    tileLawId: props.tileLawId,
    input: props.node.name,
    keyField: props.node.keyField ?? 'bsn',
    keyValue: props.node.keyValue ?? props.bsn ?? profile.value?.bsn,
    // Alleen de waarde meegeven als ze van het register komt. Bij een gegeven
    // dat al gecorrigeerd is, toont de rij de gecorrigeerde waarde, en die als
    // "oud" vastleggen zou de correctie ervóór wegpoetsen. `null` laat de store
    // opzoeken wat er zonder correcties staat.
    oldValue: props.node.corrected ? null : props.node.value,
    newValue: value,
    reason: reason.value.trim(),
    evidence: evidence.value,
    hardship: hardship.value ? { clause: hardship.value } : null,
    selfDeclared: props.selfDeclared,
    claimant: props.claimant,
    ...(props.bsn ? { bsn: props.bsn } : {}),
    caseId: props.caseId,
    // The caseworker is the one who would approve it; the correction applies at once.
    ...(caseworker.value ? { approve: true } : {}),
  });
  emit('submitted', claim);
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" width="480px" :accessible-label="t('sheet.edit.label')" @close="emit('close')">
      <nldd-page v-if="node">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="humanize(node.name)" :supporting-text="law?.name ?? node.law" :dismiss-text="t('sheet.dismiss')" @dismiss="emit('close')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-list appearance="box-tinted" :accessible-label="t('sheet.edit.current')">
            <nldd-list-item size="md">
              <nldd-cell v-if="node.service"><OrgLogo :service="node.service" /></nldd-cell>
              <nldd-spacer-cell v-if="node.service" size="12"></nldd-spacer-cell>
              <nldd-text-cell
                :overline="selfDeclared ? t('sheet.edit.overline.self') : t('sheet.edit.overline.registered')"
                :text="formatValue(node.value, spec)"
                :supporting-text="node.service ? t('sheet.edit.source', { service: corpus?.services[node.service]?.name ?? node.service }) : t('sheet.edit.no_register')"
              ></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-rich-text v-if="spec?.description"><p>{{ spec.description }}</p></nldd-rich-text>
          <nldd-form-field :label="unit ? t('sheet.edit.new_value.unit', { unit }) : t('sheet.edit.new_value')">
            <nldd-dropdown v-if="kind === 'boolean'">
              <select :value="newValue" @change="newValue = selectValue($event)">
                <option value="true">{{ t('sheet.edit.yes') }}</option>
                <option value="false">{{ t('sheet.edit.no') }}</option>
              </select>
            </nldd-dropdown>
            <!-- A table-valued register (children, housemates): rows with the
                 same columns, each cell typed on what the column holds, and
                 rows can be added or removed, as the POC allowed. -->
            <template v-else-if="kind === 'rows'">
              <nldd-table :columns="`repeat(${columns.length}, minmax(120px, 1fr)) max-content`">
                <nldd-table-row slot="header">
                  <nldd-text-cell v-for="c in columns" :key="c" size="sm" :text="humanize(c)"></nldd-text-cell>
                  <nldd-text-cell size="sm" text=""></nldd-text-cell>
                </nldd-table-row>
                <nldd-table-row v-for="(row, i) in rows" :key="i">
                  <nldd-cell v-for="c in columns" :key="c">
                    <nldd-dropdown v-if="columnKind(c) === 'boolean'">
                      <select :value="String(row[c])" @change="setCell(i, c, selectValue($event), 'boolean')">
                        <option value="true">{{ t('sheet.edit.yes') }}</option>
                        <option value="false">{{ t('sheet.edit.no') }}</option>
                      </select>
                    </nldd-dropdown>
                    <nldd-date-field v-else-if="columnKind(c) === 'date'" :value="row[c] ?? ''" width="full" @change="setCell(i, c, detailValue($event) ?? '', 'date')"></nldd-date-field>
                    <nldd-text-field v-else :value="row[c] ?? ''" width="full" :keyboard="columnKind(c) === 'number' ? 'numeric' : undefined" @input="setCell(i, c, inputValue($event), columnKind(c))"></nldd-text-field>
                  </nldd-cell>
                  <nldd-cell>
                    <nldd-icon-button size="xs" appearance="neutral-transparent" icon="trash" :accessible-label="t('sheet.edit.rows.remove')" @click="removeRow(i)"></nldd-icon-button>
                  </nldd-cell>
                </nldd-table-row>
                <nldd-inline-dialog slot="empty" :text="t('sheet.edit.rows.empty')"></nldd-inline-dialog>
              </nldd-table>
              <nldd-button size="sm" appearance="secondary" start-icon="plus" :text="t('sheet.edit.rows.add')" @click="addRow"></nldd-button>
            </template>
            <!-- A list of plain values (ages, properties): one typed field per
                 item, with the same add and remove as the table. -->
            <template v-else-if="kind === 'list'">
              <nldd-list appearance="simple" :accessible-label="t('sheet.edit.list.label')">
                <nldd-list-item v-for="(item, i) in list" :key="i" size="sm">
                  <nldd-cell width="full">
                    <nldd-dropdown v-if="valueKind(item) === 'boolean'">
                      <select :value="String(item)" @change="setListItem(i, selectValue($event), 'boolean')">
                        <option value="true">{{ t('sheet.edit.yes') }}</option>
                        <option value="false">{{ t('sheet.edit.no') }}</option>
                      </select>
                    </nldd-dropdown>
                    <nldd-date-field v-else-if="valueKind(item) === 'date'" :value="item ?? ''" width="full" @change="setListItem(i, detailValue($event) ?? '', 'date')"></nldd-date-field>
                    <nldd-text-field v-else :value="item ?? ''" width="full" :keyboard="valueKind(item) === 'number' ? 'numeric' : undefined" @input="setListItem(i, inputValue($event), valueKind(item))"></nldd-text-field>
                  </nldd-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell><nldd-icon-button size="xs" appearance="neutral-transparent" icon="trash" :accessible-label="t('sheet.edit.list.remove')" @click="removeListItem(i)"></nldd-icon-button></nldd-cell>
                </nldd-list-item>
                <nldd-inline-dialog slot="empty" :text="t('sheet.edit.list.empty')"></nldd-inline-dialog>
              </nldd-list>
              <nldd-button size="sm" appearance="secondary" start-icon="plus" :text="t('sheet.edit.list.add')" @click="addListItem"></nldd-button>
            </template>
            <!-- One record (an address, a decision): a typed field per named field. -->
            <template v-else-if="kind === 'record'">
              <nldd-list appearance="simple" :accessible-label="t('sheet.edit.record.label')">
                <nldd-list-item v-for="key in recordKeys" :key="key" size="sm">
                  <nldd-text-cell size="sm" min-width="120px" :text="humanize(key)"></nldd-text-cell>
                  <nldd-cell width="full">
                    <nldd-dropdown v-if="valueKind(record[key]) === 'boolean'">
                      <select :value="String(record[key])" @change="setField(key, selectValue($event), 'boolean')">
                        <option value="true">{{ t('sheet.edit.yes') }}</option>
                        <option value="false">{{ t('sheet.edit.no') }}</option>
                      </select>
                    </nldd-dropdown>
                    <nldd-date-field v-else-if="valueKind(record[key]) === 'date'" :value="record[key] ?? ''" width="full" @change="setField(key, detailValue($event) ?? '', 'date')"></nldd-date-field>
                    <nldd-text-field v-else :value="record[key] ?? ''" width="full" :keyboard="valueKind(record[key]) === 'number' ? 'numeric' : undefined" @input="setField(key, inputValue($event), valueKind(record[key]))"></nldd-text-field>
                  </nldd-cell>
                </nldd-list-item>
              </nldd-list>
            </template>
            <!-- Typed input, as the POC had it: a date gets a date picker and a
                 number a number field, so the value a citizen enters is of the
                 kind the law expects. An amount stays a text field: the number
                 field has no empty state and shows the Dutch comma badly. -->
            <nldd-date-field v-else-if="kind === 'date'" :value="newValue" width="full" @change="newValue = (detailValue($event) as string | undefined) ?? newValue"></nldd-date-field>
            <nldd-number-field v-else-if="kind === 'number'" :value="newValue" width="full" hide-spin-buttons :step="step" @input="newValue = String(detailValue($event) ?? newValue)" @change="newValue = String(detailValue($event) ?? newValue)"></nldd-number-field>
            <nldd-text-field v-else :value="newValue" @input="newValue = inputValue($event)"></nldd-text-field>
          </nldd-form-field>
          <nldd-form-field :label="selfDeclared ? t('sheet.edit.reason.self') : t('sheet.edit.reason.correction')" :optional="selfDeclared || undefined">
            <nldd-multi-line-text-field :value="reason" rows="3" :placeholder="caseworker ? t('sheet.edit.reason.placeholder.officer') : t('sheet.edit.reason.placeholder.citizen')" @input="reason = inputValue($event)"></nldd-multi-line-text-field>
            <nldd-form-field-help-text>{{ caseworker ? t('sheet.edit.reason.help.officer') : hardship ? t('sheet.edit.reason.help.hardship') : selfDeclared || features.AUTO_APPROVE_CLAIMS ? t('sheet.edit.reason.help.immediate') : t('sheet.edit.reason.help.pending') }}</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-form-field :label="t('sheet.edit.hardship.label')" optional>
            <nldd-dropdown width="full">
              <select :value="hardship" @change="hardship = selectValue($event)">
                <option v-for="o in HARDSHIP_CLAUSES" :key="o.value" :value="o.value">{{ t(o.labelKey) }}</option>
              </select>
            </nldd-dropdown>
            <nldd-form-field-help-text>{{ t('sheet.edit.hardship.help') }}</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-form-field :label="t('sheet.edit.evidence.label')" optional>
            <nldd-file-field accept=".pdf,image/*" :accessible-label="t('sheet.edit.evidence.choose')" @change="onEvidenceChange"></nldd-file-field>
            <nldd-form-field-help-text>{{ evidence && !evidence.dataUrl && evidence.size > EVIDENCE_INLINE_LIMIT ? t('sheet.edit.evidence.too_large') : t('sheet.edit.evidence.help') }}</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-banner v-if="error" variant="critical" :text="error"></nldd-banner>
          <nldd-form-actions>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" :text="caseworker ? t('sheet.edit.submit.officer') : selfDeclared ? t('sheet.edit.submit.self') : t('sheet.edit.submit.citizen')" @click="submit"></nldd-button>
              <nldd-button appearance="secondary" :text="t('sheet.cancel')" @click="emit('close')"></nldd-button>
            </nldd-button-group>
          </nldd-form-actions>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
