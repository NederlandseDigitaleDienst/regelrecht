<script setup lang="ts">
// Traject-details as a Home main-pane view (was TrajectInfoDialog's content).
// Renders the detail list + owner-delete / contributor-leave, each behind a
// confirmation modal. Loads on mount and whenever the traject changes.
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
  isCentralSource,
} from '../composables/useTrajectDetail';
import { useTrajectsStore } from '../stores/trajects';
import { hasRole } from '../composables/useAuth';
import { paneChromeVisible } from '../constants';

const router = useRouter();

const props = withDefaults(
  defineProps<{
    /** Traject to show (UUID id). */
    trajectId?: string | null;
  }>(),
  { trajectId: null },
);

const emit = defineEmits<{
  deleted: [trajectId: string | null];
  left: [trajectId: string | null];
}>();

const { detail, loading, error: loadError, load } = useTrajectDetail();
const { updateTraject, moveTrajectRepo, deleteTraject, leaveTraject } = useTrajectsStore();

function reload(): void {
  if (props.trajectId) load(props.trajectId);
}
onMounted(reload);
watch(() => props.trajectId, reload);

const source = computed(() => writableSource(detail.value));
const repoLabel = computed(() => {
  const s = source.value;
  if (!s || !s.gh_owner || !s.gh_repo) return null;
  return `${s.gh_owner}/${s.gh_repo}`;
});
const repoUrl = computed(() => branchTreeUrl(source.value));
const subpath = computed(() => {
  if (!source.value) return 'onbekend';
  const p = source.value.gh_path;
  return p && p.trim() ? p : 'repo-root';
});

// --- Root-pad (subpath) wijzigen (owner van een traject met eigen repo) ---
//
// Alleen de eigenaar mag het pad verzetten, en alleen op een eigen GitHub-repo:
// het centrale corpus deelt zijn indeling met elk traject dat erop schrijft, dus
// daar weigert de backend de wijziging. Wie hem niet mag wijzigen, ziet de
// huidige waarde gewoon als tekst.
const canEditSubpath = computed(
  () =>
    detail.value?.role === 'owner' &&
    !!source.value &&
    source.value.source_type === 'github' &&
    !isCentralSource(source.value),
);

const subpathDraft = ref('');
const subpathSaving = ref(false);
const subpathError = ref<string | null>(null);

// Het veld volgt de geladen bron: bij openen, bij wisselen van traject en na de
// herlaad die op een geslaagde opslag volgt. Leeg veld = repo-root, dezelfde
// afspraak als bij het aanmaken van een traject.
watch(
  source,
  (s) => {
    subpathDraft.value = s?.gh_path || '';
    subpathError.value = null;
  },
  { immediate: true },
);

function onSubpathInput(event: Event): void {
  // nldd-text-field dispatcht een CustomEvent met de nieuwe waarde in
  // event.detail.value. De host zet z'n eigen `value` vóór het dispatchen,
  // dus event.target.value klopt ook, maar detail eerst lezen houdt het
  // contract expliciet - zoals onYamlInput in EditorView.vue en de andere
  // nldd-invoerhandlers. `??` laat een bewust lege waarde (veld gewist =
  // repo-root) door.
  subpathDraft.value =
    (event as CustomEvent<{ value?: string } | null>).detail?.value ??
    (event.target as HTMLInputElement | null)?.value ??
    subpathDraft.value;
  // De melding van de vorige poging gaat weg zodra de gebruiker begint te
  // corrigeren - anders staat een afgekeurd pad nog rood onder een veld
  // waar inmiddels iets anders staat (zelfde afspraak als InviteMembersSheet).
  if (subpathError.value) subpathError.value = null;
}

async function saveSubpath(): Promise<void> {
  if (subpathSaving.value || !props.trajectId) return;
  subpathSaving.value = true;
  subpathError.value = null;
  try {
    await updateTraject(props.trajectId, { repo_path: subpathDraft.value });
    // Herladen in plaats van de waarde lokaal bijwerken: de backend normaliseert
    // (leeg/spaties wordt repo-root), dus dit toont wat er echt is opgeslagen.
    reload();
  } catch (e) {
    subpathError.value = (e as Error).message || 'Opslaan mislukt';
  } finally {
    subpathSaving.value = false;
  }
}

