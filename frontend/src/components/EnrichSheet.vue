<script setup lang="ts">
/**
 * EnrichSheet - verrijken met controle over wat er gebeurt, in twee modi.
 *
 * - **Artikel** (vanuit het artikelpaneel): één artikel, een optionele
 *   instructie van de engineer, en vooraf zichtbaar wie dit regelwerk elders
 *   leest. Diezelfde stelselcontext gaat mee naar de verrijker, zodat die geen
 *   output hernoemt waar een andere wet op leunt zonder dat iemand het ziet.
 * - **Regelwerk**: de klassieke verrijking van de hele wet, maar met een
 *   selectie van artikelen, de keuze om alleen artikelen zonder
 *   machine_readable te doen, en of de stelselcontext meegaat.
 *
 * Met `stepped` (vlag `enrich.hotspots`) loopt de artikelmodus in drie
 * stappen, en gebeurt er niets met een taalmodel vóór de laatste knop:
 *
 *   1. **Keten**: wat dit artikel raakt, per wet en per sprong, met de gaten
 *      en het pad ernaartoe (`GET …/articles/{art}/chain`).
 *   2. **Kies**: welke artikelen mee gaan. De hotspots op de keten staan al
 *      aangevinkt, met hun reden erbij.
 *   3. **Bevestig**: per wet wat er gebeurt, provider, model en rondes, de
 *      instructie, en een waarschuwing bij een grote run of een wet waar al
 *      een verrijking loopt. Geen plafond: groot is een waarschuwing, geen
 *      weigering.
 *
 * Daarna gaat er één aanvraag per wet uit (`planChainRequests` in
 * lib/enrichScope.ts) en meldt de sheet het resultaat met `submitted`.
 *
 * Beide modi kiezen provider, model en het aantal terugkoppelrondes. Wat
 * mag en niet mag bewaakt de editor-api (`enrich_scope.rs`); deze sheet biedt
 * alleen aan wat daar door de toets komt.
 *
 * Sheet-opzet (nldd-sheet > nldd-page sticky-header > nldd-top-title-bar,
 * nldd-simple-section, nldd-form) spiegelt AddLawSheet en NewHarvestJobSheet.
 */
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { apiFetchJson } from '../lib/apiFetch';
import { ENRICH_PROVIDER_OPTIONS, providerModels } from '../lib/enrichProviders';
import {
  isLargeRun,
  planChainRequests,
  selectionSummary,
  topArticles,
} from '../lib/enrichScope';
import {
  CHAIN_DEPTHS,
  DEFAULT_CHAIN_DEPTH,
  articleKey,
  chainCounts,
  chainUrl,
  gapTags,
  groupChain,
  pathText,
  preselection,
  topReason,
} from '../lib/enrichChain';
import type { Chain, ChainNode } from '../lib/enrichChain';
import type { LawDocument } from '../lib/lawFormat';
import { sameTraject } from '../lib/taskReview';
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
    /** De artikelmodus in drie stappen, met de keten (vlag `enrich.hotspots`). */
    stepped?: boolean;
  }>(),
  { mode: null, law: null, lawId: null, trajectRef: null, article: null, busy: false, stepped: false },
);
const emit = defineEmits<{
  submit: [options: EnrichOptions];
  /** De stappen-flow heeft zelf aangevraagd; dit is wat ervan kwam. */
  submitted: [outcome: PlanOutcome];
  close: [];
}>();

interface ConsumerRow {
  law_id: string;
  article: string;
  target: string;
  kind: 'source_output' | 'source_article' | 'implements';
}

const { displayName } = useCorpus();
const { requestEnrichPlan, running } = useTaskActions();

const sheetRef = ref<NLDDSheet | null>(null);
const provider = ref('');
const model = ref('');
const rounds = ref<string>('');
const instruction = ref('');
const onlyMissing = ref(true);
const withContext = ref(true);
const selected = ref<Set<string>>(new Set());
const consumers = ref<ConsumerRow[] | null>(null);
const consumersPartial = ref(false);
const consumersError = ref(false);

const isArticle = computed(() => props.mode === 'article');
const isStepped = computed(() => props.stepped && isArticle.value);
const articles = computed(() => topArticles(props.law));
const models = computed(() => providerModels(provider.value));
const title = computed(() =>
  isArticle.value ? `Verrijk artikel ${props.article ?? ''}` : 'Verrijk dit regelwerk',
);

