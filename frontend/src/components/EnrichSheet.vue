<script setup lang="ts">
/**
 * EnrichSheet - start een verrijking, met zo weinig keuzes als het kan.
 *
 * - **Artikel**: het artikel zelf staat vast aangevinkt. Met de keten (vlag
 *   `enrich.hotspots`) staan eronder een paar suggesties uit de keten: artikelen
 *   die het startartikel raakt en verrijking kunnen gebruiken. Die staan uit;
 *   wie er een aanvinkt maakt de run zelf groter. Plus een optionele instructie.
 * - **Regelwerk**: één zin met wat er gebeurt ("12 van 41 artikelen hebben nog
 *   geen machine_readable; die worden verrijkt"). Zelf kiezen kan onder
 *   "Kies artikelen".
 *
 * De stelselcontext gaat altijd mee: zonder ziet de verrijker niet wie dit
 * regelwerk elders leest. Provider, model en terugkoppelrondes staan onder
 * "Meer opties", met de standaard van de omgeving als ze dicht blijven.
 *
 * Er gebeurt niets met een taalmodel vóór de Start-knop. Gaat er meer dan het
 * startartikel mee, dan gaat er één aanvraag per wet uit (`planChainRequests`)
 * en meldt de sheet het resultaat met `submitted`; anders `submit`.
 *
 * Sheet-opzet (nldd-sheet > nldd-page sticky-header > nldd-top-title-bar,
 * nldd-simple-section, nldd-form) spiegelt AddLawSheet en NewHarvestJobSheet.
 */
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { apiFetchJson } from '../lib/apiFetch';
import { ENRICH_PROVIDER_OPTIONS, providerModels } from '../lib/enrichProviders';
import { planChainRequests, selectionSummary, topArticles } from '../lib/enrichScope';
import {
  DEFAULT_CHAIN_DEPTH,
  articleKey,
  chainUrl,
  chainSuggestions,
  pathText,
  topReason,
} from '../lib/enrichChain';
import type { Chain, ChainNode } from '../lib/enrichChain';
import type { LawDocument } from '../lib/lawFormat';
import { useTaskActions } from '../composables/useTasks';
import type { EnrichOptions, PlanOutcome } from '../composables/useTasks';
import { useCorpus } from '../corpus';

const props = withDefaults(
  defineProps<{
    /** `article` of `law`; `null` = sheet dicht. */
    mode?: 'article' | 'law' | null;
    law?: LawDocument | null;
    lawId?: string | null;
    trajectRef?: string | null;
    /** Het doelartikel in de artikelmodus. */
    article?: string | null;
    /** Terwijl de aanvraag loopt. */
    busy?: boolean;
    /** Suggesties uit de keten in de artikelmodus (vlag `enrich.hotspots`). */
    stepped?: boolean;
  }>(),
  { mode: null, law: null, lawId: null, trajectRef: null, article: null, busy: false, stepped: false },
);
const emit = defineEmits<{
  submit: [options: EnrichOptions];
  /** Meer dan één wet: de sheet heeft zelf aangevraagd; dit is wat ervan kwam. */
  submitted: [outcome: PlanOutcome];
  close: [];
}>();

/** At most this many chain suggestions; the rest is a count. */
const MAX_SUGGESTIONS = 6;

const { displayName } = useCorpus();
const { requestEnrichPlan } = useTaskActions();

const sheetRef = ref<NLDDSheet | null>(null);
const provider = ref('');
const model = ref('');
const rounds = ref<string>('');
const instruction = ref('');
const showOptions = ref(false);
// Law mode.
const showArticles = ref(false);
const onlyMissing = ref(true);
const selected = ref<Set<string>>(new Set());
// Article mode: the chain suggestions and what of them is ticked.
const chain = ref<Chain | null>(null);
const chainLoading = ref(false);
const picked = ref<Set<string>>(new Set());
const sending = ref(false);
const outcome = ref<PlanOutcome | null>(null);

const isArticle = computed(() => props.mode === 'article');
const articles = computed(() => topArticles(props.law));
const models = computed(() => providerModels(provider.value));
const title = computed(() =>
  isArticle.value ? `Verrijk artikel ${props.article ?? ''}` : 'Verrijk dit regelwerk',
);

