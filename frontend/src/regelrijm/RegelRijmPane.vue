<script setup lang="ts">
// RegelRijm in the Machine pane (RFC-054): every rule of the article as
// readable text, always in full, as the way into its actions. It reads the
// article as it is being edited, so a change in the sheet or the YAML shows up
// here at once. An operation word opens the action at that part; a name
// another rule computes is a token that leads there, in this article, another
// article or another regelwerk. Hovering or focusing a name says where it
// comes from. Under a rule stand the fragments of the article text that a
// linking note ties to it (regelrijmLinks.ts). Behind the feature flag
// `panel.regelrijm`.
import { computed, reactive, ref, useId, watch } from 'vue';
import { articleContext, formatLiteral, references, renderAction } from '@regelrecht/frontend-shared/regelrijm/index';
import type { ActionReading, ArticleContext, Path, Token, Unit } from '@regelrecht/frontend-shared/regelrijm/index';
import { fetchLaw } from '../composables/useLaw';
import { humanize } from '../utils/outputFormat';
import RowActionsMenu from '../components/RowActionsMenu.vue';
import QuotedFragment from '../components/QuotedFragment.vue';
import RegelRijmText from './RegelRijmText.vue';
import { findOutput, followReference, isNavigable, originHint, resolveReference } from './regelrijmNav';
import type { NavigableTarget, ReferenceTarget } from './regelrijmNav';
import type { LinkedFragment } from './regelrijmLinks';
import type { ExecutionAction, MachineReadableSection } from '../lib/lawFormat';

/** The article as the pane reads it. */
interface PaneArticle {
  number?: string | number;
  machine_readable?: MachineReadableSection | null;
}

const props = withDefaults(defineProps<{
  article?: PaneArticle | null;
  law?: Record<string, unknown> | null;
  lawId?: string | null;
  trajectRef?: string | null;
  editable?: boolean;
  /** Output → fragments of the article text linked to it (linksByOutput). */
  links?: ReadonlyMap<string, LinkedFragment[]>;
}>(), {
  article: null,
  law: null,
  lawId: null,
  trajectRef: null,
  editable: false,
  links: () => new Map(),
});
const emit = defineEmits<{
  /** `path`: the part of the action to open the sheet at. */
  'open-action': [action: ExecutionAction, path?: Path];
  'navigate': [target: { lawId: string; articleNumber: string }];
  'add-action': [];
  'delete-action': [index: number];
}>();

/** One rule of the article: its reading, or why it cannot be read. */
type Rule = {
  key: string;
  index: number;
  action: ExecutionAction;
  output: string | null;
} & (
  | { error: string; reading?: undefined; targets?: undefined; unit?: undefined }
  | {
    error?: undefined;
    reading: ActionReading;
    navigable: Set<string>;
    targets: Map<string, ReferenceTarget>;
    selectable: Set<string>;
    unit?: Map<string, Unit>;
  }
);

/** What is known of a regelwerk a name in this pane is computed in. */
interface Elsewhere {
  loading?: boolean;
  error?: string;
  lawName?: string;
  articleNumber?: string | null;
}

const navError = ref<string | null>(null);
watch(() => props.article?.number, () => {
  navError.value = null;
});

// Element ids per pane, so two Machine panes side by side never share one.
const uid = useId();

// The YAML is unchecked: an entry typed as an action may be anything mid-edit,
// which is why every rule below checks it before reading it.
const actions = computed((): ExecutionAction[] => {
  const list = props.article?.machine_readable?.execution?.actions;
  return Array.isArray(list) ? list : [];
});

const rules = computed(() => {
  let ctx: ArticleContext | undefined;
  try {
    ctx = articleContext(props.article);
  } catch {
    ctx = undefined;
  }
  return actions.value.map((action, index): Rule => {
    const key = String(index);
    const output = action && typeof action === 'object' && typeof action.output === 'string' ? action.output : null;
    // A tree half-way through an edit may not read yet; say so per rule.
    if (!action || typeof action !== 'object' || Array.isArray(action)) return { key, index, action, output, error: 'Deze actie is nog leeg.' };
    try {
      const reading = renderAction(action, { article: props.article, ctx });
      const navigable = new Set<string>();
      const targets = new Map<string, ReferenceTarget>();
      for (const r of references(reading.segments)) {
        if (r === `$${output}`) continue;
        const target = resolveReference(r, props.law, props.article);
        targets.set(r, target);
        if (isNavigable(target)) navigable.add(r);
      }
      // Every operation word opens the action at its part.
      const selectable = new Set(reading.numbers.keys());
      return { key, index, action, output, reading, navigable, targets, selectable, unit: ctx?.declared };
    } catch (e) {
      return { key, index, action, output, error: (e instanceof Error ? e.message : String(e)) };
    }
  });
});

// Where a name in another regelwerk is computed is only known once that
// regelwerk is loaded; the tooltip fills in as it arrives.
const elsewhere = reactive<Record<string, Elsewhere>>({});
const ownName = ref<string | null>(null);
watch(() => [props.lawId, props.trajectRef], async ([id, traject]) => {
  ownName.value = null;
  if (!id) return;
  let name: string | null = null;
  try {
    name = (await fetchLaw(traject, id))?.lawName ?? null;
  } catch {
    name = null;
  }
  // An answer for a law or traject the pane has since left is dropped.
  if (props.lawId === id && props.trajectRef === traject) ownName.value = name;
}, { immediate: true });

