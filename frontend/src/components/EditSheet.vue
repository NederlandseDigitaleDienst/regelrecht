<script setup lang="ts">
import type { ExecutionField, InputSource, RawDefinition } from '../lib/lawFormat';
import { ref, computed, watch, watchEffect, nextTick } from 'vue';
import { collectAvailableVariables } from '../utils/operationTree';
import { lawsListUrl } from '../composables/corpusUrls';
import { apiFetchJson } from '../lib/apiFetch';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { NLDDComboBox } from '@nldd/design-system/combo-box';
import type { OperationSettingsArticle } from './OperationSettings.vue';
import {
  DEFINITION_TYPES,
  NUMBER_UNITS,
  changeType,
  fromForm,
  kindOf,
  newDefinitionForm,
  toForm,
  valueSuffix,
  type DefinitionForm,
  type DefinitionType,
  type NumberUnit,
} from '../lib/definitionForm';

/** The entry the sheet edits, as the editor view hands it in. */
export interface EditSheetItem {
  section: string;
  key?: string;
  index?: number;
  isNew?: boolean;
  rawDef?: RawDefinition | null;
  data?: ExecutionField | null;
}

/** The saved entry, emitted with `save`. */
export type EditSheetSave =
  | { section: 'definition'; key: string | undefined; newKey: string; data: unknown }
  | { section: 'add-definition'; key: string; data: unknown }
  | { section: 'parameter'; index: number | undefined; data: { name: string; type?: string; required?: boolean } }
  | { section: 'add-parameter'; data: { name: string; type?: string; required?: boolean } }
  | { section: 'input'; index: number | undefined; data: ExecutionField }
  | { section: 'add-input'; data: ExecutionField }
  | { section: 'output'; index: number | undefined; data: ExecutionField }
  | { section: 'add-output'; data: ExecutionField };

/** One editable source.parameters row. */
interface ParamRow {
  _rowId: number;
  key: string;
  value: string;
  _origValue: unknown;
}

/** The form state. Which fields are set depends on the section (see the watch on `item`). */
interface FormValues {
  name?: string;
  /** The definition being edited (see lib/definitionForm). */
  def?: DefinitionForm;
  /** The definition as it was, so keys the form does not edit are kept. */
  rawDef?: RawDefinition | null;
  type?: string;
  required?: boolean;
  sourceRegulation?: string;
  sourceOutput?: string;
  sourceParameters?: ParamRow[];
  sourceParametersOverflow?: Record<string, unknown>;
}

/** A law in the corpus list (GET laws), as far as the sheet reads it. */
interface LawListEntry {
  law_id: string;
  name?: string | null;
  display_name?: string | null;
}

/** An output of a law (GET .../outputs), as far as the sheet reads it. */
interface LawOutput {
  name: string;
  output_type?: string;
  parameters?: { name: string }[] | null;
}

const props = withDefaults(defineProps<{
  item?: EditSheetItem | null;
  article?: OperationSettingsArticle | null;
  /** Active traject ref. When set the law search dropdown is scoped to
   *  the traject's corpus so newly-added laws in this traject can be
   *  selected as a source. */
  trajectRef?: string | null;
}>(), {
  item: null,
  article: null,
  trajectRef: null,
});

const emit = defineEmits<{
  save: [payload: EditSheetSave];
  close: [];
}>();

const sheetEl = ref<NLDDSheet | null>(null);
const lawComboBoxEl = ref<NLDDComboBox | null>(null);
const outputComboBoxEl = ref<NLDDComboBox | null>(null);
const values = ref<FormValues>({});

// Snapshot of the form taken when an item opens. For a NEW item the Save
// button is always shown (you opened the sheet to add it); for an existing
// item it only appears once the user actually changes something.
const baseline = ref('');
const isDirty = computed(() => {
  if (props.item?.isNew) return true;
  try {
    return JSON.stringify(values.value) !== baseline.value;
  } catch {
    return true;
  }
});

const typeOptions = ['string', 'number', 'boolean', 'amount', 'decision'];