// Wat er in de wetmodus echt verrijkt wordt: de selectie, minus wat al
// machine_readable heeft als die keuze aan staat. De server rekent hetzelfde
// uit; hier om vooraf te kunnen zeggen hoeveel het er zijn.
const effectiveArticles = computed(() =>
  articles.value
    .filter((a) => selected.value.has(a.number))
    .filter((a) => !(onlyMissing.value && a.hasMachineReadable))
    .map((a) => a.number),
);
const allSelected = computed(() => selected.value.size === articles.value.length);
const canSubmit = computed(
  () => !props.busy && (isArticle.value ? !!props.article : effectiveArticles.value.length > 0),
);
const lawSummary = computed(() => {
  const n = effectiveArticles.value.length;
  if (n === 0) return 'Niets te verrijken met deze keuze.';
  return n === 1 ? '1 artikel wordt verrijkt.' : `${n} artikelen worden verrijkt.`;
});

const consumersByTarget = computed(() => {
  const groups = new Map<string, ConsumerRow[]>();
  for (const c of consumers.value ?? []) {
    if (c.kind === 'implements') continue;
    groups.set(c.target, [...(groups.get(c.target) ?? []), c]);
  }
  return [...groups.entries()];
});
const implementors = computed(() => (consumers.value ?? []).filter((c) => c.kind === 'implements'));

// --- De stappen ---
const STEP_COUNT = 3;
const step = ref(1);
const depth = ref<number>(DEFAULT_CHAIN_DEPTH);
const chain = ref<Chain | null>(null);
const chainLoading = ref(false);
const chainError = ref<string | null>(null);
// Wat er mee gaat, als `"<wet>#<artikel>"`; het startartikel staat er altijd in.
const picked = ref<Set<string>>(new Set());
const sending = ref(false);
const outcome = ref<PlanOutcome | null>(null);

const rootKey = computed(() => (props.lawId && props.article ? articleKey(props.lawId, props.article) : ''));
const groups = computed(() => (chain.value ? groupChain(chain.value) : []));
const counts = computed(() => (chain.value ? chainCounts(chain.value) : null));
const stepText = computed(() => `Stap ${step.value} van ${STEP_COUNT}`);

// The chain's nodes per law, in chain order, for the choice and the requests.
const pickedPerLaw = computed(() => {
  const perLaw = new Map<string, string[]>();
  for (const node of chain.value?.nodes ?? []) {
    if (!picked.value.has(articleKey(node.law_id, node.article))) continue;
    perLaw.set(node.law_id, [...(perLaw.get(node.law_id) ?? []), node.article]);
  }
  return perLaw;
});
const pickedSummary = computed(() => {
  const articlesCount = [...pickedPerLaw.value.values()].reduce((n, list) => n + list.length, 0);
  return selectionSummary(articlesCount, pickedPerLaw.value.size);
});
const requests = computed(() => {
  if (!props.lawId || !props.article) return [];
  const parsedRounds = rounds.value === '' ? undefined : Number(rounds.value);
  return planChainRequests(
    { lawId: props.lawId, article: props.article },
    pickedPerLaw.value,
    {
      provider: provider.value || undefined,
      model: model.value || undefined,
      feedbackRounds: Number.isFinite(parsedRounds) ? parsedRounds : undefined,
    },
    instruction.value,
  );
});
const largeRun = computed(() => isLargeRun(requests.value));
const requestArticleCount = computed(() =>
  requests.value.reduce((n, r) => n + (r.options.articles?.length ?? 0), 0),
);
// Laws where an enrichment of yours already runs in this traject: the
// server answers those with a 409. Someone else's run shows up only then.
const busyLaws = computed(() => {
  const laws = new Set(requests.value.map((r) => r.lawId));
  return [
    ...new Set(
      running.value
        .filter((j) => j.job_type === 'enrich' && laws.has(j.law_id) && sameTraject(j.traject_ref, props.trajectRef))
        .map((j) => j.law_id),
    ),
  ];
});

