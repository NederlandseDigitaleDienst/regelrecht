<script setup lang="ts">
/**
 * TasksCategoriesPane - het categorie-panel van Taken (secondary sidebar van
 * Home). Navigatie, geen inhoud: de gekozen categorie opent zijn takenlijst in
 * main (TasksListPane), zoals Instellingen zijn tabs naar main stuurt.
 *
 * Twee groepen. Bovenaan Prioriteit / Wachten op / Alle taken - die staan er
 * altijd, ook leeg, want ze zijn de vaste ingang tot de lijst. Daaronder de
 * contexten: waar het werk over gaat. Werkdocumenten is er één, en elke wet met
 * open taken staat er plat naast - geen overkoepelende "Wetten"-ingang, want die
 * verzameling voegt niets toe boven de wetten zelf.
 *
 * Contexten zijn volledig data-gedreven, Werkdocumenten net zo goed als de
 * wetten: een context bestaat zolang er werk over ligt en verdwijnt daarna. Ligt
 * er niets meer, dan valt het hele blok weg - inclusief de kop, die anders boven
 * een ingang naar een lege lijst zou hangen. Wetsnamen via de corpuslezer (de
 * payload draagt alleen het rauwe law_id); die valt zelf al terug op
 * humanizeLawId voor een wet buiten het actieve corpus.
 *
 * De aantallen zijn secundaire tekst, geen badge: een badge is hier rood
 * (critical) en dat hoort bij werk dat vandaag af moet of te laat is. Een open
 * taak zonder deadline verdient geen alarm - zie ook TasksSidebarItem, dat die
 * afweging nog niet maakt.
 *
 * `useTasks()` start de gedeelde 30s-poll, maar dit component mount alleen
 * binnen de taken-route (requiresAuth), dus anonieme bezoekers pollen nooit -
 * zelfde afspraak als TasksSidebarItem/TasksListPane. Samen gemount blijft het
 * één interval (consumers-teller in useTasks).
 */
import { computed } from 'vue';
import { useTrajectTasks } from '../composables/useTasks';
import { useCorpus } from '../corpus';
import {
  categoryCounts,
  lawContexts,
  ALLE,
  PRIORITEIT,
  WACHTEN,
  WERKDOCUMENTEN,
  WET,
} from '../lib/taskCategories';

const props = withDefaults(
  defineProps<{
    trajectRef?: string | null;
    categorie?: string | null;
    lawId?: string | null;
  }>(),
  { trajectRef: null, categorie: null, lawId: null },
);

// The `select` payload; `categorie` is the key LibraryView destructures.
interface CategorySelection {
  categorie: string;
  lawId: string | null;
}

const emit = defineEmits<{ select: [selection: CategorySelection] }>();

const { tasks, running } = useTrajectTasks(() => props.trajectRef);
const { displayName } = useCorpus();

const counts = computed(() => categoryCounts(tasks.value, running.value));
const laws = computed(() => lawContexts(tasks.value, running.value, displayName));

// Een context bestaat zolang er werk over ligt - voor Werkdocumenten net zo
// goed als voor de wetten. Ligt er niets, dan is er geen contexten-blok: geen
// kop, geen spacers, geen dode ingang.
const hasWerkdocumenten = computed(() => counts.value[WERKDOCUMENTEN] > 0);
const hasContexten = computed(() => hasWerkdocumenten.value || laws.value.length > 0);

// Een wet-context is alleen geselecteerd bij een exacte match op law_id;
// "Wetten" zelf alleen zonder law_id, anders lichten beide tegelijk op.
function isSelected(categorie: string, lawId: string | null = null): boolean {
  return props.categorie === categorie && (props.lawId ?? null) === lawId;
}

function select(categorie: string, lawId: string | null = null): void {
  emit('select', { categorie, lawId });
}
</script>