// Available variables from the article's machine_readable for parameter
// value dropdowns, grouped by category with alphabetical sorting within
// each group. Reuses the same collectAvailableVariables utility that
// OperationSettings uses for subject dropdowns.
const paramValueGroups = computed(() => {
  const vars = collectAvailableVariables(props.article);
  const groups = new Map<string, { value: string; label: string }[]>();
  for (const v of vars) {
    if (!groups.has(v.category)) groups.set(v.category, []);
    // Set on the line above when missing.
    groups.get(v.category)!.push({
      value: v.ref,
      label: v.name.replace(/_/g, ' '),
    });
  }
  for (const opts of groups.values()) {
    opts.sort((a, b) => a.label.localeCompare(b.label, 'nl'));
  }
  return groups;
});

// --- Law search / output selection ---
// Cache scoped per (traject, instance). Resetting on a traject change
// keeps cross-traject content from leaking when the same sheet is
// opened again under a different traject.
let lawsCache: LawListEntry[] | null = null;
let lawsCacheTrajectRef: string | null = null;
async function fetchLawsList(): Promise<LawListEntry[]> {
  if (lawsCacheTrajectRef !== props.trajectRef) {
    lawsCache = null;
    lawsCacheTrajectRef = props.trajectRef;
  }
  if (lawsCache) return lawsCache;
  try {
    lawsCache = await apiFetchJson<LawListEntry[]>(lawsListUrl(props.trajectRef, 'limit=1000'));
  } catch {
    // HTTP or network failure - the law combobox just offers no
    // suggestions; nothing is cached so the next open retries.
    return [];
  }
  return lawsCache;
}

const allLaws = ref<LawListEntry[]>([]);
const availableOutputs = ref<LawOutput[]>([]);
const outputsLoading = ref(false);

