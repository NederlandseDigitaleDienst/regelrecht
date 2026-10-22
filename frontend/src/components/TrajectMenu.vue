<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { useTrajects } from '../composables/useTrajects';
import type { TrajectSummary } from '../composables/useTrajects';
import { useAuth } from '../composables/useAuth';
import { useLoginToChooser } from '../composables/useLoginToChooser';
import { trajectSwitchTarget } from '../composables/useLastVisitedRoute';
import TrajectCreateForm from './TrajectCreateForm.vue';

const props = withDefaults(
  defineProps<{
    // Mobile presentation: stretch the trigger button to the full toolbar
    // width with left-aligned content.
    fullWidth?: boolean;
  }>(),
  { fullWidth: false },
);

const emit = defineEmits<{ switched: [trajectRef: string] }>();

const {
  trajects,
  activeTrajectRef,
  activeTraject,
  loading,
  createTraject,
} = useTrajects();
// Auth gates the menu: logged-in users get the traject switcher; everyone
// else gets a popover explaining that trajecten unlock after login.
const { authenticated } = useAuth();
const route = useRoute();
const router = useRouter();

// Not logged in: log in, then land on the trajectchooser carrying the current
// section + law/article (shared composable, see also MobileTrajectSheet).
const loginToChooser = useLoginToChooser();

// "Account aanvragen" op de niet-ingelogd-popover (zoals de editor/bewerken
// login-popover): naar de publieke aanvraagpagina. /account-aanvragen is een
// top-level route, dus de popover verdwijnt met de shell.
const accountRequestHref = computed(() => router.resolve({ name: 'account-aanvragen' }).href);
function goToAccountRequest(): void {
  router.push({ name: 'account-aanvragen' });
}

// Open the active traject's werkdocumenten (now folded into Home, not a sheet).
function goToWerkdocumenten(): void {
  if (!activeTrajectRef.value) return;
  router.push({ name: 'werkdocumenten-traject', params: { trajectRef: activeTrajectRef.value } });
}

// Open the active traject's taken - the same Home panel the sidebar's
// Taken item opens, reachable without going back to Home first.
function goToTaken(): void {
  if (!activeTrajectRef.value) return;
  router.push({ name: 'taken-traject', params: { trajectRef: activeTrajectRef.value } });
}

/**
 * Navigate to a traject - push the user into the traject-scoped view of the
 * section they are currently in (bibliotheek or editor), at that section's
 * ROOT. A switch deliberately drops the law you were viewing: the new traject
 * has its own corpus, so carrying the old lawId across would try to open a
 * document it doesn't have (and leak the previous traject's tab). The editor
 * shows the new traject's own saved tabs in the bar without auto-opening one.
 * Per-tab state: a switch here only affects this tab, never other open tabs.
 */
async function goToTraject(trajectRef: string): Promise<void> {
  await router.push(trajectSwitchTarget(route.name, trajectRef));
}


// A traject in the URL whose name hasn't resolved yet: the button shows a
// spinner (see :loading) with the neutral 'Trajecten' label, not a '…'
// placeholder. Without a traject in the URL there is nothing to load.
const menuLoading = computed(
  () => !!activeTrajectRef.value && loading.value && !activeTraject.value,
);
const activeLabel = computed(() => {
  if (activeTraject.value) return activeTraject.value.name;
  // No traject in the URL (Corpus juris, or still loading): the neutral label.
  return 'Trajecten';
});

// --- Create-sheet state ---
// Veldenstate en validatie leven in het gedeelde TrajectCreateForm
// (zelfde formulier als de /editor/nieuw-traject-pagina); de sheet bezit
// alleen submit, busy-state en navigatie.
const sheetEl = ref<NLDDSheet | null>(null);
const createFormEl = ref<InstanceType<typeof TrajectCreateForm> | null>(null);
const showCreate = ref(false);

const createBusy = ref(false);
const createError = ref<string | null>(null);

// nldd-sheet exposes imperative show()/hide(); mirror showCreate into those
// calls so the sheet animates instead of mounting/unmounting (matches the
// ActionSheet / EditSheet pattern in this app).
watch(showCreate, async (v) => {
  await nextTick();
  if (v) {
    sheetEl.value?.show();
    await nextTick();
    createFormEl.value?.focus?.();
  } else {
    sheetEl.value?.hide();
  }
});

function openCreate(): void {
  createError.value = null;
  createFormEl.value?.reset();
  showCreate.value = true;
}

// Open the active traject's settings (details / leden) in Home.
function goToInstellingen(tab: string): void {
  if (!activeTrajectRef.value) return;
  router.push({ name: 'instellingen-traject', params: { trajectRef: activeTrajectRef.value, tab } });
}

function closeCreate(): void {
  if (createBusy.value) return;
  showCreate.value = false;
}

