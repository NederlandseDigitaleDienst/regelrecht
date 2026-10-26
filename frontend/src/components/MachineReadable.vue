<script setup lang="ts">
import { isRawDefinition, type ExecutionAction, type ExecutionField, type MachineReadableSection, type RawDefinition } from '../lib/lawFormat';
import { computed, inject, ref, toRef, watch, nextTick } from 'vue';
import { humanize } from '../utils/outputFormat';
import { decisionOf } from '@regelrecht/frontend-shared/produces';
import type { Decision, ProducedItem } from '@regelrecht/frontend-shared/produces';
import { useCorpusLaws } from '../composables/useCorpusLaws';
import BreakableName from './BreakableName.vue';
import RowActionsMenu from './RowActionsMenu.vue';
import MachineEmptyState from './MachineEmptyState.vue';
import RegelRijmPane from '../regelrijm/RegelRijmPane.vue';
import type { LinkedFragment } from '../regelrijm/regelrijmLinks';
import type { Path } from '@regelrecht/frontend-shared/regelrijm/index';
import { useFeatureFlags } from '../composables/useFeatureFlags';

// Law-format shapes (schema/latest/schema.json), only the fields read here.

/** A definition written as an object, with its value and type_spec. */
interface DefinitionObject {
  value?: unknown;
  type_spec?: { unit?: string } | null;
}

/** The article prop: only the fields this component reads. */
interface MachineReadableArticle {
  number?: string | number;
  machine_readable?: MachineReadableSection | null;
}

type ItemSection = 'parameter' | 'input' | 'output' | 'action';

/** Payload of `open-edit`: an existing item to edit, or a new one to add. */
type OpenEditPayload =
  | { section: 'definition'; key: string; rawDef: RawDefinition }
  | { section: 'parameter' | 'input' | 'output'; index: number; data: ExecutionField }
  | { section: 'add-definition' | 'add-parameter' | 'add-input' | 'add-output'; isNew: true };

/** Payload of `delete`: the section plus the identity of the row. */
type DeletePayload =
  | { section: 'definition'; key: string }
  | { section: ItemSection; index: number };

/** A staged delete: the payload plus the label the confirmation shows. */
type PendingDelete = DeletePayload & { label: string };

/** The subset of the nldd-modal-dialog element used here; see the watcher. */
interface ModalDialogElement extends HTMLElement {
  show?: () => void;
  hide?: () => void;
}

const props = withDefaults(defineProps<{
  article?: MachineReadableArticle | null;
  editable?: boolean;
  /** True when the in-memory machine_readable differs from the saved copy */
  dirty?: boolean;
  /** True while a save PUT is in flight */
  saving?: boolean;
  /** Error from the most recent save attempt (Error instance or null) */
  /** Active traject ref - scopes the corpus-laws lookup so display
   *  names reflect the traject's view (including its in-progress edits). */
  trajectRef?: string | null;
  /** Read-only context: offer a button that opens the editor so the missing
   *  machine-readable version can be created there. Ignored when `editable`,
   *  which seeds it in place instead. */
  canCreate?: boolean;
  /** Whether an enrichment can be requested here. The run covers the whole
   *  law, not just this article, so the empty state says so. */
  canEnrich?: boolean;
  /** Doorgegeven aan MachineEmptyState: '' | 'login' | 'traject'. */
  needs?: string;
  /** An enrichment for this law is already pending or processing. The empty
   *  state then reports that instead of offering the buttons again. */
  enriching?: boolean;
  /** Message from the most recent failed enrich request, shown in place of
   *  the supporting text. */
  enrichError?: string;
  /** Er staat een openstaande review-taak klaar voor deze wet. */
  reviewReady?: boolean;
  reviewArticle?: string;
  /** Anchor target for that button. Leave unset when the user isn't logged
   *  in, so the click gates on the login popover instead of the href. */
  createHref?: string;
  /** Show the actions as RegelRijm readings (with flag `panel.regelrijm`).
   *  Only the editor sets it: it is the view that handles `navigate`. */
  regelrijm?: boolean;
  /** For the RegelRijm reading: the law the article
   *  belongs to, so a name in the reading leads to the rule computing it. */
  law?: Record<string, unknown> | null;
  lawId?: string | null;
  /** Output → fragments of the article text that linking notes tie to it. */
  links?: ReadonlyMap<string, LinkedFragment[]>;
}>(), {
  article: null,
  editable: false,
  dirty: false,
  saving: false,
  trajectRef: null,
  canCreate: false,
  canEnrich: false,
  needs: '',
  enriching: false,
  enrichError: '',
  reviewReady: false,
  reviewArticle: '',
  createHref: undefined,
  regelrijm: false,
  law: null,
  lawId: null,
  links: () => new Map(),
});