function displayName(law: LawListEntry) {
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

function outputDisplayName(out: LawOutput) {
  return `${out.name} (${out.output_type})`;
}

async function fetchOutputsForLaw(lawId: string | undefined) {
  if (!lawId) {
    availableOutputs.value = [];
    return;
  }
  outputsLoading.value = true;
  try {
    availableOutputs.value = await apiFetchJson<LawOutput[]>(
      `/api/corpus/laws/${encodeURIComponent(lawId)}/outputs`,
    );
  } catch {
    // HTTP or network failure - no output suggestions for this law.
    availableOutputs.value = [];
  } finally {
    outputsLoading.value = false;
  }
}

// The combo-box derives its shown `text` from the matching menu item only when
// `value` changes, but the menu items arrive later, from the fetch. Set `text`
// whenever the form's sourceRegulation changes OR allLaws finishes loading.
// Without this, an existing input opened in the sheet would show an empty
// input until the user typed.
watchEffect(() => {
  const lawId = values.value?.sourceRegulation;
  if (!lawComboBoxEl.value) return;
  const match = allLaws.value.find(l => l.law_id === lawId);
  lawComboBoxEl.value.text = match ? displayName(match) : (lawId || '');
});

watchEffect(() => {
  const outName = values.value?.sourceOutput;
  if (!outputComboBoxEl.value) return;
  const match = availableOutputs.value.find(o => o.name === outName);
  outputComboBoxEl.value.text = match ? outputDisplayName(match) : (outName || '');
});

async function onLawComboChange(event: Event) {
  const next = (event as CustomEvent<{ value?: string } | undefined>).detail?.value ?? '';
  if (next === values.value.sourceRegulation) return;
  values.value.sourceRegulation = next;
  values.value.sourceOutput = '';
  await fetchOutputsForLaw(next);
}

function onOutputComboChange(event: Event) {
  const outputName = (event as CustomEvent<{ value?: string } | undefined>).detail?.value ?? '';
  values.value.sourceOutput = outputName;
  if (!outputName) return;
  const match = availableOutputs.value.find(o => o.name === outputName);
  if (match) {
    if (!values.value.name) {
      values.value.name = outputName;
    }
    // Always set type from the source output - it's determined by the external law
    values.value.type = match.output_type || 'string';
    // Always reset and pre-populate source parameters from the selected
    // output's article. Without the unconditional reset, switching to an
    // output with fewer (or no) parameters would leave stale rows that
    // produce invalid source.parameters on save.
    values.value.sourceParameters = (match.parameters ?? []).map(p =>
      makeParamRow(p.name, ''),
    );
  }
}

// Monotonic counter for stable v-for keys on the source.parameters rows.
// We can't use the row index as a key because that would let Vue reuse the
// DOM (and the per-row data-testid attributes) across deletions, which
// confuses focus, the testid contract, and the data-testid-bound e2e
// helpers. Each row gets a fresh _rowId when pushed onto the array.
//
// `_origValue` carries the row's original (un-stringified) value so that
// numeric / boolean parameters round-trip back to their original type on
// save when the user didn't touch the value field. Without it the form
// would silently rewrite `peildatum: 2024` to `peildatum: '2024'`.
let nextRowId = 1;
function makeParamRow(key = '', value = '', origValue: unknown = undefined): ParamRow {
  return { _rowId: nextRowId++, key, value, _origValue: origValue };
}

// Type inference for controls
/** The definition being edited; only set while the sheet edits one. */
const def = computed(() => (values.value as FormValues).def ?? null);
const defKind = computed(() => (def.value ? kindOf(def.value) : null));
const defSuffix = computed(() => (def.value ? valueSuffix(def.value) : ''));
// The suffix sits in its own cell, so it also goes into the field's name: a
// screen reader then hears "Waarde, %" rather than only "Waarde".
const defLabel = computed(() => (defSuffix.value ? `Waarde (${defSuffix.value})` : 'Waarde'));

function onDefTypeChange(event: Event) {
  if (!def.value) return;
  changeType(def.value, (event.target as HTMLSelectElement).value as DefinitionType | '');
}

function onDefUnitChange(event: Event) {
  if (!def.value) return;
  def.value.unit = (event.target as HTMLSelectElement).value as NumberUnit | '';
}

watch(() => props.item, async (item) => {
  if (!item) {
    sheetEl.value?.hide();
    return;
  }

  const s = item.section;
  if (s === 'definition' || s === 'add-definition') {
    // structuredClone throws DataCloneError on Vue reactive proxies; defs are JSON-safe.
    const rawDef: RawDefinition | null =
      !item.isNew && item.rawDef != null ? JSON.parse(JSON.stringify(item.rawDef)) : null;
    values.value = {
      name: item.key ?? '',
      def: rawDef == null ? newDefinitionForm() : toForm(rawDef),
      rawDef,
    };
  } else if (s === 'parameter' || s === 'add-parameter') {
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
      required: item.data?.required ?? false,
    };
  } else if (s === 'input' || s === 'add-input') {
    // Flatten source.parameters into an ordered key/value pair list so the
    // user can edit existing entries and add new ones via the form.
    //
    // Design decisions:
    //   - Each row gets a stable `_rowId` (monotonic counter) so Vue's
    //     v-for keying survives deletions without confusing focus or
    //     data-testid attributes.
    //   - `_origValue` preserves the original (un-stringified) value so
    //     numeric/boolean parameters round-trip correctly when the user
    //     doesn't touch the value field (avoids `peildatum: 2024` becoming
    //     `peildatum: '2024'`). On save, if the stringified form hasn't
    //     changed we emit the original typed value.
    //   - Non-scalar parameter values (objects/arrays) are stashed in
    //     `sourceParametersOverflow` and merged back on save so they
    //     survive untouched - the user can only edit those via the YAML
    //     pane.
    const params = item.data?.source?.parameters;
    const paramList: ParamRow[] = [];
    const overflowParams: Record<string, unknown> = {};
    if (params && typeof params === 'object') {
      for (const [k, v] of Object.entries(params)) {
        if (v == null || typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') {
          paramList.push(makeParamRow(k, v == null ? '' : String(v), v));
        } else {
          overflowParams[k] = v;
          // eslint-disable-next-line no-console
          console.warn(
            `EditSheet: source.parameter '${k}' on input '${item.data?.name}' has a non-scalar value; preserved untouched (edit via the YAML pane).`,
          );
        }
      }
    }
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
      sourceRegulation: item.data?.source?.regulation ?? '',
      sourceOutput: item.data?.source?.output ?? '',
      sourceParameters: paramList,
      sourceParametersOverflow: overflowParams,
    };

    // Load law list for the combo-box menu and pre-populate outputs if
    // editing an existing input. The watchEffect above handles syncing the
    // combo-box's display once allLaws + sourceRegulation are both set.
    availableOutputs.value = [];
    fetchLawsList().then(laws => {
      allLaws.value = laws;
      const reg = item.data?.source?.regulation;
      if (reg) fetchOutputsForLaw(reg);
    });
  } else if (s === 'output' || s === 'add-output') {
    values.value = {
      name: item.data?.name ?? '',
      type: item.data?.type ?? 'string',
    };
  }

  baseline.value = JSON.stringify(values.value);
  await nextTick();
  sheetEl.value?.show();
}, { immediate: true });