<template>
  <!-- Aflopend naar urgentie: eerst wat nu moet, dan waar je op wacht, en
       "Alle taken" als vangnet onderaan - dat is bladeren, geen signaal.
       Prioriteit staat vooraan omdat het panel aandacht stuurt, en omdat de
       badge in de primary sidebar er naar wijst: je oog hoort er als eerste te
       landen. -->
  <!-- Alleen gevulde categorieën: een categorie zonder taken is geen keuze,
       dus verdwijnt hij. Zo is elke ingang die je ziet ook echt ergens goed
       voor - inclusief Prioriteit, dat wegvalt zodra er niets vastzit. -->
  <nldd-list v-if="counts[ALLE] > 0" appearance="simple">
    <nldd-list-item
      v-if="counts[PRIORITEIT] > 0"
      size="md"
      button
      :current="isSelected(PRIORITEIT) || undefined"
      @click="select(PRIORITEIT)"
    >
      <nldd-icon-cell size="20"><nldd-icon icon="exclamation-circle"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell text="Prioriteit"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell
        :text="String(counts[PRIORITEIT])"
        color="secondary"
        width="fit-content"
        horizontal-alignment="right"
      ></nldd-text-cell>
      <nldd-spacer-cell size="2"></nldd-spacer-cell>
      <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
    </nldd-list-item>

    <nldd-list-item
      v-if="counts[WACHTEN] > 0"
      size="md"
      button
      :current="isSelected(WACHTEN) || undefined"
      @click="select(WACHTEN)"
    >
      <nldd-icon-cell size="20"><nldd-icon icon="clock"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell text="Wachten op"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell
        :text="String(counts[WACHTEN])"
        color="secondary"
        width="fit-content"
        horizontal-alignment="right"
      ></nldd-text-cell>
      <nldd-spacer-cell size="2"></nldd-spacer-cell>
      <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
    </nldd-list-item>

    <nldd-list-item
      size="md"
      button
      :current="isSelected(ALLE) || undefined"
      @click="select(ALLE)"
    >
      <nldd-icon-cell size="20"><nldd-icon icon="circle-grid-2x2-top-left-check-mark"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell text="Alle taken"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell
        :text="String(counts[ALLE])"
        color="secondary"
        width="fit-content"
        horizontal-alignment="right"
      ></nldd-text-cell>
      <nldd-spacer-cell size="2"></nldd-spacer-cell>
      <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
    </nldd-list-item>
  </nldd-list>

  <!-- Helemaal niets: geen categorieën om te tonen (counts[ALLE] telt taken én
       lopende jobs, dus de contexten zijn dan ook leeg). -->
  <nldd-inline-dialog v-else text="Geen taken"></nldd-inline-dialog>

  <!-- Het hele blok is data-gedreven: geen werk, geen contexten, geen kop.
       Anders staat er een kop boven een ingang die naar een lege lijst leidt. -->
  <template v-if="hasContexten">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="display-5"><h3>Contexten</h3></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>

    <nldd-list appearance="simple">
      <nldd-list-item
        v-if="hasWerkdocumenten"
        size="md"
        button
        :current="isSelected(WERKDOCUMENTEN) || undefined"
        @click="select(WERKDOCUMENTEN)"
      >
        <nldd-icon-cell size="20"><nldd-icon icon="label"></nldd-icon></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell text="Werkdocumenten"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell
          :text="String(counts[WERKDOCUMENTEN])"
          color="secondary"
          width="fit-content"
          horizontal-alignment="right"
        ></nldd-text-cell>
        <nldd-spacer-cell size="2"></nldd-spacer-cell>
        <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
      </nldd-list-item>

      <!-- Elke wet met open taken is zelf een context, plat naast Werkdocumenten. -->
      <nldd-list-item
        v-for="law in laws"
        :key="law.lawId"
        size="md"
        button
        :current="isSelected(WET, law.lawId) || undefined"
        @click="select(WET, law.lawId)"
      >
        <nldd-icon-cell size="20"><nldd-icon icon="label"></nldd-icon></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="law.name"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell
          :text="String(law.count)"
          color="secondary"
          width="fit-content"
          horizontal-alignment="right"
        ></nldd-text-cell>
        <nldd-spacer-cell size="2"></nldd-spacer-cell>
        <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
      </nldd-list-item>
    </nldd-list>
  </template>
</template>