const { displayName: lawDisplayName } = useCorpusLaws(toRef(props, 'trajectRef'));

// Provided by AppShell; see LibraryView's "Bewerken" button.
const onLoginTriggerPointerdown = inject('onLoginTriggerPointerdown', () => {});

const emit = defineEmits<{
  /** `path` selects a part of the action in the sheet. */
  'open-action': [action: ExecutionAction, path?: Path];
  /** RegelRijm: a name leads to the article that computes it. */
  'navigate': [target: { lawId: string; articleNumber: string }];
  'open-edit': [payload: OpenEditPayload];
  'init-mr': [];
  /** Ask for a generated proposal instead of writing one by hand. */
  'enrich': [trigger: EventTarget | null];
  /** Open the task list filtered to this law, while a run is pending. */
  'view-tasks': [];
  /** Open review-modus voor de klaarstaande taak. */
  'review': [];
  /**
   * Read-only empty state: the user wants the machine-readable version
   * created, which only the editor can do. Payload is the button element, so
   * the parent can anchor the login popover to it.
   */
  'create-mr': [trigger: EventTarget | null];
  'add-action': [];
  // The Machine-pane "Opslaan" button - a real backend save of the law.
  'save': [];
  /**
   * Patch a single machine_readable field in place (no backend save, just
   * marks the model dirty). Same `{ section, key, data }` shape the parent's
   * handleSave dispatches on - used for inline edits like the produces
   * dropdowns. Distinct from `save` (which is the law PUT).
   */
  'patch': [payload: { section: 'produces'; key: string; data: string | null }];
  /**
   * Delete a single item from the machine_readable. Payload shape mirrors
   * `open-edit` so the parent's `handleSave`/`handleDelete` can dispatch on
   * `section`. Examples:
   *   { section: 'definition', key: 'drempelinkomen_alleenstaande' }
   *   { section: 'parameter', index: 0 }
   *   { section: 'input', index: 2 }
   *   { section: 'output', index: 1 }
   *   { section: 'action', index: 0 }
   */
  'delete': [payload: DeletePayload];
}>();

const mr = computed<MachineReadableSection | null>(() => props.article?.machine_readable ?? null);
const execution = computed(() => mr.value?.execution ?? null);

const definitions = computed(() => {
  const defs = mr.value?.definitions;
  if (!defs) return [];
  return Object.entries(defs).map(([name, def]) => {
    // A YAML-null definition (`foo: ~`) is a bare null value, not an object.
    const isObject = def !== null && typeof def === 'object';
    const val = isObject ? (def as DefinitionObject).value : def;
    const unit = isObject ? (def as DefinitionObject).type_spec?.unit : undefined;
    // Per-token parts for read-only display: a list keeps its raw items
    // (each wrapped in <wbr>-breakable tokens), a scalar is the formatted
    // single value. Lets long underscore identifiers wrap instead of
    // overflowing the (no longer fit-content) value cell.
    const isList = Array.isArray(val);
    const parts = isList ? val.map((v) => String(v)) : [formatValue(val, unit)];
    return { name, value: val, unit, isList, parts };
  });
});