async function selectTraject(t: TrajectSummary): Promise<void> {
  // `t.ref` is a server-supplied `Option<String>` and serialises to
  // `null` when a `TrajectSummary` is built without calling
  // `fill_ref()`. Refuse to navigate - silently routing to
  // `/editor/null/...` would just bounce off the trajectRef regex
  // and confuse the user. Treat as a programming error on the
  // backend side, log and bail.
  if (!t.ref) {
    console.warn('TrajectMenu: traject has no ref', t);
    return;
  }
  if (t.ref === activeTrajectRef.value) return;
  await goToTraject(t.ref);
  emit('switched', t.ref);
}

async function submitCreate(): Promise<void> {
  createError.value = null;
  // Validatie en request-body komen uit het gedeelde formulier.
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
    showCreate.value = false;
    // Jump straight into the new traject - same per-tab navigation as
    // selecting from the dropdown. Mirror the `selectTraject` guard:
    // the backend's `create` handler always calls `fill_ref()` so this
    // shouldn't fire today, but a future refactor that returns a
    // half-filled `TrajectSummary` would otherwise navigate to
    // `/editor/undefined/...` and silently no-op.
    if (!created.ref) {
      console.warn('TrajectMenu: created traject has no ref', created);
      return;
    }
    await goToTraject(created.ref);
    emit('switched', created.ref);
  } catch (e) {
    createError.value = (e as Error).message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}

</script>

<template>
  <nldd-button
    data-testid="traject-menu-trigger"
    size="md"
    expandable
    :loading="menuLoading || undefined"
    :start-icon="fullWidth ? 'traject' : undefined"
    :text="activeLabel"
    :width="fullWidth ? 'full' : undefined"
    :max-width="fullWidth ? undefined : '220px'"
    :horizontal-alignment="fullWidth ? 'left' : undefined"
  >
    <!-- Logged in: the active traject's actions first, then the trajecten to
         switch to + create below a divider. -->
    <nldd-menu v-if="authenticated" slot="popup">
      <nldd-menu-item
        v-if="activeTraject"
        text="Werkdocumenten"
        icon="documents"
        @click="goToWerkdocumenten"
      ></nldd-menu-item>
      <nldd-menu-item
        v-if="activeTraject"
        text="Taken"
        icon="tasks"
        @click="goToTaken"
      ></nldd-menu-item>
      <!-- Zelfde nesting als de zijbalk: Instellingen met Algemeen en Leden
           eronder, zodat het menu en de navigatie hetzelfde model tonen. -->
      <nldd-menu-item v-if="activeTraject" text="Instellingen" icon="settings">
        <nldd-menu>
          <nldd-menu-item
            text="Algemeen"
            icon="traject"
            @click="goToInstellingen('details')"
          ></nldd-menu-item>
          <nldd-menu-item
            text="Leden"
            icon="person-2"
            @click="goToInstellingen('leden')"
          ></nldd-menu-item>
        </nldd-menu>
      </nldd-menu-item>
      <!-- The group draws its own divider above (auto-suppressed when it's the
           first child, i.e. no active-traject actions precede it), so no manual
           nldd-menu-divider here. -->
      <nldd-menu-group text="Trajecten">
        <nldd-menu-item
          v-for="t in trajects"
          :key="t.id"
          type="radio"
          :selected="t.ref === activeTrajectRef || undefined"
          :text="`${t.name}${t.status === 'afgerond' ? ' (afgerond)' : ''}`"
          @select="selectTraject(t)"
        ></nldd-menu-item>
        <nldd-menu-item
          text="Nieuw traject…"
          icon="plus"
          @click="openCreate"
        ></nldd-menu-item>
      </nldd-menu-group>
    </nldd-menu>

    <!-- Not logged in: no menu - a popover explaining that trajecten unlock
         once you sign in. -->
    <nldd-popover v-else slot="popup" accessible-label="Trajecten" width="320px">
      <nldd-container padding="16">
        <nldd-inline-dialog
          icon="login"
          text="Log in om een traject te kiezen of aan te maken"
          supporting-text="Zodra je bent ingelogd zie je hier je lopende trajecten en kun je gemakkelijk wisselen."
        >
          <nldd-button
            slot="actions"
            appearance="primary"
            text="Inloggen"
            @click="loginToChooser"
          ></nldd-button>
          <nldd-button
            slot="actions"
            appearance="secondary"
            text="Account aanvragen"
            :href="accountRequestHref"
            @click.prevent="goToAccountRequest"
          ></nldd-button>
        </nldd-inline-dialog>
      </nldd-container>
    </nldd-popover>
  </nldd-button>

  <!-- Teleport the sheet out of the toolbar so it doesn't inherit the
       toolbar's positioning / clipping. Matches the ScenarioBuilder
       pattern. Each TrajectMenu instance (md/lg/sm) teleports its own
       sheet to body but only one is active per breakpoint, so they
       don't visually collide. -->
  <Teleport to="body">
    <nldd-sheet
      ref="sheetEl"
      placement="right"
      width="520px"
      full-height
      @close="closeCreate"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Nieuw traject"
          :dismiss-text="createBusy ? '' : 'Annuleer'"
          @dismiss="closeCreate"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <TrajectCreateForm ref="createFormEl"
            :busy="createBusy"
            :error="createError"
            @submit="submitCreate"
          />
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
