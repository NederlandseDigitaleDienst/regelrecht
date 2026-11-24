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
import { topArticles } from '../lib/enrichScope';
import type { LawDocument } from '../lib/lawFormat';
import type { EnrichOptions } from '../composables/useTasks';

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
  }>(),
  { mode: null, law: null, lawId: null, trajectRef: null, article: null, busy: false },
);
const emit = defineEmits<{
  submit: [options: EnrichOptions];
  close: [];
}>();

interface ConsumerRow {
  law_id: string;
  article: string;
  target: string;
  kind: 'source_output' | 'source_article' | 'implements';
}

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
          heading-level="2"
          dismiss-text="Annuleer"
          collapse-anchor="enrich-sheet-title"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-title id="enrich-sheet-title" size="display-3" heading-level="2" :text="title"></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-form @submit.prevent="onSubmit">
            <!-- Artikelmodus: wie leest dit regelwerk, en wat moet de verrijker weten. -->
            <template v-if="isArticle">
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

            <!-- Beide modi: waarop en hoe grondig. -->
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

            <nldd-form-actions>
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
