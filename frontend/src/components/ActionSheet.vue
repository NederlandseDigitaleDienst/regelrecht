<script setup lang="ts">
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { buildOperationTree, type OperationNode, type OperationTreeRow } from '../utils/operationTree';
import OperationSettings, { type OperationSettingsArticle } from './OperationSettings.vue';
import { articleContext, formatLiteral, pathKey, renderAction, renderFocus } from '@regelrecht/frontend-shared/regelrijm/index';
import type { Path, Segment, Token } from '@regelrecht/frontend-shared/regelrijm/index';
import { useFeatureFlags } from '../composables/useFeatureFlags';
import { fetchLaw } from '../composables/useLaw';
import { humanize } from '../utils/outputFormat';
import RegelRijmText from '../regelrijm/RegelRijmText.vue';
import { followReference, isNavigable, originHint, rawAt, resolveReference, type NavigableTarget } from '../regelrijm/regelrijmNav';

/** An execution action: an operation node (or a `value`) plus its output name. */
interface SheetAction extends Partial<OperationNode> {
  output?: string;
}

const props = withDefaults(defineProps<{
  action?: SheetAction | null;
  article?: OperationSettingsArticle | null;
  editable?: boolean;
  /** A freshly added action - Save is always offered (you opened the
   *  sheet to create it), regardless of the dirty snapshot. */
  isNew?: boolean;
  /** Show the selected part as a RegelRijm reading (with flag
   *  `panel.regelrijm`); only the editor sets it, as it handles `navigate`. */
  regelrijm?: boolean;
  /** For the RegelRijm reading (flag `panel.regelrijm`): the law the article
   *  belongs to, so a name in the reading can lead to the rule computing it. */
  law?: Record<string, unknown> | null;
  lawId?: string | null;
  trajectRef?: string | null;
  /** The path of the part to open at, from a click on an operation word in a
   *  RegelRijm reading. Null opens at the top of the action. */
  focusPath?: Path | null;
}>(), {
  action: null,
  article: null,
  editable: false,
  isNew: false,
  regelrijm: false,
  law: null,
  lawId: null,
  trajectRef: null,
  focusPath: null,
});

const emit = defineEmits<{
  close: [];
  save: [];
  edit: [];
  navigate: [target: { lawId: string; articleNumber: string }];
  'open-output': [output: string];
}>();
const { isEnabled } = useFeatureFlags();

const sheetEl = ref<NLDDSheet | null>(null);

const operationTree = computed(() => props.action ? buildOperationTree(props.action) : []);

const selectedOpIndex = ref(0);

// Monotonic counter used as :key for the nldd-page element so it remounts
// every time a new action opens (fixing the sticky-header height
// measurement) without remounting on every keystroke in the output field.
let actionSeq = 0;
const actionKey = ref('none');

// Snapshot the action when it opens so we can show "Opslaan" only when
// something actually changed. Edits mutate the action in place, so a
// JSON compare of the live action vs this baseline is the dirty signal.
// Perf note: this is O(action tree) and re-runs whenever any nested
// field changes (an edit), not on every unrelated render. It's the
// same snapshot approach EditSheet/ScenarioBuilder use deliberately for
// consistency. If a pathologically deep SWITCH/IF tree ever makes the
// per-keystroke stringify noticeable, swap to a deep watch that flips a
// boolean (O(change) instead of O(tree)).
const actionBaseline = ref('');
const isDirty = computed(() => {
  if (!props.action) return false;
  try {
    return JSON.stringify(props.action) !== actionBaseline.value;
  } catch {
    return true;
  }
});

watch(() => props.action, async (action) => {
  selectedOpIndex.value = 0;
  actionKey.value = action ? String(++actionSeq) : 'none';

  if (!action) {
    sheetEl.value?.hide();
    return;
  }
  actionBaseline.value = JSON.stringify(action);
  if (props.focusPath) selectPath(props.focusPath);
  await nextTick();
  sheetEl.value?.show();
}, { immediate: true });

const selectedOperation = computed(() => operationTree.value[selectedOpIndex.value] ?? null);

// When the action has no operation (e.g. `value: $PERCENTAGE_LID_5`), the
// operation tree is empty and the sheet would otherwise show a blank body.
// Show the value verbatim from the YAML - `$VAR` refs stay in their CAPS +
// underscore form because the `$` is a code-marker that pairs visually
// with code-style identifiers (humanizing them to `$percentage lid 5`
// reads inconsistently).
const directValue = computed(() => {
  if (selectedOperation.value) return null;
  const v = props.action?.value;
  if (v == null) return null;
  return { label: String(v) };
});

const parentOperations = computed(() => {
  const selected = selectedOperation.value;
  if (!selected) return [];
  return operationTree.value.filter(op =>
    op !== selected && selected.number.startsWith(op.number + '.')
  );
});

function selectOperation(op: OperationTreeRow) {
  const idx = operationTree.value.indexOf(op);
  if (idx >= 0) selectedOpIndex.value = idx;
}

