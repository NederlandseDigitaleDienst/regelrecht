<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { useTrajects } from '../composables/useTrajects';
import type { TrajectSummary } from '../composables/useTrajects';
import type { OpenTab } from '../lib/openTabsStorage';
import { useAuth } from '../composables/useAuth';
import { useLoginToChooser } from '../composables/useLoginToChooser';
import { trajectSwitchTarget } from '../composables/useLastVisitedRoute';
import { useAppChrome } from '../composables/useAppChrome';
import TrajectCreateForm from './TrajectCreateForm.vue';

// Mobiele (sm) samenvoeging van de traject-knop en de artikel-tabbladen-knop:
// één full-width knop die een bottom-sheet opent met een "Traject"-lijst (de
// menu-acties + traject-switcher + nieuw traject) en - in de editor - een
// "Artikelen"-lijst (de open tabbladen). md/lg houden TrajectMenu + de
// document-tab-bar. Hergebruikt dezelfde composables/handlers als TrajectMenu.
const { trajects, activeTrajectRef, activeTraject, loading, createTraject } = useTrajects();
const { authenticated } = useAuth();
const { documentTabs, activeDocumentTab, documentTabsTrajectRef, tabActions } = useAppChrome();
const route = useRoute();
const router = useRouter();

// Not logged in: log in, then land on the trajectchooser carrying the current
// section + law/article (shared composable, see also TrajectMenu).
const loginToChooser = useLoginToChooser();

const sheetEl = ref<NLDDSheet | null>(null);
const sheetMode = ref<'list' | 'create'>('list'); // 'list' | 'create'

// Editor = de editor-view publiceert open tabbladen via useAppChrome.
const hasArticles = computed(() => documentTabs.value.length > 0 && !!tabActions.value);

// --- Knoptekst: in de editor het actieve artikel · wet, met het traject als
//     ondersteunende tekst; in de bibliotheek de trajectnaam; anders Trajecten.
const buttonText = computed(() => {
  const tab = activeDocumentTab.value;
  if (tab && tabActions.value) {
    return `Artikel ${tab.articleNumber} · ${tabActions.value.displayName(tab)}`;
  }
  if (activeTraject.value) return activeTraject.value.name;
  // No traject in the URL (Corpus juris, or still loading): the neutral label.
  return 'Trajecten';
});
// A traject in the URL whose name hasn't resolved yet: the trigger shows a
// spinner (see :loading) with the neutral 'Trajecten' label, not a '…'
// placeholder. Without a traject in the URL there is nothing to load.
const menuLoading = computed(
  () => !!activeTrajectRef.value && loading.value && !activeTraject.value,
);
const buttonSupporting = computed(() =>
  activeDocumentTab.value && activeTraject.value ? activeTraject.value.name : undefined,
);

const sheetTitle = computed(() => {
  if (sheetMode.value === 'create') return 'Nieuw traject';
  if (authenticated.value && activeTraject.value) return activeTraject.value.name;
  return 'Trajecten';
});

function openSheet(): void {
  sheetMode.value = 'list';
  sheetEl.value?.show();
}
function closeSheet(): void {
  sheetEl.value?.hide();
}
// Reset naar de lijst zodat een volgende opening niet in create-modus blijft.
function onSheetClose(): void {
  sheetMode.value = 'list';
}

// --- Navigatie naar traject (bibliotheek vs editor) ---
// Een traject-wissel landt op de root van de sectie waar je in zit: de wet die
// je bekeek gaat niet mee (het nieuwe traject heeft z'n eigen corpus), zodat we
// geen document proberen te openen dat daar niet bestaat. De editor toont de
// eigen opgeslagen tabbladen van het nieuwe traject in de balk zonder er een te
// openen. Zie trajectSwitchTarget.
async function goToTraject(trajectRef: string): Promise<void> {
  await router.push(trajectSwitchTarget(route.name, trajectRef));
}

async function selectTraject(t: TrajectSummary): Promise<void> {
  if (!t.ref) {
    console.warn('MobileTrajectSheet: traject has no ref', t);
    return;
  }
  closeSheet();
  if (t.ref === activeTrajectRef.value) return;
  await goToTraject(t.ref);
}

