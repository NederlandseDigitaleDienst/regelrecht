<script setup lang="ts">
/**
 * TasksSidebarItem - het "Taken"-item in de primary sidebar van Home, met
 * badge. Eigen component zodat `useTasks()` (start de gedeelde 30s-poll bij de
 * eerste aanroep) pas gebeurt wanneer dit item daadwerkelijk gemount wordt -
 * LibraryView rendert het alleen binnen een actief traject (een ingelogde
 * context), dus anonieme bezoekers pollen /api/tasks nooit. Zelfde
 * consumers-teller-patroon als TasksCategoriesPane/TasksListPane: samen gemount
 * blijft het één poll-interval.
 *
 * De badge telt de PRIORITEIT-categorie, niet alle open taken: rood is een
 * alarm, en dat hoort alleen bij werk dat nu vastzit of vandaag af moet. Een
 * stapel taken die kan wachten hoort daar niet toe - die zie je in het panel.
 * Gevolg: open taken zonder prioriteit geven hier géén badge, dat is de
 * bedoeling.
 */
import { computed } from 'vue';
import { useTrajectTasks } from '../composables/useTasks';
import { isPrioriteit } from '../lib/taskCategories';

const props = withDefaults(
  defineProps<{ current?: boolean; trajectRef?: string | null }>(),
  { current: false, trajectRef: null },
);

// Only this traject's tasks: the badge points at the panel, which shows no others.
const { tasks } = useTrajectTasks(() => props.trajectRef);

// Uit de lijst geteld, niet uit `open_count`: dat is het servertotaal van álle
// open taken en zegt niets over prioriteit. De lijst is server-side op 100
// begrensd (tasks.rs:101), dus dit telt in theorie te laag bij >100 open taken.
// Geen prioriteit = geen badge: een lopende taak (Wachten op) is geen alarm en
// krijgt hier dus ook geen stille stip meer.
const prioriteitCount = computed(() => tasks.value.filter(isPrioriteit).length);
</script>

<template>
  <nldd-list-item size="md" button :current="current || undefined">
    <nldd-icon-cell size="20"><nldd-icon icon="tasks"></nldd-icon></nldd-icon-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell text="Taken"></nldd-text-cell>
    <template v-if="prioriteitCount > 0">
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-cell>
        <nldd-badge size="sm" :number="prioriteitCount" accessible-label="Taken met prioriteit"></nldd-badge>
      </nldd-cell>
      <nldd-spacer-cell size="2"></nldd-spacer-cell>
    </template>
    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
  </nldd-list-item>
</template>