// The decision the article produces, in the object form's terms: the object
// itself, as written, or in a list (schema v0.8.1) the item with a decision
// `type`. The fields below show and edit it; EditorView patches the same
// item.
const produces = computed<ProducedItem | Decision | null>(() => {
  const raw = execution.value?.produces;
  if (raw == null) return null;
  if (!Array.isArray(raw)) return raw;
  return decisionOf(raw) ?? {};
});

// Whether there is a decision to hold the decision-only fields. A list
// without a decision item has none until a legal character adds one, so
// its decision type is not offered before then.
const hasDecision = computed(() => {
  const raw = execution.value?.produces;
  return !Array.isArray(raw) || decisionOf(raw) != null;
});

// Enum values copied from the law schema. Keep in sync with
// schema/latest/schema.json → execution.produces.legal_character.enum
// and .decision_type.enum. If the schema adds a value here, add it here
// too or the dropdown silently can't set it.
//
// That is not hypothetical: RECHTSPOSITIE was added to legal_character in
// schema v0.5.9 (RFC-038) and this list was not updated, so the editor could
// not set it on a law for the whole of that release. Current as of v0.8.12.
const LEGAL_CHARACTERS = [
  'BESCHIKKING', 'TOETS', 'WAARDEBEPALING', 'BESLUIT_VAN_ALGEMENE_STREKKING', 'INFORMATIEF',
  'RECHTSPOSITIE', 'UITSPRAAK',
];
const DECISION_TYPES = [
  'TOEKENNING', 'AFWIJZING', 'GOEDKEURING', 'GEEN_BESLUIT',
  'ALGEMEEN_VERBINDEND_VOORSCHRIFT', 'BELEIDSREGEL', 'VOORBEREIDINGSBESLUIT',
  'ANDERE_HANDELING', 'AANSLAG',
];

// Mutations live in the parent; emit a save with the produces section.
// An empty selection clears the key (data: null).
function updateProduces(key: string, value: string) {
  emit('patch', { section: 'produces', key, data: value === '' ? null : value });
}

const parameters = computed(() =>
  (execution.value?.parameters ?? []).map((p) => ({
    name: p.name,
    type: p.type,
    required: p.required ?? false,
  }))
);

const inputs = computed(() =>
  (execution.value?.input ?? []).map((i) => ({
    name: i.name,
    type: i.type,
    // Only an actual source regulation gets a supporting line. A cross-law
    // output reference humanises to roughly the field name, which would
    // just duplicate it.
    sourceRegulation: i.source?.regulation ?? null,
  }))
);

const outputs = computed(() =>
  (execution.value?.output ?? []).map((o) => ({
    name: o.name,
    type: o.type,
  }))
);

const actions = computed(() => execution.value?.actions ?? []);
// Behind `panel.regelrijm` the actions come first, as RegelRijm readings: the
// way into the article's rules, instead of a list of output names at the end.
const { isEnabled } = useFeatureFlags();
const regelrijm = computed(() => props.regelrijm && isEnabled('panel.regelrijm'));

function onProducesChange(key: string, event: Event) {
  updateProduces(key, (event.target as HTMLSelectElement).value);
}