// --- Law mode ---
const effectiveArticles = computed(() =>
  articles.value
    .filter((a) => selected.value.has(a.number))
    .filter((a) => !(onlyMissing.value && a.hasMachineReadable))
    .map((a) => a.number),
);
const allSelected = computed(() => selected.value.size === articles.value.length);
const missingCount = computed(() => articles.value.filter((a) => !a.hasMachineReadable).length);
const lawSummary = computed(() => {
  const n = effectiveArticles.value.length;
  const total = articles.value.length;
  if (n === 0) return 'Niets te verrijken met deze keuze.';
  // The default choice says why these articles, not only how many.
  if (onlyMissing.value && allSelected.value) {
    const have = n === 1 ? '1 van de' : `${n} van de`;
    return `${have} ${total} artikelen ${n === 1 ? 'heeft' : 'hebben'} nog geen machine_readable. ${n === 1 ? 'Dat wordt' : 'Die worden'} verrijkt.`;
  }
  return n === 1 ? '1 artikel wordt verrijkt.' : `${n} artikelen worden verrijkt.`;
});

// --- Article mode ---
const suggestions = computed(() => (chain.value ? chainSuggestions(chain.value) : []));
const shownSuggestions = computed(() => suggestions.value.slice(0, MAX_SUGGESTIONS));
const hiddenSuggestions = computed(() => suggestions.value.length - shownSuggestions.value.length);
const pickedPerLaw = computed(() => {
  const perLaw = new Map<string, string[]>();
  if (props.lawId && props.article) perLaw.set(props.lawId, [props.article]);
  for (const node of suggestions.value) {
    if (!picked.value.has(articleKey(node.law_id, node.article))) continue;
    perLaw.set(node.law_id, [...(perLaw.get(node.law_id) ?? []), node.article]);
  }
  return perLaw;
});
const pickedCount = computed(() => [...pickedPerLaw.value.values()].reduce((n, l) => n + l.length, 0));

const canSubmit = computed(
  () => !props.busy && !sending.value && (isArticle.value ? !!props.article : effectiveArticles.value.length > 0),
);
const startText = computed(() => {
  if (!isArticle.value) return 'Start';
  if (pickedCount.value <= 1) return 'Start';
  return `Start · ${selectionSummary(pickedCount.value, pickedPerLaw.value.size)}`;
});

function suggestionLabel(node: ChainNode): string {
  return node.law_id === props.lawId ? `Art. ${node.article}` : `${displayName(node.law_id)} art. ${node.article}`;
}
function suggestionReason(node: ChainNode): string {
  if (node.hotspot) return topReason(node.hotspot.reasons, displayName);
  const path = chain.value ? pathText(node, chain.value, displayName) : null;
  return path ?? (node.has_mr ? 'heeft machine_readable' : 'geen machine_readable');
}

// Bumped per load: an answer for a sheet that was closed and opened again
// must not land in the new one.
let chainGeneration = 0;
async function loadChain() {
  if (!props.trajectRef || !props.lawId || !props.article) return;
  const mine = ++chainGeneration;
  chainLoading.value = true;
  try {
    const result = await apiFetchJson<Chain>(
      chainUrl(props.trajectRef, props.lawId, props.article, DEFAULT_CHAIN_DEPTH),
    );
    if (mine === chainGeneration) chain.value = result;
  } catch {
    // Suggestions are extra: without them the article alone is still enriched.
    if (mine === chainGeneration) chain.value = null;
  } finally {
    if (mine === chainGeneration) chainLoading.value = false;
  }
}

function togglePicked(key: string, checked: boolean) {
  const next = new Set(picked.value);
  if (checked) next.add(key);
  else next.delete(key);
  picked.value = next;
}
function toggleArticle(number: string, checked: boolean) {
  const next = new Set(selected.value);
  if (checked) next.add(number);
  else next.delete(number);
  selected.value = next;
}
function selectAll(on: boolean) {
  selected.value = on ? new Set(articles.value.map((a) => a.number)) : new Set();
}

function outcomeLines(result: PlanOutcome): string[] {
  const lines: string[] = [];
  if (result.started.length) lines.push(`Gestart: ${result.started.map(displayName).join(', ')}.`);
  for (const c of result.conflicts) lines.push(`${displayName(c.lawId)}: er loopt al een verrijking.`);
  for (const lawId of result.tooMany) lines.push(`${displayName(lawId)}: te veel verrijkingen tegelijk.`);
  for (const f of result.failed) lines.push(`${displayName(f.lawId)}: ${f.message}`);
  return lines;
}

function reset() {
  provider.value = '';
  model.value = '';
  rounds.value = '';
  instruction.value = '';
  showOptions.value = false;
  showArticles.value = false;
  onlyMissing.value = true;
  selected.value = new Set(articles.value.map((a) => a.number));
  chainGeneration += 1;
  chain.value = null;
  chainLoading.value = false;
  picked.value = new Set();
  outcome.value = null;
}