// --- Traject-acties (sluiten dan openen - geen sheet-over-sheet) ---
function openDocuments(): void {
  closeSheet();
  if (activeTrajectRef.value) {
    router.push({ name: 'werkdocumenten-traject', params: { trajectRef: activeTrajectRef.value } });
  }
}

function openTaken(): void {
  closeSheet();
  if (activeTrajectRef.value) {
    router.push({ name: 'taken-traject', params: { trajectRef: activeTrajectRef.value } });
  }
}

// Open the active traject's settings (details / leden) in Home.
// The template calls it without a tab, which opens the settings' default tab.
function goToInstellingen(tab?: string): void {
  closeSheet();
  if (!activeTrajectRef.value) return;
  router.push({ name: 'instellingen-traject', params: { trajectRef: activeTrajectRef.value, tab } });
}

// --- Nieuw traject: het gedeelde formulier in dezelfde sheet ---
const createFormEl = ref<InstanceType<typeof TrajectCreateForm> | null>(null);
const createBusy = ref(false);
const createError = ref<string | null>(null);
async function startCreate(): Promise<void> {
  createError.value = null;
  sheetMode.value = 'create';
  await nextTick();
  createFormEl.value?.reset();
}
async function submitCreate(): Promise<void> {
  createError.value = null;
  // The form emits `submit` itself, so it is mounted.
  const { payload, error } = createFormEl.value!.buildPayload();
  if (error) {
    createError.value = error;
    return;
  }
  createBusy.value = true;
  try {
    // buildPayload returns a payload whenever it returns no error.
    const created = await createTraject(payload!);
    if (!created.ref) {
      // A half-filled summary would navigate to /editor/undefined; surface it
      // and keep the sheet open instead of silently closing after a real
      // resource was created (re-submitting would make a duplicate).
      console.warn('MobileTrajectSheet: created traject has no ref', created);
      createError.value =
        'Traject aangemaakt maar kon niet worden geopend. Ga terug en selecteer het.';
      return;
    }
    closeSheet();
    await goToTraject(created.ref);
  } catch (e) {
    createError.value = (e as Error).message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}

// --- Artikelen (open tabbladen), zelfde acties als de oude DocumentTabsSheet ---
function tabText(tab: OpenTab): string {
  return `Artikel ${tab.articleNumber}`;
}
function tabSupporting(tab: OpenTab): string | undefined {
  return tabActions.value?.displayName(tab);
}
function isActiveTab(tab: OpenTab): boolean {
  const active = activeDocumentTab.value;
  return !!(active && tabActions.value && tabActions.value.key(active) === tabActions.value.key(tab));
}
function selectTab(tab: OpenTab): void {
  closeSheet();
  tabActions.value?.select(tab);
}
function closeTab(tab: OpenTab): void {
  tabActions.value?.close(tab);
}
// Reorder is tijdelijk uitgeschakeld op sm (drag-bugs). De editor exposeert
// `tabActions.reorder` nog, dus opnieuw inschakelen = `reorderable`
// @nldd-reorder + de drag-handle-cells terugzetten op de Artikelen-lijst.

// Sluit de sheet zodra het scherm groter dan sm wordt (md+ heeft eigen chrome).
let breakpointQuery: MediaQueryList | null = null;
function onBreakpointChange(e: MediaQueryListEvent): void {
  if (e.matches) closeSheet();
}
onMounted(() => {
  breakpointQuery = window.matchMedia?.('(min-width: 641px)') || null;
  breakpointQuery?.addEventListener?.('change', onBreakpointChange);
});
onBeforeUnmount(() => {
  breakpointQuery?.removeEventListener?.('change', onBreakpointChange);
});
</script>

<template>
  <nldd-button
    size="md"
    expandable
    :loading="menuLoading || undefined"
    start-icon="traject"
    width="full"
    horizontal-alignment="left"
    single-line
    :text="buttonText"
    :supporting-text="buttonSupporting"
    @click="openSheet"
  ></nldd-button>

  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="bottom" @close="onSheetClose">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="sheetTitle"
          dismiss-text="Sluit"
          :back-text="sheetMode === 'create' ? 'Terug' : undefined"
          @dismiss="closeSheet"
          @back="sheetMode = 'list'"
        ></nldd-top-title-bar>

        <!-- Niet ingelogd -->
        <nldd-simple-section v-if="!authenticated">
          <nldd-inline-dialog
            icon="login"
            text="Log in om een traject te kiezen of aan te maken"
            supporting-text="Zodra je bent ingelogd zie je hier je lopende trajecten en kun je gemakkelijk wisselen."
          >
            <nldd-button slot="actions" appearance="primary" text="Inloggen" @click="loginToChooser"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>

        <!-- Nieuw traject - zelfde formulier als de aanmaakpagina, in de sheet -->
        <nldd-simple-section v-else-if="sheetMode === 'create'">
          <TrajectCreateForm
            ref="createFormEl"
            :busy="createBusy"
            :error="createError"
            @submit="submitCreate"
          />
        </nldd-simple-section>

        <!-- Lijsten -->
        <nldd-simple-section v-else>
          <!-- Acties van het actieve traject - bovenaan, zonder titel (de
               sheet-titel dekt dit al). -->
          <nldd-list v-if="activeTraject" appearance="box-tinted">
            <nldd-list-item size="md" button @click="openDocuments">
              <nldd-icon-cell size="20"><nldd-icon icon="documents"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Werkdocumenten"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="md" button @click="openTaken">
              <nldd-icon-cell size="20"><nldd-icon icon="tasks"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Taken"></nldd-text-cell>
            </nldd-list-item>
            <!-- Eén rij naar Instellingen, net als in de zijbalk: Algemeen en
                 Leden staan daarbinnen, niet ernaast. -->
            <nldd-list-item size="md" button @click="goToInstellingen()">
              <nldd-icon-cell size="20"><nldd-icon icon="settings"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Instellingen"></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>

          <!-- Trajecten-switcher + nieuw traject. The "Trajecten" section title
               only shows when the traject actions precede it (activeTraject); without
               a traject nothing is above it, so it would just duplicate the
               sheet header. -->
          <template v-if="activeTraject">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5"><h2>Trajecten</h2></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
          </template>
          <nldd-list appearance="box-tinted">
            <nldd-list-item
              v-for="t in trajects"
              :key="t.id"
              size="md"
              button
              :current="t.ref === activeTrajectRef || undefined"
              @click="selectTraject(t)"
            >
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-icon-cell v-if="t.ref === activeTrajectRef" size="20"><nldd-icon icon="check-mark"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell v-else size="20"></nldd-spacer-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="`${t.name}${t.status === 'afgerond' ? ' (afgerond)' : ''}`"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="md" button @click="startCreate">
              <nldd-icon-cell size="20"><nldd-icon icon="plus"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Nieuw traject"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>

          <!-- Lijst 2: Artikelen (editor met open tabbladen). Reorder staat
               tijdelijk uit op sm - eerst nog wat drag-bugs oplossen. -->
          <!-- hasArticles implies tabActions is set, hence the `!` below. -->
          <template v-if="hasArticles">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5"><h2>Artikelen</h2></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list appearance="box-tinted">
              <nldd-list-item
                v-for="tab in documentTabs"
                :key="`${documentTabsTrajectRef ?? ''}:${tabActions!.key(tab)}`"
                size="md"
                button
                :current="isActiveTab(tab) || undefined"
                @click="selectTab(tab)"
              >
                <nldd-spacer-cell size="12"></nldd-spacer-cell>
                <nldd-icon-cell v-if="isActiveTab(tab)" size="20"><nldd-icon icon="check-mark"></nldd-icon></nldd-icon-cell>
                <nldd-spacer-cell v-else size="20"></nldd-spacer-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="tabText(tab)" :supporting-text="tabSupporting(tab)"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-icon-button
                    size="sm"
                    icon="dismiss"
                    text="Sluit tabblad"
                    @click.stop="closeTab(tab)"
                  ></nldd-icon-button>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

</template>