function save() {
  const item = props.item;
  if (!item) return;
  const s = item.section;

  if (s === 'definition' || s === 'add-definition') {
    // The watch on `item` filled values for this same section.
    const { name, def: form, rawDef } = values.value as FormValues & { name: string };
    if (!name.trim() || !form) return;
    const data = fromForm(form, rawDef);
    if (s === 'definition') {
      emit('save', { section: 'definition', key: item.key, newKey: name.trim(), data });
    } else {
      emit('save', { section: 'add-definition', key: name.trim(), data });
    }
  } else if (s === 'parameter' || s === 'add-parameter') {
    const { name, type, required } = values.value as FormValues & { name: string };
    if (!name.trim()) return;
    if (s === 'parameter') {
      emit('save', { section: 'parameter', index: item.index, data: { name: name.trim(), type, required } });
    } else {
      emit('save', { section: 'add-parameter', data: { name: name.trim(), type, required } });
    }
  } else if (s === 'input' || s === 'add-input') {
    const { name, type, sourceRegulation, sourceOutput, sourceParameters, sourceParametersOverflow } =
      values.value as FormValues & { name: string };
    if (!name.trim()) return;
    // Keys of the input this form does not edit (`nullable`, `description`,
    // ...) are kept as they were; `source` and `type_spec` are rebuilt below.
    const { name: _n, type: _t, source: _s, type_spec: _ts, ...keptInput }: Partial<ExecutionField> =
      s === 'input' ? item.data || {} : {};
    const data: ExecutionField = { ...keptInput, name: name.trim(), type };
    // Reduce the form's parameter rows back into a plain object. Skip
    // rows with an empty key - they're either still being typed or were
    // added and abandoned. Duplicate keys: last write wins (matches what
    // serializing an object would do anyway).
    //
    // Two correctness rules from iteration 2 review:
    //   1. If the row's value string still equals String(_origValue), the
    //      user didn't touch the value field; emit the original (which
    //      preserves number/boolean types). Otherwise emit the typed
    //      string verbatim.
    //   2. Overflow parameters (object/array values that the form
    //      doesn't render) get merged back BEFORE the user-edited rows
    //      so the user can override them by adding a row with the same
    //      key, but un-overridden overflow keys survive.
    const paramObj: Record<string, unknown> = { ...(sourceParametersOverflow || {}) };
    for (const row of sourceParameters || []) {
      const k = (row.key || '').trim();
      if (!k) continue;
      const origStringForm = row._origValue == null ? '' : String(row._origValue);
      if (row._origValue !== undefined && row.value === origStringForm) {
        paramObj[k] = row._origValue;
      } else {
        paramObj[k] = row.value ?? '';
      }
    }
    // The keys of `source` this form does not edit (`article`,
    // `parameters_from`, `decisions`, ...) are kept as they were.
    const { regulation: _r, output: _o, parameters: _p, ...kept }: InputSource = item.data?.source || {};
    const keptTarget = ['article', 'decisions', 'facts', 'endpoint'].some((k) => k in kept);
    // Emit `source` only when at least regulation OR output is set, or a
    // kept key names what it calls. Parameters alone don't make a valid
    // source block, so if the user has cleared both regulation and output
    // of a regulation reference we drop the entire source - including any
    // overflow params. This matches the user's intent ("disable the source
    // binding") and avoids producing a malformed source: { parameters:
    // {...} } that would fail schema validation.
    if (sourceRegulation || sourceOutput || keptTarget) {
      const source: InputSource = { ...kept };
      if (sourceRegulation) source.regulation = sourceRegulation;
      if (sourceOutput) source.output = sourceOutput;
      if (Object.keys(paramObj).length > 0) source.parameters = paramObj;
      data.source = source;
    }
    if (type === 'amount' && item.data?.type_spec) data.type_spec = item.data.type_spec;
    if (s === 'input') {
      emit('save', { section: 'input', index: item.index, data });
    } else {
      emit('save', { section: 'add-input', data });
    }
  } else if (s === 'output' || s === 'add-output') {
    const { name, type } = values.value as FormValues & { name: string };
    if (!name.trim()) return;
    const data: ExecutionField = { name: name.trim(), type };
    if (type === 'amount' && item.data?.type_spec) data.type_spec = item.data.type_spec;
    if (s === 'output') {
      emit('save', { section: 'output', index: item.index, data });
    } else {
      emit('save', { section: 'add-output', data });
    }
  }

  emit('close');
}