function articlesText(n: number): string {
  return n === 1 ? '1 artikel' : `${n} artikelen`;
}
function nodeText(node: ChainNode): string {
  return `Artikel ${node.article}`;
}
function nodeSupporting(node: ChainNode): string {
  if (!chain.value) return '';
  const path = pathText(node, chain.value, displayName) ?? 'Startartikel';
  const status = node.has_mr ? 'heeft machine_readable' : 'geen machine_readable';
  return `${path} · ${status}`;
}
function groupTitle(lawId: string, groupDepth: number): string {
  const jumps = groupDepth === 0 ? 'deze wet' : groupDepth === 1 ? '1 sprong' : `${groupDepth} sprongen`;
  return `${displayName(lawId)} · ${jumps}`;
}
function hotspotReason(node: ChainNode): string | undefined {
  return node.hotspot ? topReason(node.hotspot.reasons, displayName) : undefined;
}

async function loadChain() {
  if (!props.trajectRef || !props.lawId || !props.article) return;
  chainLoading.value = true;
  chainError.value = null;
  try {
    const result = await apiFetchJson<Chain>(chainUrl(props.trajectRef, props.lawId, props.article, depth.value));
    chain.value = result;
    picked.value = preselection(result);
  } catch {
    chainError.value = 'De keten kon niet worden opgehaald.';
    chain.value = null;
  } finally {
    chainLoading.value = false;
  }
}

function setDepth(value: string) {
  const next = Number(value);
  if (!Number.isFinite(next) || next === depth.value) return;
  depth.value = next;
  loadChain();
}

function togglePicked(key: string, checked: boolean) {
  if (key === rootKey.value) return;
  const next = new Set(picked.value);
  if (checked) next.add(key);
  else next.delete(key);
  picked.value = next;
}
// Per law: everything of it on the chain, or only what has no model yet.
function pickLaw(lawId: string, onlyWithoutModel: boolean) {
  const next = new Set(picked.value);
  for (const node of chain.value?.nodes ?? []) {
    if (node.law_id !== lawId) continue;
    const key = articleKey(node.law_id, node.article);
    if (key === rootKey.value) continue;
    if (!onlyWithoutModel || !node.has_mr) next.add(key);
    else next.delete(key);
  }
  picked.value = next;
}

function nextStep() {
  if (step.value < STEP_COUNT) step.value += 1;
}
function previousStep() {
  if (step.value > 1) step.value -= 1;
}

async function startPlan() {
  if (!props.trajectRef || sending.value || requests.value.length === 0) return;
  sending.value = true;
  try {
    const result = await requestEnrichPlan(props.trajectRef, requests.value);
    outcome.value = result;
    emit('submitted', result);
    // Everything started: done. A refusal keeps the sheet open on the
    // outcome, so you see which law it was and why.
    if (!result.conflicts.length && !result.tooMany.length && !result.failed.length) close();
  } finally {
    sending.value = false;
  }
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
  onlyMissing.value = true;
  withContext.value = isArticle.value;
  selected.value = new Set(articles.value.map((a) => a.number));
  consumers.value = null;
  consumersPartial.value = false;
  consumersError.value = false;
  step.value = 1;
  depth.value = DEFAULT_CHAIN_DEPTH;
  chain.value = null;
  chainError.value = null;
  picked.value = new Set(rootKey.value ? [rootKey.value] : []);
  outcome.value = null;
}

async function loadConsumers() {
  if (!props.trajectRef || !props.lawId) return;
  try {
    const res = await apiFetchJson<{ consumers: ConsumerRow[]; partial: boolean }>(
      `/api/trajects/${encodeURIComponent(props.trajectRef)}/corpus/laws/${encodeURIComponent(props.lawId)}/consumers`,
    );
    consumers.value = res.consumers;
    consumersPartial.value = res.partial;
  } catch {
    consumersError.value = true;
    consumers.value = [];
  }
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
    // Alleen waar het iets zegt: in de artikelmodus vooraf, in de wetmodus
    // zodra de stelselcontext aan gaat (zie watch hieronder).
    if (mode === 'article') loadConsumers();
    if (isStepped.value) loadChain();
  },
  { immediate: true },
);
watch(withContext, (on) => {
  if (on && consumers.value === null && props.mode) loadConsumers();
});