// A traject can hold another version of a law, so the cache goes with it.
let cachedFor = props.trajectRef;
watch([rules, () => props.trajectRef], ([list, traject]) => {
  if (traject !== cachedFor) {
    for (const k of Object.keys(elsewhere)) delete elsewhere[k];
    cachedFor = traject;
  }
  for (const rule of list) {
    for (const target of rule.targets?.values() ?? []) {
      if (target.kind !== 'external') continue;
      const k = `${target.lawId}|${target.output}`;
      if (k in elsewhere) continue;
      elsewhere[k] = { loading: true };
      // An answer that arrives after a traject switch belongs to the old one.
      const keep = (v: Elsewhere) => { if (cachedFor === traject) elsewhere[k] = v; };
      fetchLaw(traject, target.lawId).then(
        (entry) => {
          const hit = findOutput(entry?.law, target.output);
          keep({ lawName: entry?.lawName ?? humanize(target.lawId), articleNumber: hit ? String(hit.article.number) : null });
        },
        (e: unknown) => {
          keep({ error: (e instanceof Error ? e.message : String(e)) });
        },
      );
    }
  }
}, { immediate: true });

function destination(target: NavigableTarget): string {
  if (target.kind === 'action') {
    const sameLaw = target.lawId === (props.law?.$id ?? props.lawId);
    if (sameLaw && String(target.articleNumber) === String(props.article?.number)) return 'dit artikel';
    const law = sameLaw ? (ownName.value ?? humanize(props.lawId ?? target.lawId)) : humanize(target.lawId);
    return `${law}, artikel ${target.articleNumber}`;
  }
  const e = elsewhere[`${target.lawId}|${target.output}`];
  if (!e || e.loading) return `${humanize(target.lawId)} (wordt opgezocht…)`;
  if (e.error) return `${humanize(target.lawId)} (niet geladen)`;
  if (!e.articleNumber) return `${e.lawName} (geen rekenregel gevonden)`;
  return `${e.lawName}, artikel ${e.articleNumber}`;
}

// The tooltip of a name: where its value comes from.
function hintsOf(rule: Rule): Map<string, string> {
  const hints = new Map<string, string>();
  const format = (t: { name: string; value: unknown }) => formatLiteral(t.value, rule.unit?.get(t.name)) ?? JSON.stringify(t.value);
  for (const [r, target] of rule.targets ?? []) {
    const text = originHint(r, target, { destination, format, label: humanize });
    if (text) hints.set(r, text);
  }
  return hints;
}

async function follow(seg: Token) {
  navError.value = null;
  // Only a name with a reference can be followed (RegelRijmText's `navigable`).
  if (seg.ref === undefined) return;
  const from = `${props.lawId}|${props.article?.number}`;
  const to = await followReference(seg.ref, {
    law: props.law, lawId: props.lawId, article: props.article, label: humanize,
    load: (id) => fetchLaw(props.trajectRef, id),
  });
  // The editor may have moved to another article while another law loaded.
  if (!to || `${props.lawId}|${props.article?.number}` !== from) return;
  if (to.kind === 'error') navError.value = to.message;
  else if (to.kind === 'navigate') emit('navigate', { lawId: to.lawId, articleNumber: to.articleNumber });
  else {
    const index = actions.value.findIndex((a) => a?.output === to.output);
    document.getElementById(ruleId(String(index)))?.scrollIntoView?.({ behavior: 'smooth', block: 'start' });
  }
}

function ruleId(key: string): string {
  return `regelrijm-pane-${uid}-${key}`;
}
</script>

<template>
  <div data-testid="regelrijm-pane">
    <nldd-title size="5" data-testid="section-actions"><h5>Acties</h5></nldd-title>
    <nldd-banner v-if="navError" variant="warning" :text="navError"></nldd-banner>

    <template v-for="rule in rules" :key="rule.key">
      <nldd-spacer size="24"></nldd-spacer>
      <div :id="ruleId(rule.key)" :data-testid="`regelrijm-pane-rule-${rule.output ?? rule.key}`">
        <nldd-title size="6"><h6>{{ rule.output ? humanize(rule.output) : 'Actie zonder uitkomst' }}</h6></nldd-title>
        <nldd-spacer size="4"></nldd-spacer>
        <nldd-box background="tinted">
          <nldd-container padding="12">
            <nldd-inline-dialog v-if="rule.error" variant="alert" text="Deze regel is nog niet te lezen" :supporting-text="rule.error"></nldd-inline-dialog>
            <RegelRijmText
              v-else-if="rule.reading"
              :segments="rule.reading.segments"
              interactive
              :navigable="rule.navigable"
              :blocks="rule.navigable"
              :selectable="rule.selectable"
              :hints="hintsOf(rule)"
              @follow="follow"
              @select="emit('open-action', rule.action, $event)"
            />
          </nldd-container>
        </nldd-box>
        <template v-if="rule.output && links.get(rule.output)?.length">
          <nldd-spacer size="8"></nldd-spacer>
          <div :data-testid="`regelrijm-pane-text-${rule.output}`">
            <nldd-text size="sm" weight="medium" color="secondary">Uit de wettekst</nldd-text>
            <QuotedFragment v-for="frag in links.get(rule.output)" :key="frag.start" :fragment="frag" />
          </div>
        </template>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-button-group orientation="horizontal">
          <nldd-button size="sm" text="Actie openen" :data-testid="`regelrijm-pane-open-${rule.output ?? rule.key}`" @click="emit('open-action', rule.action)"></nldd-button>
          <RowActionsMenu
            v-if="editable"
            :accessible-label="`Acties voor actie ${rule.output}`"
            :edit-testid="`action-${rule.output}-edit-btn`"
            :delete-testid="`action-${rule.output}-delete-btn`"
            @edit="emit('open-action', rule.action)"
            @delete="emit('delete-action', rule.index)"
          />
        </nldd-button-group>
      </div>
    </template>

    <template v-if="editable">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-button width="full" start-icon="plus-small" data-testid="add-action-btn" text="Actie toevoegen" @click="emit('add-action')"></nldd-button>
    </template>
  </div>
</template>