watch(
  () => props.mode,
  async (mode) => {
    if (!mode) {
      sheetRef.value?.hide?.();
      return;
    }
    reset();
    await nextTick();
    sheetRef.value?.show?.();
    if (mode === 'article' && props.stepped) loadChain();
  },
  { immediate: true },
);

function close() {
  emit('close');
}
function onSheetClose() {
  if (props.mode) emit('close');
}

type ValueEvent = Event & { detail?: { value?: string; checked?: boolean } | null };
const valueOf = (e: Event) =>
  (e as ValueEvent).detail?.value ?? (e.target as HTMLInputElement | null)?.value ?? '';
const checkedOf = (e: Event) => Boolean((e as ValueEvent).detail?.checked);

function onProviderChange(e: Event) {
  provider.value = valueOf(e);
  model.value = '';
}

function commonChoices(): EnrichOptions {
  const parsedRounds = rounds.value === '' ? undefined : Number(rounds.value);
  return {
    provider: provider.value || undefined,
    model: model.value || undefined,
    feedbackRounds: Number.isFinite(parsedRounds) ? parsedRounds : undefined,
  };
}

async function startPlan() {
  if (!props.trajectRef || !props.lawId || !props.article) return;
  const common = commonChoices();
  const requests = planChainRequests(
    { lawId: props.lawId, article: props.article },
    pickedPerLaw.value,
    { provider: common.provider, model: common.model, feedbackRounds: common.feedbackRounds },
    instruction.value,
  );
  sending.value = true;
  try {
    const result = await requestEnrichPlan(props.trajectRef, requests);
    outcome.value = result;
    emit('submitted', result);
    // Everything started: done. A refusal keeps the sheet open on the
    // outcome, so you see which law it was and why.
    if (!result.conflicts.length && !result.tooMany.length && !result.failed.length) close();
  } finally {
    sending.value = false;
  }
}