function close() {
  emit('close');
}
function onSheetClose() {
  if (props.mode) emit('close');
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

type ValueEvent = Event & { detail?: { value?: string; checked?: boolean } | null };
const valueOf = (e: Event) =>
  (e as ValueEvent).detail?.value ?? (e.target as HTMLInputElement | null)?.value ?? '';
const checkedOf = (e: Event) => Boolean((e as ValueEvent).detail?.checked);

function onProviderChange(e: Event) {
  provider.value = valueOf(e);
  model.value = '';
}

function onSubmit() {
  if (isStepped.value) {
    if (step.value < STEP_COUNT) nextStep();
    else startPlan();
    return;
  }
  if (!canSubmit.value) return;
  const parsedRounds = rounds.value === '' ? undefined : Number(rounds.value);
  const common: EnrichOptions = {
    provider: provider.value || undefined,
    model: model.value || undefined,
    feedbackRounds: Number.isFinite(parsedRounds) ? parsedRounds : undefined,
  };
  if (isArticle.value) {
    emit('submit', {
      ...common,
      mode: 'article',
      articles: [props.article as string],
      instruction: instruction.value,
      context: withContext.value,
    });
  } else {
    emit('submit', {
      ...common,
      mode: 'law',
      // Alles geselecteerd en niets uitgesloten: geen lijst, dan is het de
      // klassieke verrijking (met de cursor van de worker).
      articles: allSelected.value && !onlyMissing.value ? undefined : [...selected.value],
      onlyMissing: onlyMissing.value,
      context: withContext.value,
    });
  }
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
          :supporting-text="isStepped ? stepText : undefined"
          heading-level="2"
          dismiss-text="Annuleer"
          collapse-anchor="enrich-sheet-title"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-title
            id="enrich-sheet-title"
            size="display-3"
            heading-level="2"
            :text="title"
            :supporting-text="isStepped ? stepText : undefined"
          ></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-form @submit.prevent="onSubmit">
            <!-- De artikelmodus in stappen: keten, kiezen, bevestigen. -->
            <template v-if="isStepped">
              <!-- Stap 1: wat dit artikel raakt. -->
              <template v-if="step === 1">
                <nldd-form-field label="Hoe ver de keten gaat">
                  <nldd-segmented-control
                    size="md"
                    width="full"
                    :value="String(depth)"
                    accessible-label="Aantal sprongen naar andere wetten"
                    @change="setDepth(valueOf($event))"
                  >
                    <nldd-segmented-control-item
                      v-for="d in CHAIN_DEPTHS"
                      :key="d.value"
                      :value="String(d.value)"
                      :text="d.label"
                    ></nldd-segmented-control-item>
                  </nldd-segmented-control>
                  <nldd-form-field-help-text>
                    Sprongen naar andere wetten. Binnen een wet volgt de keten elke verwijzing en
                    binding; een begripsbepaling is een eindpunt.
                  </nldd-form-field-help-text>
                </nldd-form-field>
                <nldd-spacer size="16"></nldd-spacer>

                <nldd-activity-indicator v-if="chainLoading" text="Keten laden" show-text></nldd-activity-indicator>
                <nldd-inline-dialog v-else-if="chainError" variant="alert" :text="chainError">
                  <nldd-button slot="actions" appearance="secondary" size="md" text="Probeer opnieuw" @click="loadChain"></nldd-button>
                </nldd-inline-dialog>
                <template v-else-if="chain && counts">
                  <nldd-text-cell
                    :text="selectionSummary(counts.articles, counts.laws)"
                    :supporting-text="`${counts.missing} zonder machine_readable · ${counts.hotspots} kunnen verrijking gebruiken`"
                  ></nldd-text-cell>
                  <template v-if="chain.partial.unread_laws || chain.partial.depth_capped || chain.partial.node_capped">
                    <nldd-spacer size="16"></nldd-spacer>
                    <nldd-inline-dialog
                      data-testid="chain-partial"
                      text="De keten is niet compleet"
                      :supporting-text="[
                        chain.partial.unread_laws ? `${chain.partial.unread_laws} wetten konden nog niet gelezen worden.` : '',
                        chain.partial.depth_capped ? 'Voorbij deze diepte gaat de keten verder.' : '',
                        chain.partial.node_capped ? 'De keten is afgekapt op 2000 artikelen.' : '',
                      ].filter(Boolean).join(' ')"
                    >
                      <nldd-button slot="actions" appearance="secondary" size="md" text="Haal opnieuw op" @click="loadChain"></nldd-button>
                    </nldd-inline-dialog>
                  </template>
                  <template v-for="group in groups" :key="group.key">
                    <nldd-spacer size="24"></nldd-spacer>
                    <nldd-form-section :text="groupTitle(group.lawId, group.depth)">
                    <nldd-list appearance="box-tinted">
                      <nldd-list-item v-for="node in group.nodes" :key="`${node.law_id}#${node.article}`" size="md">
                        <nldd-text-cell :supporting-text="nodeSupporting(node)">
                          {{ nodeText(node) }}
                          <nldd-tag
                            v-for="tag in gapTags(node.gaps)"
                            :key="tag"
                            size="sm"
                            color="warning"
                          >{{ tag }}</nldd-tag>
                        </nldd-text-cell>
                      </nldd-list-item>
                    </nldd-list>
                    </nldd-form-section>
                  </template>
                </template>
                <nldd-spacer size="24"></nldd-spacer>

                <nldd-form-section text="Wie leest dit regelwerk elders">
                  <nldd-text-cell v-if="consumers === null" text="Stelselcontext laden…"></nldd-text-cell>
                  <nldd-inline-dialog
                    v-else-if="consumersError"
                    variant="alert"
                    text="De stelselcontext kon niet worden opgehaald."
                    supporting-text="De verrijking kan wel; de verrijker ziet dan alleen deze wet."
                  ></nldd-inline-dialog>
                  <template v-else>
                    <nldd-list v-if="consumersByTarget.length || implementors.length" appearance="box-tinted">
                      <nldd-list-item v-for="[target, rows] in consumersByTarget" :key="`c-${target}`" size="md">
                        <nldd-text-cell
                          :text="target"
                          :supporting-text="rows.map((r) => `${displayName(r.law_id)} art. ${r.article}`).join(', ')"
                        ></nldd-text-cell>
                      </nldd-list-item>
                      <nldd-list-item v-for="row in implementors" :key="`i-${row.law_id}-${row.article}-${row.target}`" size="md">
                        <nldd-text-cell
                          :text="`Open term ${row.target}`"
                          :supporting-text="`ingevuld door ${displayName(row.law_id)} art. ${row.article}`"
                        ></nldd-text-cell>
                      </nldd-list-item>
                    </nldd-list>
                    <nldd-text-cell v-else text="Geen artikel elders in dit traject leest dit regelwerk."></nldd-text-cell>
                  </template>
                </nldd-form-section>
              </template>

              <!-- Stap 2: wat er mee gaat. -->
              <template v-else-if="step === 2">
                <nldd-text-cell :text="pickedSummary" supporting-text="Wat kan verrijking gebruiken staat al aangevinkt."></nldd-text-cell>
                <template v-for="group in groups" :key="`pick-${group.key}`">
                  <nldd-spacer size="24"></nldd-spacer>
                  <nldd-form-section :text="groupTitle(group.lawId, group.depth)">
                  <nldd-button-group>
                    <nldd-button appearance="secondary" size="sm" text="Alles in deze wet" @click="pickLaw(group.lawId, false)"></nldd-button>
                    <nldd-button appearance="secondary" size="sm" text="Alleen zonder machine_readable" @click="pickLaw(group.lawId, true)"></nldd-button>
                  </nldd-button-group>
                  <nldd-spacer size="8"></nldd-spacer>
                  <nldd-list appearance="box-tinted">
                    <nldd-list-item v-for="node in group.nodes" :key="`p-${node.law_id}#${node.article}`" size="md">
                      <nldd-cell>
                        <nldd-checkbox-field
                          :label="nodeText(node)"
                          :checked="picked.has(articleKey(node.law_id, node.article)) || undefined"
                          :disabled="articleKey(node.law_id, node.article) === rootKey || undefined"
                          @change="togglePicked(articleKey(node.law_id, node.article), checkedOf($event))"
                        ></nldd-checkbox-field>
                      </nldd-cell>
                      <nldd-text-cell :supporting-text="hotspotReason(node) ?? (node.has_mr ? 'heeft machine_readable' : 'geen machine_readable')"></nldd-text-cell>
                    </nldd-list-item>
                  </nldd-list>
                  </nldd-form-section>
                </template>
              </template>

              <!-- Stap 3: wat er gebeurt, en waarmee. -->
              <template v-else>
                <nldd-list appearance="box-tinted" accessible-label="Wat er per wet gebeurt">
                  <nldd-list-item v-for="r in requests" :key="`r-${r.lawId}`" size="md">
                    <nldd-text-cell
                      :text="displayName(r.lawId)"
                      :supporting-text="`${r.options.mode === 'article' ? 'Artikelmodus' : 'Wetmodus'} · ${articlesText(r.options.articles?.length ?? 0)}`"
                    ></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
                <nldd-spacer size="16"></nldd-spacer>
                <template v-if="largeRun">
                  <nldd-inline-dialog
                    data-testid="large-run"
                    icon="exclamation-triangle"
                    icon-color="warning"
                    :text="`Dit is een grote run: ${selectionSummary(requestArticleCount, requests.length)}`"
                    supporting-text="Elke wet wordt een eigen verrijking met eigen voorstellen. Dat kost tijd en tokens; je kunt ze stoppen zolang ze lopen."
                  ></nldd-inline-dialog>
                  <nldd-spacer size="16"></nldd-spacer>
                </template>
                <template v-if="busyLaws.length">
                  <nldd-inline-dialog
                    data-testid="busy-laws"
                    icon="exclamation-triangle"
                    icon-color="warning"
                    text="Hier loopt al een verrijking"
                    :supporting-text="`${busyLaws.map(displayName).join(', ')}: die aanvraag wordt geweigerd tot de lopende klaar is.`"
                  ></nldd-inline-dialog>
                  <nldd-spacer size="16"></nldd-spacer>
                </template>
                <template v-if="outcome">
                  <nldd-inline-dialog
                    data-testid="plan-outcome"
                    :variant="outcome.failed.length || outcome.conflicts.length || outcome.tooMany.length ? 'alert' : 'success'"
                    text="Zo ging het versturen"
                    :supporting-text="outcomeLines(outcome).join(' ')"
                  ></nldd-inline-dialog>
                  <nldd-spacer size="16"></nldd-spacer>
                </template>
                <nldd-form-field label="Instructie voor de verrijker" optional>
                  <nldd-multi-line-text-field
                    size="md"
                    resize="auto"
                    :value="instruction"
                    maxlength="2000"
                    @input="instruction = valueOf($event)"
                  ></nldd-multi-line-text-field>
                  <nldd-form-field-help-text>
                    Gaat mee met de verrijking van {{ displayName(lawId ?? '') }}. De andere wetten
                    worden in de wetmodus verrijkt, zonder instructie.
                  </nldd-form-field-help-text>
                </nldd-form-field>
              </template>
            </template>

            <!-- Artikelmodus in één keer: wie leest dit regelwerk, en wat moet de verrijker weten. -->
            <template v-else-if="isArticle">
              <nldd-rich-text>
                <p>
                  De verrijker schrijft machine_readable voor dit artikel. Artikelen in deze wet die
                  er direct van afhangen mag hij meenemen; elk daarvan krijg je als eigen onderdeel
                  ter beoordeling. Problemen in andere wetten komen terug als signaal, niet als
                  wijziging.
                </p>
              </nldd-rich-text>
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-form-section text="Wie leest dit regelwerk elders">
                <nldd-text-cell v-if="consumers === null" text="Stelselcontext laden…"></nldd-text-cell>
                <nldd-inline-dialog
                  v-else-if="consumersError"
                  variant="alert"
                  text="De stelselcontext kon niet worden opgehaald."
                  supporting-text="De verrijking kan wel; de verrijker ziet dan alleen deze wet."
                ></nldd-inline-dialog>
                <template v-else>
                  <nldd-list v-if="consumersByTarget.length || implementors.length" appearance="box-tinted">
                    <nldd-list-item v-for="[target, rows] in consumersByTarget" :key="`c-${target}`" size="md">
                      <nldd-text-cell
                        :text="target"
                        :supporting-text="rows.map((r) => `${r.law_id} art. ${r.article}`).join(', ')"
                      ></nldd-text-cell>
                    </nldd-list-item>
                    <nldd-list-item v-for="row in implementors" :key="`i-${row.law_id}-${row.article}-${row.target}`" size="md">
                      <nldd-text-cell
                        :text="`Open term ${row.target}`"
                        :supporting-text="`ingevuld door ${row.law_id} art. ${row.article}`"
                      ></nldd-text-cell>
                    </nldd-list-item>
                  </nldd-list>
                  <nldd-text-cell v-else text="Geen artikel elders in dit traject leest dit regelwerk."></nldd-text-cell>
                  <nldd-text-cell
                    v-if="consumersPartial"
                    supporting-text="Nog niet alle wetten zijn gelezen; er kunnen meer lezers zijn."
                  ></nldd-text-cell>
                </template>
              </nldd-form-section>
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-form-field label="Instructie voor de verrijker" optional>
                <nldd-multi-line-text-field
                  size="md"
                  resize="auto"
                  :value="instruction"
                  maxlength="2000"
                  @input="instruction = valueOf($event)"
                ></nldd-multi-line-text-field>
                <nldd-form-field-help-text>
                  Bijvoorbeeld: "bind het inkomen aan het toetsingsinkomen uit de Awir". De verrijker
                  volgt dit alleen waar de wettekst het draagt; anders meldt hij waarom niet.
                </nldd-form-field-help-text>
              </nldd-form-field>
            </template>

            <!-- Wetmodus: welke artikelen. -->
            <template v-else>
              <nldd-form-field label="Welke artikelen">
                <nldd-segmented-control
                  size="md"
                  width="full"
                  :value="onlyMissing ? 'missing' : 'all'"
                  accessible-label="Alleen artikelen zonder machine_readable of ook opnieuw"
                  @change="onlyMissing = valueOf($event) === 'missing'"
                >
                  <nldd-segmented-control-item value="missing" text="Alleen zonder machine_readable"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="all" text="Ook opnieuw"></nldd-segmented-control-item>
                </nldd-segmented-control>
              </nldd-form-field>

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
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-text-cell :text="lawSummary"></nldd-text-cell>
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-form-field>
                <nldd-switch-field
                  label="Stelselcontext meesturen"
                  :checked="withContext || undefined"
                  @change="withContext = checkedOf($event)"
                ></nldd-switch-field>
                <nldd-form-field-help-text>
                  Wie dit regelwerk elders leest, welke open termen het heeft en wie die invult, en
                  de artikelen uit andere wetten waar het van afhangt.
                </nldd-form-field-help-text>
              </nldd-form-field>
            </template>

            <!-- Waarop en hoe grondig: in de stappen pas bij het bevestigen. -->
            <template v-if="!isStepped || step === STEP_COUNT">
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
                <nldd-form-field-help-text>
                  Meer rondes geven de verrijker vaker de kans een bevinding te herstellen, maar delen
                  dezelfde tijd: de vertaling zelf krijgt dan minder.
                </nldd-form-field-help-text>
              </nldd-form-field>
            </template>

            <nldd-form-actions v-if="isStepped">
              <nldd-button-group>
                <nldd-button
                  v-if="step > 1"
                  appearance="secondary"
                  text="Terug"
                  data-testid="step-back"
                  @click="previousStep"
                ></nldd-button>
                <nldd-button
                  v-if="step < STEP_COUNT"
                  appearance="accent-filled"
                  text="Volgende"
                  data-testid="step-next"
                  :disabled="(step === 1 && !chain) || undefined"
                  @click="nextStep"
                ></nldd-button>
                <nldd-button
                  v-else
                  appearance="accent-filled"
                  data-testid="step-start"
                  :text="requests.length > 1 ? `Start ${requests.length} verrijkingen` : 'Start verrijking'"
                  :loading="sending || undefined"
                  @click="startPlan"
                ></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
            <nldd-form-actions v-else>
              <nldd-button-group orientation="vertical">
                <nldd-button
                  appearance="accent-filled"
                  :text="isArticle ? 'Verrijk dit artikel' : 'Verrijk'"
                  :disabled="!canSubmit || undefined"
                  :loading="busy || undefined"
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