function selectOperationByNode(node: OperationNode) {
  const idx = operationTree.value.findIndex(op => op.node === node);
  if (idx >= 0) selectedOpIndex.value = idx;
}

// The output field only renders while an action is open (directly, or via
// directValue, which is null without one), so `action` is set here.
function onOutputInput(event: Event) {
  const action = props.action!;
  const target = event.target as { value?: string } | null;
  const detail = (event as CustomEvent<{ value?: string } | undefined>).detail;
  action.output = target?.value ?? detail?.value ?? action.output;
}

// RegelRijm (RFC-054), behind `panel.regelrijm`: the selected operation as
// readable text above its settings. An operation word in it selects that
// part, so the reading is the way down into the tree; a name another rule
// computes is a token that leads to that rule.
const navError = ref<string | null>(null);
watch(() => props.action, () => { navError.value = null; });

/** The reading shown above the settings; `error` when it cannot be read. */
interface Reading {
  error: string | null;
  segments: Segment[];
  selectable: Set<string>;
  navigable: Set<string>;
  hints: Map<string, string>;
}

const reading = computed((): Reading | null => {
  if (!props.regelrijm || !isEnabled('panel.regelrijm') || !props.action) return null;
  try {
    const full = renderAction(props.action, { article: props.article });
    // An action without an operation (a value or a single name) reads whole.
    if (!selectedOperation.value) return { error: null, segments: full.segments, selectable: new Set<string>(), ...namesIn(full.segments) };
    // Every operation of the reading, by its node in the action as written.
    const pathOf = new Map<object, string[]>();
    for (const key of full.numbers.keys()) {
      const node = rawAt(props.action, key.split('/'));
      if (node && typeof node === 'object' && !pathOf.has(node)) pathOf.set(node, key.split('/'));
    }
    const path = pathOf.get(selectedOperation.value.node) ?? ['value'];
    const focus = renderFocus(props.action, path, { article: props.article, depth: Infinity });
    const inTree = new Set<object>(operationTree.value.map((op) => op.node));
    const selectable = new Set<string>();
    for (const [node, p] of pathOf) if (inTree.has(node) && node !== selectedOperation.value.node) selectable.add(pathKey(p));
    return { error: null, segments: focus.segments, selectable, ...namesIn(focus.segments) };
  } catch (e) {
    return { error: (e as Error).message, segments: [], selectable: new Set(), navigable: new Set(), hints: new Map() };
  }
});

// The names in a reading: which lead to the rule computing them, and the
// tooltip of each, saying where its value comes from.
function namesIn(segments: readonly Segment[]) {
  let declared;
  try {
    declared = articleContext(props.article)?.declared;
  } catch {
    declared = undefined;
  }
  const format = (t: { name: string; value: unknown }) => formatLiteral(t.value, declared?.get(t.name)) ?? JSON.stringify(t.value);
  const navigable = new Set<string>();
  const hints = new Map<string, string>();
  for (const seg of segments) {
    if (typeof seg === 'string' || (seg.tk !== 'name' && seg.tk !== 'term') || seg.path?.[0] === 'output' || seg.ref === undefined) continue;
    const target = resolveReference(seg.ref, props.law, props.article);
    if (isNavigable(target)) navigable.add(seg.ref);
    const text = originHint(seg.ref, target, { destination: destinationOf, format, label: humanize });
    if (text) hints.set(seg.ref, text);
  }
  return { navigable, hints };
}

// Where a computed name is computed, for its tooltip. Another regelwerk is
// named by its id; which article computes it there is only known on a click.
function destinationOf(target: NavigableTarget) {
  if (target.kind !== 'action') return humanize(target.lawId);
  const sameLaw = target.lawId === (props.law?.$id ?? props.lawId);
  if (sameLaw && String(target.articleNumber) === String((props.article as { number?: string | number } | null)?.number)) return 'dit artikel';
  return `${humanize(target.lawId)}, artikel ${target.articleNumber}`;
}

function selectPath(path: Path) {
  const node = rawAt(props.action, path);
  if (node) selectOperationByNode(node as OperationNode);
}