// --- Repo wijzigen (alleen editor-admin, traject met eigen repo) ---
//
// Voor als de repo verhuist, bijvoorbeeld naar een andere GitHub-organisatie.
// Naar welke repo een traject schrijft bepaalt waar de tokens van het platform
// heen gaan, dus dat is aan een beheerder en niet aan de eigenaar. Branch, base
// branch en subpath blijven staan; de backend controleert eerst of de editor op
// de nieuwe repo kan werken en weigert anders met een uitleg.
const canEditRepo = computed(
  () =>
    hasRole('editor-admin') &&
    !!source.value &&
    source.value.source_type === 'github' &&
    !isCentralSource(source.value),
);

const repoDraft = ref('');
const repoSaving = ref(false);
const repoError = ref<string | null>(null);

watch(
  repoLabel,
  (label) => {
    repoDraft.value = label || '';
    repoError.value = null;
  },
  { immediate: true },
);

function onRepoInput(event: Event): void {
  repoDraft.value =
    (event as CustomEvent<{ value?: string } | null>).detail?.value ??
    (event.target as HTMLInputElement | null)?.value ??
    repoDraft.value;
  if (repoError.value) repoError.value = null;
}

async function saveRepo(): Promise<void> {
  if (repoSaving.value || !props.trajectId) return;
  const parts = repoDraft.value.trim().split('/');
  // parts[0] and parts[1] exist once the length check has passed.
  if (parts.length !== 2 || !parts[0]!.trim() || !parts[1]!.trim()) {
    repoError.value = 'Schrijf de repo als eigenaar/naam, bijvoorbeeld example-org/regelrecht-corpus.';
    return;
  }
  repoSaving.value = true;
  repoError.value = null;
  try {
    await moveTrajectRepo(props.trajectId, parts[0]!.trim(), parts[1]!.trim());
    reload();
  } catch (e) {
    repoError.value = (e as Error).message || 'Opslaan mislukt';
  } finally {
    repoSaving.value = false;
  }
}

function orDash(v: string | null | undefined): string {
  return v && String(v).trim() ? v : '—';
}

// De structuurcontrole hangt aan de URL-vorm van het traject (`{slug}-{8hex}`),
// niet aan de UUID die deze pane verder gebruikt; `detail.ref` draagt die.
// Pushen en niet lokaal openen: het adres is de open-stand van de sheet, dus
// Terug sluit hem en een gedeelde link opent hem.
function openStructureCheck(): void {
  if (!detail.value?.ref) return;
  router.push({
    name: 'traject-structuur-controle',
    params: { trajectRef: detail.value.ref },
  });
}

// --- Verwijderen (owner-only) ---
const deleteModalEl = ref<NLDDModalDialog | null>(null);
const confirmingDelete = ref(false);
const deleteBusy = ref(false);
const deleteError = ref<string | null>(null);
watch(confirmingDelete, async (open) => {
  await nextTick();
  const el = deleteModalEl.value;
  if (!el) return;
  if (open) el.show?.();
  else el.hide?.();
});
function askDelete(): void {
  deleteError.value = null;
  confirmingDelete.value = true;
}
function cancelDelete(): void {
  if (!confirmingDelete.value || deleteBusy.value) return;
  confirmingDelete.value = false;
}
async function confirmDelete(): Promise<void> {
  // The traject can disappear while the modal is open; then there is nothing
  // to delete, so the modal just closes. The event carries the id the request
  // was for, even if the prop changes while it runs.
  const trajectId = props.trajectId;
  if (deleteBusy.value) return;
  if (!trajectId) {
    confirmingDelete.value = false;
    return;
  }
  deleteBusy.value = true;
  deleteError.value = null;
  try {
    await deleteTraject(trajectId);
    confirmingDelete.value = false;
    emit('deleted', trajectId);
  } catch (e) {
    deleteError.value = (e as Error).message || 'Verwijderen mislukt';
  } finally {
    deleteBusy.value = false;
  }
}

// --- Verlaten (bijdrager) ---
const leaveModalEl = ref<NLDDModalDialog | null>(null);
const confirmingLeave = ref(false);
const leaveBusy = ref(false);
const leaveError = ref<string | null>(null);
watch(confirmingLeave, async (open) => {
  await nextTick();
  const el = leaveModalEl.value;
  if (!el) return;
  if (open) el.show?.();
  else el.hide?.();
});
function askLeave(): void {
  leaveError.value = null;
  confirmingLeave.value = true;
}
function cancelLeave(): void {
  if (!confirmingLeave.value || leaveBusy.value) return;
  confirmingLeave.value = false;
}
async function confirmLeave(): Promise<void> {
  // Same as confirmDelete: no traject left to leave closes the modal.
  const trajectId = props.trajectId;
  if (leaveBusy.value) return;
  if (!trajectId) {
    confirmingLeave.value = false;
    return;
  }
  leaveBusy.value = true;
  leaveError.value = null;
  try {
    await leaveTraject(trajectId);
    confirmingLeave.value = false;
    emit('left', trajectId);
  } catch (e) {
    leaveError.value = (e as Error).message || 'Verlaten mislukt';
  } finally {
    leaveBusy.value = false;
  }
}