const sectionLabels: Record<string, string> = {
  'definition': 'Definitie',
  'add-definition': 'Definitie toevoegen',
  'parameter': 'Parameter',
  'add-parameter': 'Parameter toevoegen',
  'input': 'Input',
  'add-input': 'Input toevoegen',
  'output': 'Output',
  'add-output': 'Output toevoegen',
};
</script>

<template>
  <nldd-sheet ref="sheetEl" placement="right" width="640px" @close="emit('close')">
    <!-- `:key` forces nldd-page to remount whenever the section changes.
         nldd-page captures the sticky-header height ONCE per mount via
         requestAnimationFrame; if the header text changes after that
         (which happens here because :text is reactive on item.section),
         the measurement stays at the empty/initial value and the body
         content slides up under the title bar. Remounting on section
         change re-runs the measurement with the now-set title text,
         which is the storybook-conventional way to handle a header
         whose content swaps in. -->
    <nldd-page :key="item?.section ?? 'none'" sticky-header>
      <nldd-top-title-bar slot="header" :text="item ? (sectionLabels[item.section] || 'Bewerk') : ''" dismiss-text="Annuleer" @dismiss="emit('close')"></nldd-top-title-bar>

      <nldd-simple-section v-if="item">
          <!-- Definition -->
          <template v-if="(item.section === 'definition' || item.section === 'add-definition') && def">
            <nldd-list appearance="box-tinted" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>

              <!-- A type the form does not edit (object, decision, an amount
                   in euro): the definition is kept as it is. -->
              <nldd-list-item v-if="def.unsupported" size="md" data-testid="def-unsupported">
                <nldd-text-cell text="Waarde" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="`${def.unsupported}: niet te bewerken in dit venster, wel in de YAML`" color="secondary"></nldd-text-cell>
              </nldd-list-item>

              <template v-else>
                <nldd-list-item size="md">
                  <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell>
                    <nldd-dropdown size="md">
                      <select :value="def.type" aria-label="Type" data-testid="def-type" @change="onDefTypeChange">
                        <option v-for="t in DEFINITION_TYPES" :key="t.value" :value="t.value">{{ t.label }}</option>
                      </select>
                    </nldd-dropdown>
                  </nldd-cell>
                </nldd-list-item>

                <nldd-list-item v-if="def.type === 'number'" size="md">
                  <nldd-text-cell text="Eenheid" max-width="140px"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell>
                    <nldd-dropdown size="md">
                      <select :value="def.unit" aria-label="Eenheid" data-testid="def-unit" @change="onDefUnitChange">
                        <option v-for="u in NUMBER_UNITS" :key="u.value" :value="u.value">{{ u.label }}</option>
                      </select>
                    </nldd-dropdown>
                  </nldd-cell>
                </nldd-list-item>
                <!-- Money is always eurocent (the corpus convention); the user
                     types euros. -->
                <nldd-list-item v-else-if="def.type === 'amount'" size="md">
                  <nldd-text-cell text="Eenheid" max-width="140px"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-text-cell text="eurocent (invoer in euro's)" color="secondary"></nldd-text-cell>
                </nldd-list-item>

                <nldd-list-item size="md">
                  <nldd-text-cell text="Waarde" max-width="140px"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <template v-if="defKind === 'amount'">
                    <nldd-text-cell text="€" width="fit-content"></nldd-text-cell>
                    <nldd-spacer-cell size="6"></nldd-spacer-cell>
                  </template>
                  <nldd-cell v-if="defKind === 'boolean'">
                    <nldd-switch-field :checked="def.bool ? true : undefined" @change="def.bool = Boolean($event.detail?.checked)">Waarde</nldd-switch-field>
                  </nldd-cell>
                  <nldd-cell v-else-if="defKind === 'number' || defKind === 'amount'">
                    <!-- number-field with hide-spin-buttons: keeps numeric input
                         validation (rejects non-numeric, handles locale) without
                         the visual clutter that felt out of place for "fixed
                         value from law" semantics. Both @input and @change are
                         intentional: @input gives live-as-you-type updates;
                         @change delivers the value the field normalises on
                         commit/blur. The assignment is idempotent. -->
                    <nldd-number-field
                      :value="def.number"
                      :step="defKind === 'amount' ? '0.01' : '0.000001'"
                      :accessible-label="defLabel"
                      width="full"
                      hide-spin-buttons
                      data-testid="def-number"
                      @input="def.number = Number($event.detail?.value ?? def.number)"
                      @change="def.number = Number($event.detail?.value ?? def.number)"
                    ></nldd-number-field>
                  </nldd-cell>
                  <nldd-cell v-else-if="defKind === 'date'">
                    <nldd-date-field
                      :value="def.date"
                      accessible-label="Waarde"
                      width="full"
                      data-testid="def-date"
                      @input="def.date = $event.detail?.value ?? def.date"
                      @change="def.date = $event.detail?.value ?? def.date"
                    ></nldd-date-field>
                  </nldd-cell>
                  <nldd-cell v-else-if="defKind === 'array'">
                    <nldd-token-field
                      .values="def.items"
                      allow-custom
                      accessible-label="Waarde"
                      placeholder="Toevoegen..."
                      data-testid="def-items"
                      @change="def.items = [...($event.detail?.values ?? def.items)]"
                    ></nldd-token-field>
                  </nldd-cell>
                  <nldd-cell v-else>
                    <nldd-text-field size="md" accessible-label="Waarde" :value="def.text" @input="def.text = $event.target?.value ?? $event.detail?.value ?? def.text"></nldd-text-field>
                  </nldd-cell>
                  <template v-if="defSuffix">
                    <nldd-spacer-cell size="6"></nldd-spacer-cell>
                    <nldd-text-cell :text="defSuffix" width="fit-content" data-testid="def-suffix"></nldd-text-cell>
                  </template>
                </nldd-list-item>
              </template>
            </nldd-list>
          </template>

          <!-- Parameter -->
          <template v-if="item.section === 'parameter' || item.section === 'add-parameter'">
            <nldd-list appearance="box-tinted" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md">
                    <select :value="values.type" @change="values.type = ($event.target as HTMLSelectElement).value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Verplicht" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-switch :checked="values.required ? true : undefined" @change="values.required = Boolean($event.detail?.checked)"></nldd-switch>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Input -->
          <template v-if="item.section === 'input' || item.section === 'add-input'">
            <nldd-list appearance="box-tinted" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item v-if="!values.sourceOutput" size="md">
                <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md">
                    <select :value="values.type" @change="values.type = ($event.target as HTMLSelectElement).value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Bron regelgeving" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-combo-box
                    ref="lawComboBoxEl"
                    size="md"
                    width="100%"
                    placeholder="Zoek regelgeving..."
                    accessible-label="Bron regelgeving"
                    :value="values.sourceRegulation"
                    data-testid="law-combo-box"
                    @change="onLawComboChange"
                  >
                    <nldd-menu>
                      <nldd-menu-item
                        v-for="law in allLaws"
                        :key="law.law_id"
                        :text="displayName(law)"
                        :aliases="law.law_id"
                        :value="law.law_id"
                        :data-testid="`law-result-${law.law_id}`"
                      ></nldd-menu-item>
                    </nldd-menu>
                  </nldd-combo-box>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Bron output" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-combo-box
                    v-if="availableOutputs.length > 0"
                    ref="outputComboBoxEl"
                    size="md"
                    width="100%"
                    placeholder="Selecteer output..."
                    accessible-label="Bron output"
                    :value="values.sourceOutput"
                    data-testid="output-combo-box"
                    @change="onOutputComboChange"
                  >
                    <nldd-menu>
                      <nldd-menu-item
                        v-for="out in availableOutputs"
                        :key="out.name"
                        :text="outputDisplayName(out)"
                        :value="out.name"
                      ></nldd-menu-item>
                    </nldd-menu>
                  </nldd-combo-box>
                  <nldd-text-field v-else size="md" :value="values.sourceOutput" data-testid="output-text-field" @input="values.sourceOutput = $event.target?.value ?? $event.detail?.value ?? values.sourceOutput"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>

            <template v-if="values.sourceParameters?.length">
              <nldd-spacer size="12"></nldd-spacer>
              <nldd-title size="6"><h6>Bron parameters</h6></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-list appearance="box-tinted" class="edit-settings-list" data-testid="source-parameters-list">
                <nldd-list-item
                  v-for="param in values.sourceParameters"
                  :key="param._rowId"
                  size="md"
                >
                  <nldd-text-cell
                    :text="param.key"
                    max-width="140px"
                    :data-testid="`source-param-key-${param._rowId}`"
                  ></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell>
                    <nldd-dropdown size="md" :data-testid="`source-param-value-${param._rowId}`">
                      <select :value="param.value" :aria-label="`Waarde voor ${param.key}`" @change="param.value = ($event.target as HTMLSelectElement).value">
                        <option value="">Selecteer...</option>
                        <option v-if="param.value && !param.value.startsWith('$')" :value="param.value" :selected="true">{{ param.value }}</option>
                        <optgroup v-for="[category, opts] in paramValueGroups" :key="category" :label="category">
                          <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === param.value">{{ opt.label }}</option>
                        </optgroup>
                      </select>
                    </nldd-dropdown>
                  </nldd-cell>
                </nldd-list-item>
              </nldd-list>
            </template>
          </template>

          <!-- Output -->
          <template v-if="item.section === 'output' || item.section === 'add-output'">
            <nldd-list appearance="box-tinted" class="edit-settings-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-text-field size="md" :value="values.name" @input="values.name = $event.target?.value ?? $event.detail?.value ?? values.name"></nldd-text-field>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Type" max-width="140px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-dropdown size="md">
                    <select :value="values.type" @change="values.type = ($event.target as HTMLSelectElement).value" aria-label="Type">
                      <option v-for="t in typeOptions" :key="t" :value="t">{{ t }}</option>
                    </select>
                  </nldd-dropdown>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
      </nldd-simple-section>

      <nldd-container v-if="isDirty" slot="footer" padding="16">
        <nldd-button appearance="primary" size="md" width="full" data-testid="edit-sheet-save-btn" @click="save" text="Opslaan"></nldd-button>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

<style>
/* Form field layout in the edit sheet's settings list.
 *
 * The previous attempt used `display: grid` on the host element, but
 * `nldd-list-item` is a Lit web component whose internal shadow DOM lays
 * out slotted children with its own flexbox - the user-side grid rule
 * is silently ignored, so the labels collapsed and the value fields
 * shrank to ~80px wide.
 *
 * The pattern that DOES work is the one OperationSettings.vue uses:
 * pin the label cell width via the nldd-text-cell `max-width` attribute
 * (handled inside the component's shadow DOM), and let the value cell
 * grow with `flex: 1; min-width: 0`. The slotted children participate
 * as flex items in nldd-list-item's shadow DOM flex container. */
.edit-settings-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.edit-settings-list nldd-text-field,
.edit-settings-list nldd-dropdown,
.edit-settings-list nldd-number-field {
  width: 100%;
}
</style>