function onSubmit() {
  if (!canSubmit.value) return;
  if (isArticle.value) {
    if (pickedPerLaw.value.size > 1 || pickedCount.value > 1) {
      startPlan();
      return;
    }
    emit('submit', {
      ...commonChoices(),
      mode: 'article',
      articles: [props.article as string],
      instruction: instruction.value,
      context: true,
    });
    return;
  }
  emit('submit', {
    ...commonChoices(),
    mode: 'law',
    // Everything selected and nothing excluded: no list, so it is the
    // classic enrichment (with the worker's cursor).
    articles: allSelected.value && !onlyMissing.value ? undefined : [...selected.value],
    onlyMissing: onlyMissing.value,
    context: true,
  });
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetRef"
      placement="right"
      width="480px"
      :accessible-label="title"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="title"
          heading-level="2"
          dismiss-text="Annuleer"
          collapse-anchor="enrich-sheet-title"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-title id="enrich-sheet-title" size="display-3" heading-level="2" :text="title"></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-form @submit.prevent="onSubmit">
            <!-- Artikel: het artikel, suggesties uit de keten, een instructie. -->
            <template v-if="isArticle">
              <nldd-list appearance="box-tinted" accessible-label="Wat er verrijkt wordt">
                <nldd-list-item size="md">
                  <nldd-cell>
                    <nldd-checkbox-field :label="`Art. ${article ?? ''}`" checked disabled></nldd-checkbox-field>
                  </nldd-cell>
                  <nldd-text-cell supporting-text="dit artikel"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item
                  v-for="node in shownSuggestions"
                  :key="`s-${node.law_id}#${node.article}`"
                  size="md"
                  data-testid="chain-suggestion"
                >
                  <nldd-cell>
                    <nldd-checkbox-field
                      :label="suggestionLabel(node)"
                      :checked="picked.has(articleKey(node.law_id, node.article)) || undefined"
                      @change="togglePicked(articleKey(node.law_id, node.article), checkedOf($event))"
                    ></nldd-checkbox-field>
                  </nldd-cell>
                  <nldd-text-cell :supporting-text="suggestionReason(node)"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <template v-if="stepped">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-activity-indicator
                  v-if="chainLoading"
                  text="Zoeken naar artikelen die hiermee samenhangen"
                  show-text
                ></nldd-activity-indicator>
                <nldd-text-cell
                  v-else-if="suggestions.length"
                  :supporting-text="hiddenSuggestions
                    ? `Uit de keten van dit artikel; nog ${hiddenSuggestions} andere. Aangevinkt gaat mee.`
                    : 'Uit de keten van dit artikel. Aangevinkt gaat mee.'"
                ></nldd-text-cell>
              </template>
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-form-field label="Instructie" optional>
                <nldd-multi-line-text-field
                  size="md"
                  resize="auto"
                  :value="instruction"
                  maxlength="2000"
                  @input="instruction = valueOf($event)"
                ></nldd-multi-line-text-field>
                <nldd-form-field-help-text>
                  Bijvoorbeeld: "bind het inkomen aan het toetsingsinkomen uit de Awir".
                </nldd-form-field-help-text>
              </nldd-form-field>
            </template>

            <!-- Regelwerk: één zin, en zelf kiezen als je wilt. -->
            <template v-else>
              <nldd-text-cell data-testid="law-summary" :text="lawSummary"></nldd-text-cell>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-switch-field
                label="Zelf artikelen kiezen"
                data-testid="toggle-articles"
                :checked="showArticles || undefined"
                @change="showArticles = checkedOf($event)"
              ></nldd-switch-field>
              <template v-if="showArticles">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-segmented-control
                  size="md"
                  width="full"
                  :value="onlyMissing ? 'missing' : 'all'"
                  accessible-label="Alleen artikelen zonder machine_readable of ook opnieuw"
                  @change="onlyMissing = valueOf($event) === 'missing'"
                >
                  <nldd-segmented-control-item value="missing" :text="`Zonder machine_readable (${missingCount})`"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="all" text="Ook opnieuw"></nldd-segmented-control-item>
                </nldd-segmented-control>
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-list appearance="box-tinted">
                  <nldd-list-item size="md">
                    <nldd-cell>
                      <nldd-checkbox-field
                        label="Alle artikelen"
                        :checked="allSelected || undefined"
                        @change="selectAll(checkedOf($event))"
                      ></nldd-checkbox-field>
                    </nldd-cell>
                  </nldd-list-item>
                  <nldd-list-item v-for="a in articles" :key="a.number" size="md">
                    <nldd-cell>
                      <nldd-checkbox-field
                        :label="`Artikel ${a.number}`"
                        :checked="selected.has(a.number) || undefined"
                        :disabled="(onlyMissing && a.hasMachineReadable) || undefined"
                        @change="toggleArticle(a.number, checkedOf($event))"
                      ></nldd-checkbox-field>
                    </nldd-cell>
                    <nldd-text-cell
                      v-if="a.hasMachineReadable"
                      horizontal-alignment="right"
                      supporting-text="heeft machine_readable"
                    ></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </template>
            </template>

            <!-- Meer opties: dicht = de standaard van de omgeving. -->
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-switch-field
              label="Meer opties"
              data-testid="toggle-options"
              :checked="showOptions || undefined"
              @change="showOptions = checkedOf($event)"
            ></nldd-switch-field>
            <template v-if="showOptions">
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-form-field label="Verrijken met">
                <nldd-dropdown size="md" width="full">
                  <select aria-label="Verrijken met" :value="provider" @change="onProviderChange">
                    <option value="">Standaard van deze omgeving</option>
                    <option v-for="o in ENRICH_PROVIDER_OPTIONS" :key="o.id" :value="o.id">{{ o.label }}</option>
                  </select>
                </nldd-dropdown>
              </nldd-form-field>
              <nldd-form-field v-if="models.length" label="Model" optional>
                <nldd-dropdown size="md" width="full">
                  <select aria-label="Model" :value="model" @change="model = valueOf($event)">
                    <option value="">Standaard</option>
                    <option v-for="m in models" :key="m.id" :value="m.id">{{ m.label }}</option>
                  </select>
                </nldd-dropdown>
              </nldd-form-field>
              <nldd-form-field label="Terugkoppelrondes per controle" optional>
                <nldd-segmented-control
                  size="md"
                  width="full"
                  :value="rounds === '' ? 'default' : rounds"
                  accessible-label="Terugkoppelrondes per controle"
                  @change="rounds = valueOf($event) === 'default' ? '' : valueOf($event)"
                >
                  <nldd-segmented-control-item value="default" text="Standaard"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="0" text="0"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="1" text="1"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="2" text="2"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="3" text="3"></nldd-segmented-control-item>
                </nldd-segmented-control>
              </nldd-form-field>
            </template>

            <template v-if="outcome">
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-inline-dialog
                data-testid="plan-outcome"
                variant="alert"
                text="Niet alles is gestart"
                :supporting-text="outcomeLines(outcome).join(' ')"
              ></nldd-inline-dialog>
            </template>

            <nldd-form-actions>
              <nldd-button-group>
                <nldd-button
                  appearance="accent-filled"
                  data-testid="enrich-start"
                  :text="startText"
                  :disabled="!canSubmit || undefined"
                  :loading="busy || sending || undefined"
                  @click="onSubmit"
                ></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