// `loadError` holds whatever load() caught; read its message the way the template did.
function errorMessage(e: unknown): string | undefined {
  return (e as { message?: string } | null)?.message;
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title v-if="paneChromeVisible(loading)" id="instellingen-pane-titel" size="display-3"><h3>Algemeen</h3></nldd-title>
    <nldd-spacer v-if="paneChromeVisible(loading)" size="16"></nldd-spacer>

    <nldd-activity-indicator v-if="loading" text="Trajectgegevens laden" show-text></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="loadError" variant="alert" :text="errorMessage(loadError) || 'Fout bij laden'"></nldd-inline-dialog>
    <template v-else-if="detail">
    <nldd-list appearance="box-tinted">
      <nldd-list-item size="md">
        <nldd-text-cell text="Naam" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="detail.name"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Beschrijving" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="orDash(detail.description)"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Status" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="detail.status"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Jouw rol" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="detail.role"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Repo" max-width="180px" vertical-alignment="top"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-cell v-if="canEditRepo" width="full" vertical-alignment="top">
          <nldd-link
            v-if="repoUrl"
            size="md"
            :href="repoUrl"
            target="_blank"
            rel="noopener noreferrer"
            end-icon="external-link"
            text="Traject-branch op GitHub"
          ></nldd-link>
          <nldd-spacer v-if="repoUrl" size="8"></nldd-spacer>
          <nldd-form-field>
            <nldd-text-field
              size="md"
              name="repo"
              accessible-label="Repo"
              :value="repoDraft"
              :invalid="repoError ? true : undefined"
              :unmet="repoError ? 'repo-error' : undefined"
              @input="onRepoInput"
            ></nldd-text-field>
            <nldd-form-field-help-text>
              Eigenaar/naam van de GitHub-repo. Pas dit aan als de repo naar een andere organisatie is verhuisd.
            </nldd-form-field-help-text>
            <nldd-validation-list>
              <nldd-validation-item id="repo-error">
                {{ repoError }}
              </nldd-validation-item>
            </nldd-validation-list>
          </nldd-form-field>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button
            appearance="secondary"
            size="md"
            :text="repoSaving ? 'Bezig…' : 'Repo wijzigen'"
            :disabled="repoSaving || undefined"
            @click="saveRepo"
          ></nldd-button>
        </nldd-cell>
        <nldd-text-cell
          v-else-if="repoUrl"
          supporting-text="Opent de traject-branch op GitHub in een nieuw tabblad."
          vertical-alignment="top"
        >
          <nldd-link
            size="md"
            :href="repoUrl"
            target="_blank"
            rel="noopener noreferrer"
            end-icon="external-link"
            :text="repoLabel"
          ></nldd-link>
        </nldd-text-cell>
        <nldd-text-cell v-else :text="repoLabel || 'onbekend'"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Branch" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="source?.gh_branch || 'onbekend'"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Base branch" max-width="180px"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="source?.gh_base_branch || 'onbekend'"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item size="md">
        <nldd-text-cell text="Subpath" max-width="180px" vertical-alignment="top"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell v-if="!canEditSubpath" :text="subpath"></nldd-text-cell>
        <nldd-cell v-else width="full" vertical-alignment="top">
          <nldd-form-field>
            <nldd-text-field
              size="md"
              name="repo_path"
              accessible-label="Subpath"
              :value="subpathDraft"
              :invalid="subpathError ? true : undefined"
              :unmet="subpathError ? 'subpath-error' : undefined"
              @input="onSubpathInput"
            ></nldd-text-field>
            <nldd-form-field-help-text>
              Submap met regulation YAML-bestanden. Laat leeg voor repo-root.
            </nldd-form-field-help-text>
            <nldd-validation-list>
              <nldd-validation-item id="subpath-error">
                {{ subpathError }}
              </nldd-validation-item>
            </nldd-validation-list>
          </nldd-form-field>
          <nldd-spacer size="8"></nldd-spacer>
          <!-- Permanent, niet pas na een fout: het pad bepaalt alles wat de
               editor op deze repo leest en schrijft, dus dit is wat je moet
               weten vóór je opslaat. -->
          <nldd-banner
            variant="warning"
            text="Let op: alles buiten deze map ziet de editor niet meer als regulation. Dat geldt ook voor annotaties en documenten van dit traject."
          ></nldd-banner>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button
            appearance="secondary"
            size="md"
            :text="subpathSaving ? 'Bezig…' : 'Opslaan'"
            :disabled="subpathSaving || undefined"
            @click="saveSubpath"
          ></nldd-button>
        </nldd-cell>
      </nldd-list-item>
    </nldd-list>

    <!-- Eigen vlak, net als het verwijder-vlak eronder: een kop, een zin en een
         knop. Geen status of aantal hier - dat zou de repo bij elke keer
         Instellingen openen opnieuw laten doorlezen, terwijl je hier meestal
         iets heel anders komt doen. -->
    <template v-if="detail.ref">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-box>
        <nldd-container padding="16">
          <nldd-title size="display-5"><h4>Traject-structuur controleren</h4></nldd-title>
          <nldd-spacer size="4"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Werk je buiten de editor om in de repo, dan kunnen map- en
              bestandsnamen uit de pas raken met wat erin staat. Deze controle
              zoekt die verschillen, plus dubbele id's en verwijzingen die
              nergens uitkomen.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button appearance="secondary" size="md" text="Controleer" @click="openStructureCheck"></nldd-button>
        </nldd-container>
      </nldd-box>
    </template>

    <!-- De onomkeerbare actie in een eigen vlak, met een kopje erboven: zo zie je
         bij het scrollen dat er nog iets komt, in plaats van dat je een rode knop
         onder een tabel moet ontdekken. -->
    <template v-if="detail.role === 'owner'">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-box background="critical">
        <nldd-container padding="16">
          <nldd-title size="display-5"><h4>Traject {{ detail.name }} verwijderen</h4></nldd-title>
          <nldd-spacer size="4"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Het traject verdwijnt met zijn leden en uitnodigingen. De
              traject-branch op GitHub blijft bestaan.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button appearance="destructive" size="md" text="Traject verwijderen" @click="askDelete"></nldd-button>
        </nldd-container>
      </nldd-box>
    </template>
    <template v-else-if="detail.role">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-box>
        <nldd-container padding="16">
          <nldd-title size="display-5"><h4>Traject verlaten</h4></nldd-title>
          <nldd-spacer size="4"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Je verliest toegang tot dit traject. Een eigenaar kan je later
              opnieuw uitnodigen.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button appearance="destructive" size="md" text="Traject verlaten" @click="askLeave"></nldd-button>
        </nldd-container>
      </nldd-box>
    </template>
    </template>
  </nldd-simple-section>

  <Teleport to="body">
    <nldd-modal-dialog
      ref="deleteModalEl"
      variant="alert"
      horizontal-alignment="center"
      :text="`Traject ${detail?.name || ''} verwijderen?`"
      supporting-text="Het traject wordt definitief verwijderd, inclusief leden en uitnodigingen. De traject-branch op GitHub blijft bestaan. Dit kan niet ongedaan worden gemaakt."
      @close="cancelDelete"
    >
      <nldd-inline-dialog v-if="deleteError" variant="alert" :text="deleteError"></nldd-inline-dialog>
      <nldd-button slot="actions" appearance="primary" text="Behoud traject" @click="cancelDelete"></nldd-button>
      <nldd-button
        slot="actions"
        appearance="destructive"
        :text="deleteBusy ? 'Bezig…' : 'Verwijder traject'"
        :disabled="deleteBusy || undefined"
        @click="confirmDelete"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>

  <Teleport to="body">
    <nldd-modal-dialog
      ref="leaveModalEl"
      variant="alert"
      horizontal-alignment="center"
      :text="`Traject ${detail?.name || ''} verlaten?`"
      supporting-text="Je verlaat dit traject definitief en verliest meteen je toegang. Wil je later weer bijdragen, dan moet een beheerder je opnieuw uitnodigen."
      @close="cancelLeave"
    >
      <nldd-inline-dialog v-if="leaveError" variant="alert" :text="leaveError"></nldd-inline-dialog>
      <nldd-button slot="actions" appearance="primary" text="Blijf in traject" @click="cancelLeave"></nldd-button>
      <nldd-button
        slot="actions"
        appearance="destructive"
        :text="leaveBusy ? 'Bezig…' : 'Verlaat traject'"
        :disabled="leaveBusy || undefined"
        @click="confirmLeave"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