function formatValue(val: unknown, unit: string | undefined): string {
  if (typeof val === 'number') {
    if (unit === 'eurocent') {
      return (val / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR' });
    }
    if (val > 0 && val < 1 && !unit) {
      return (val * 100).toLocaleString('nl-NL', { maximumFractionDigits: 3 }) + '%';
    }
  }
  // A list definition value (YAML sequence) renders here; String([...])
  // joins with a bare comma ("a,b,c") which has no break opportunity and
  // pushes the column width. Use comma + space so it can wrap.
  if (Array.isArray(val)) return val.join(', ');
  // A YAML-null value renders as empty, not as the word "null".
  if (val == null) return '';
  return String(val);
}

// structuredClone throws DataCloneError on Vue reactive proxies; YAML data is JSON-safe.
function snapshot<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

// Open edit sheet for existing items
function editDef(name: string) {
  const rawDef = mr.value?.definitions?.[name];
  // `== null` is the old check; the guard only tells the type what is left.
  if (rawDef == null || !isRawDefinition(rawDef)) return;
  emit('open-edit', { section: 'definition', key: name, rawDef: snapshot(rawDef) });
}

function editParam(index: number) {
  const p = execution.value?.parameters?.[index];
  if (p) emit('open-edit', { section: 'parameter', index, data: snapshot(p) });
}

function editInput(index: number) {
  const raw = execution.value?.input?.[index];
  if (raw) emit('open-edit', { section: 'input', index, data: snapshot(raw) });
}

function editOutput(index: number) {
  const raw = execution.value?.output?.[index];
  if (raw) emit('open-edit', { section: 'output', index, data: snapshot(raw) });
}

// Delete handlers - stage a confirmation in `pendingDelete`, then on
// confirm emit the delete event with the section + identity of the row.
// The parent (EditorApp) is the source of truth for machineReadable,
// so all mutations live there. The modal-dialog confirmation is here
// because there is no undo yet.
const SECTION_LABELS: Record<DeletePayload['section'], string> = {
  definition: 'definitie',
  parameter: 'parameter',
  input: 'input',
  output: 'output',
  action: 'actie',
};

const pendingDelete = ref<PendingDelete | null>(null);
const deleteModalEl = ref<ModalDialogElement | null>(null);
const pendingSectionLabel = computed(
  () => (pendingDelete.value ? SECTION_LABELS[pendingDelete.value.section] ?? '' : ''),
);

watch(pendingDelete, async (val) => {
  await nextTick();
  // Guard against test envs where the modal-dialog custom element isn't
  // upgraded - the ref then holds a plain HTMLElement without show/hide.
  // Optional-chaining on `?.show()` would still throw because it only
  // skips when `.value` is nullish, not when `.show` itself is undefined.
  const el = deleteModalEl.value;
  if (val) {
    if (typeof el?.show === 'function') el.show();
  } else {
    if (typeof el?.hide === 'function') el.hide();
  }
});

function deleteDef(name: string) {
  pendingDelete.value = { section: 'definition', key: name, label: `definitie '${name}'` };
}

function deleteParam(index: number) {
  const p = parameters.value[index];
  pendingDelete.value = { section: 'parameter', index, label: `parameter '${p?.name ?? index}'` };
}

function deleteInput(index: number) {
  const i = inputs.value[index];
  pendingDelete.value = { section: 'input', index, label: `input '${i?.name ?? index}'` };
}

function deleteOutput(index: number) {
  const o = outputs.value[index];
  pendingDelete.value = { section: 'output', index, label: `output '${o?.name ?? index}'` };
}

function deleteAction(index: number) {
  const a = actions.value[index];
  pendingDelete.value = { section: 'action', index, label: `actie '${a?.output ?? index}'` };
}

function confirmDelete() {
  if (!pendingDelete.value) return;
  const { label, ...payload } = pendingDelete.value;
  emit('delete', payload);
  pendingDelete.value = null;
}

function cancelDelete() {
  // Idempotent guard: the Behoud button's @click and the modal's @close
  // both wire to this handler. Clicking Behoud sets pendingDelete = null,
  // the watcher then calls el.hide(), which fires @close → cancelDelete
  // again. Vue's ref equality check makes the second assignment a no-op
  // today, but a future side-effect would silently double-fire without
  // this early return.
  if (pendingDelete.value === null) return;
  pendingDelete.value = null;
}

// Open edit sheet for new items
function addDef() {
  emit('open-edit', { section: 'add-definition', isNew: true });
}

function addParam() {
  emit('open-edit', { section: 'add-parameter', isNew: true });
}

function addInput() {
  emit('open-edit', { section: 'add-input', isNew: true });
}

function addOutput() {
  emit('open-edit', { section: 'add-output', isNew: true });
}
</script>

<template>
  <!-- De verrijking schrijft machine_readable en scenario's, dus dit is de
       plek waar je erom vraagt. De supporting-text benoemt de scope: de run
       gaat over de hele wet, terwijl deze pane één artikel toont. -->
  <MachineEmptyState
    v-if="!mr"
    :review-ready="reviewReady"
    :review-article="reviewArticle"
    :enriching="enriching"
    :can-enrich="canEnrich"
    :needs="needs"
    :can-write-here="editable"
    :can-create="canCreate"
    :create-href="createHref"
    :enrich-error="enrichError"
    @enrich="emit('enrich', $event)"
    @write="emit('init-mr')"
    @create="emit('create-mr', $event)"
    @view-tasks="emit('view-tasks')"
    @review="emit('review')"
  ></MachineEmptyState>

  <div v-else data-testid="machine-readable">
    <template v-if="regelrijm && (actions.length || editable)">
      <RegelRijmPane
        :article="article"
        :law="law"
        :law-id="lawId"
        :traject-ref="trajectRef"
        :editable="editable"
        :links="links"
        @open-action="(action, path) => emit('open-action', action, path)"
        @navigate="emit('navigate', $event)"
        @add-action="emit('add-action')"
        @delete-action="deleteAction"
      />
      <nldd-spacer size="16"></nldd-spacer>
    </template>
    <!-- Metadata: produces -->
    <nldd-list v-if="produces" appearance="box-tinted">
      <nldd-list-item v-if="produces.legal_character || editable" size="md">
        <nldd-text-cell text="Juridische basis" min-width="120px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-cell v-if="editable" width="full" min-width="120px" max-width="280px">
          <nldd-dropdown size="md">
            <select
              :key="`lc-${article?.number}`"
              aria-label="Juridische basis"
              :value="produces.legal_character || ''"
              @change="onProducesChange('legal_character', $event)"
            >
              <option value="">(geen)</option>
              <!-- Preserve a value the hardcoded enum doesn't know (e.g. a
                   law authored against a newer schema, or set via the YAML
                   view): show + keep it instead of silently collapsing to
                   (geen), which any stray edit would then commit. -->
              <option
                v-if="produces.legal_character && !LEGAL_CHARACTERS.includes(produces.legal_character)"
                :value="produces.legal_character"
              >{{ humanize(produces.legal_character) }}</option>
              <option v-for="v in LEGAL_CHARACTERS" :key="v" :value="v">{{ humanize(v) }}</option>
            </select>
          </nldd-dropdown>
        </nldd-cell>
        <nldd-text-cell v-else horizontal-alignment="right" :text="humanize(produces.legal_character)"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item v-if="hasDecision && (produces.decision_type || editable)" size="md">
        <nldd-text-cell text="Besluit-type" min-width="120px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-cell v-if="editable" width="full" min-width="120px" max-width="280px">
          <nldd-dropdown size="md">
            <select
              :key="`dt-${article?.number}`"
              aria-label="Besluit-type"
              :value="produces.decision_type || ''"
              @change="onProducesChange('decision_type', $event)"
            >
              <option value="">(geen)</option>
              <!-- Same as legal_character: keep an out-of-enum value
                   visible/selected so it isn't silently cleared. -->
              <option
                v-if="produces.decision_type && !DECISION_TYPES.includes(produces.decision_type)"
                :value="produces.decision_type"
              >{{ humanize(produces.decision_type) }}</option>
              <option v-for="v in DECISION_TYPES" :key="v" :value="v">{{ humanize(v) }}</option>
            </select>
          </nldd-dropdown>
        </nldd-cell>
        <nldd-text-cell v-else horizontal-alignment="right" :text="humanize(produces.decision_type)"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>

    <!-- Definities -->
    <template v-if="definitions.length || editable">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5" data-testid="section-definitions"><h5>Definities</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="box-tinted">
        <nldd-list-item v-for="def in definitions" :key="def.name" size="md">
          <template v-if="editable">
            <nldd-text-cell><BreakableName :name="def.name" /> = {{ formatValue(def.value, def.unit) }}</nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
          </template>
          <template v-else>
            <nldd-text-cell min-width="120px"><BreakableName :name="def.name" /></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell
              v-if="def.isList"
              horizontal-alignment="right"
            ><template
              v-for="(part, i) in def.parts"
              :key="i"
            ><span v-if="i > 0">, </span><BreakableName :name="part" /></template></nldd-text-cell>
            <nldd-text-cell
              v-else
              width="fit-content"
              horizontal-alignment="right"
              :text="def.parts[0]"
            ></nldd-text-cell>
          </template>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor definitie ${def.name}`"
              :delete-testid="`def-${def.name}-delete-btn`"
              @edit="editDef(def.name)"
              @delete="deleteDef(def.name)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-def-btn" @click="addDef" text="Definitie toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length || editable">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5" data-testid="section-parameters"><h5>Parameters</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="box-tinted">
        <nldd-list-item v-for="(param, index) in parameters" :key="param.name" size="md">
          <nldd-text-cell><BreakableName :name="param.name" /> <nldd-tag size="sm" :text="param.type"></nldd-tag></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor parameter ${param.name}`"
              :delete-testid="`param-${param.name}-delete-btn`"
              @edit="editParam(index)"
              @delete="deleteParam(index)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-param-btn" @click="addParam" text="Parameter toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length || editable">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5" data-testid="section-inputs"><h5>Inputs</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="box-tinted">
        <nldd-list-item v-for="(input, index) in inputs" :key="input.name" :data-testid="`input-row-${input.name}`" size="md">
          <nldd-text-cell
            :supporting-text="input.sourceRegulation ? lawDisplayName(input.sourceRegulation) : undefined"
          ><BreakableName :name="input.name" /> <nldd-tag size="sm" :text="input.type"></nldd-tag></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor input ${input.name}`"
              :edit-testid="`input-${input.name}-edit-btn`"
              :delete-testid="`input-${input.name}-delete-btn`"
              @edit="editInput(index)"
              @delete="deleteInput(index)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-input-btn" @click="addInput" text="Input toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length || editable">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5" data-testid="section-outputs"><h5>Outputs</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="box-tinted">
        <nldd-list-item v-for="(output, index) in outputs" :key="output.name" size="md">
          <nldd-text-cell><BreakableName :name="output.name" /> <nldd-tag size="sm" :text="output.type"></nldd-tag></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor output ${output.name}`"
              :delete-testid="`output-${output.name}-delete-btn`"
              @edit="editOutput(index)"
              @delete="deleteOutput(index)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-output-btn" @click="addOutput" text="Output toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Acties (with the flag panel.regelrijm: first, as readings) -->
    <template v-if="!regelrijm && (actions.length || editable)">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="5" data-testid="section-actions"><h5>Acties</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="box-tinted">
        <nldd-list-item
          v-for="(action, index) in actions"
          :key="index"
          size="md"
          :button="!editable"
          @click="!editable && emit('open-action', action)"
        >
          <nldd-text-cell :text="action.output"></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor actie ${action.output}`"
              :edit-testid="`action-${action.output}-edit-btn`"
              :delete-testid="`action-${action.output}-delete-btn`"
              @edit="emit('open-action', action)"
              @delete="deleteAction(index)"
            />
          </nldd-cell>
          <template v-else>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon icon="chevron-right"></nldd-icon>
            </nldd-icon-cell>
          </template>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-action-btn" @click="emit('add-action')" text="Actie toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </div>

  <nldd-modal-dialog
    ref="deleteModalEl"
    variant="alert"
    :text="pendingDelete ? `Weet je zeker dat je ${pendingDelete.label} wilt verwijderen?` : ''"
    supporting-text="Deze actie kan niet ongedaan gemaakt worden."
    @close="cancelDelete"
  >
    <nldd-button slot="actions" appearance="primary" :text="`Behoud ${pendingSectionLabel}`" @click="cancelDelete"></nldd-button>
    <nldd-button slot="actions" appearance="destructive" text="Verwijder" @click="confirmDelete"></nldd-button>
  </nldd-modal-dialog>
</template>