async function followName(seg: Token) {
  navError.value = null;
  // Only a name with a reference can be followed (RegelRijmText's `navigable`).
  if (seg.ref === undefined) return;
  // Following a name closes the sheet; unsaved edits would go with it.
  if (isDirty.value) {
    navError.value = 'Sla je wijziging eerst op of annuleer hem; daarna kun je naar die rekenregel.';
    return;
  }
  const from = props.action;
  const to = await followReference(seg.ref, {
    law: props.law, lawId: props.lawId, article: props.article, label: humanize,
    load: (id: string) => fetchLaw(props.trajectRef ?? null, id),
  });
  // The sheet may have closed or moved on while another law loaded.
  if (!to || props.action !== from) return;
  if (to.kind === 'error') navError.value = to.message;
  else if (to.kind === 'navigate') emit('navigate', { lawId: to.lawId, articleNumber: to.articleNumber });
  else emit('open-output', to.output);
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.action) {
    emit('close');
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <nldd-sheet ref="sheetEl" placement="right" :width="editable ? '640px' : '480px'" @close="emit('close')">
    <!-- :key forces nldd-page to remount whenever a NEW action opens.
         nldd-page captures the sticky-header height ONCE per mount via
         requestAnimationFrame; when the sheet opens with a new action the
         header may already be rendered but the measurement happened while
         the sheet was still hidden, producing a zero-height offset that
         lets the body content slide up under the title bar (visible as a
         fade). Remounting fixes the measurement.

         actionKey is captured once when the action changes (in the
         watcher), NOT reactively bound to action.output, so editing the
         output text field does not trigger a remount on every keystroke. -->
    <nldd-page :key="actionKey" sticky-header>
      <nldd-top-title-bar slot="header" text="Actie" :dismiss-text="editable ? 'Annuleer' : 'Sluit'" @dismiss="emit('close')"></nldd-top-title-bar>

      <nldd-simple-section>
        <!-- Output binding lives inside the operation's settings list (via
             OperationSettings' lead-row slot) at the top level, or in the
             direct-value list below - so it sits in the same box as the
             other root rows instead of a separate box. -->

        <!-- Section A: Bovenliggende operaties -->
        <template v-if="parentOperations.length">
          <nldd-list appearance="box-tinted">
            <!-- Back/up navigation - clickable parent rows with a
                 chevron-left, identical in view and edit: click any
                 ancestor to jump up one or more levels. -->
            <nldd-list-item
              v-for="op in parentOperations"
              :key="op.number"
              size="md"
              :data-testid="`parent-op-${op.number}`"
              button
              @click="selectOperation(op)"
            >
              <nldd-icon-cell size="20">
                <nldd-icon icon="chevron-left"></nldd-icon>
              </nldd-icon-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-text-cell :text="`${op.number}. ${op.title}`" :supporting-text="op.subtitle">
              </nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </template>

        <!-- RegelRijm reading of the selected part (flag panel.regelrijm) -->
        <template v-if="reading">
          <nldd-spacer v-if="parentOperations.length" size="16"></nldd-spacer>
          <nldd-box background="tinted" data-testid="action-sheet-regelrijm">
            <nldd-container padding="16">
              <nldd-inline-dialog v-if="reading.error" variant="alert" text="Deze regel is nog niet te lezen" :supporting-text="reading.error"></nldd-inline-dialog>
              <RegelRijmText
                v-else
                :segments="reading.segments"
                interactive
                :navigable="reading.navigable"
                :blocks="reading.navigable"
                :selectable="reading.selectable"
                :hints="reading.hints"
                @select="selectPath"
                @follow="followName"
              />
            </nldd-container>
          </nldd-box>
          <template v-if="navError">
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-banner variant="warning" :text="navError"></nldd-banner>
          </template>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <!-- Section B: Operation Settings -->
        <nldd-spacer v-if="!reading && parentOperations.length && selectedOperation" size="24"></nldd-spacer>
        <OperationSettings v-if="selectedOperation" :operation="selectedOperation" :article="article" :editable="editable" :hide-title-row="editable && !parentOperations.length" @select-operation="selectOperationByNode">
          <template #lead-row>
            <nldd-list-item v-if="editable && action && !parentOperations.length" size="md">
              <nldd-text-cell text="Output" width="120px"></nldd-text-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field size="md" :value="action?.output" @input="onOutputInput" data-testid="action-output-field"></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
          </template>
        </OperationSettings>

        <!-- Direct value: action has no operation, just outputs a value
             (literal or $VAR reference). Mirror OperationSettings' Titel +
             Waarde layout so the sheet body isn't blank and the user sees
             which action they're looking at. -->
        <nldd-list v-if="directValue" appearance="box-tinted">
          <nldd-list-item size="md">
            <nldd-text-cell text="Output" :width="editable ? '120px' : 'fit-content'"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-cell v-if="editable">
              <nldd-text-field size="md" :value="action?.output" @input="onOutputInput" data-testid="action-output-field"></nldd-text-field>
            </nldd-cell>
            <nldd-text-cell v-else horizontal-alignment="right" :text="action?.output || '(leeg)'"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-text-cell text="Waarde" width="fit-content"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-text-cell horizontal-alignment="right" :text="directValue.label"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </nldd-simple-section>

      <nldd-container slot="footer" padding="16">
        <nldd-button v-if="editable && (isNew || isDirty)" appearance="primary" size="md" width="full" data-testid="action-sheet-save-btn" @click="emit('save')" text="Opslaan"></nldd-button>
        <nldd-button v-else-if="!editable" appearance="secondary" size="md" width="full" data-testid="action-sheet-edit-btn" @click="emit('edit')" text="Bewerken"></nldd-button>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>
