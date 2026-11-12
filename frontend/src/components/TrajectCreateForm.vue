<script setup lang="ts">
import { computed, ref } from 'vue';
import type { NLDDTextField } from '@nldd/design-system/text-field';
import { deriveAuthRef, tokenEnvName as tokenEnvNameFor } from '../lib/corpusAuth';
import type { CreateTrajectRequest } from '../stores/trajects';

// Gedeeld aanmaakformulier voor een traject - gebruikt door de
// TrajectMenu-sheet en de /editor/nieuw-traject-pagina. Gebouwd op de
// NLDD-formulierfamilie: nldd-form (light-DOM <form>), nldd-form-field
// met help-teksten en nldd-form-actions met de submit-knop. Het formulier
// bezit de veldenstate en de validatie; de host bezit de submit-afhandeling
// (busy-state, createTraject, navigatie) en luistert op het
// `submit`-event. De host valideert via `buildPayload()` (exposed) en
// krijgt `{ payload }` of `{ error }` terug; `reset()` maakt de velden
// leeg.
withDefaults(
  defineProps<{
    // Foutmelding van de host (validatie of mislukte create) - boven de
    // acties getoond zodat de melding bij de velden staat.
    error?: string | null;
    // Create-call onderweg: toont de busy-regel en blokkeert de knop.
    busy?: boolean;
  }>(),
  { error: null, busy: false },
);

const emit = defineEmits<{ submit: [] }>();

/** The form's field state; the repo keys match CreateTrajectRequest. */
interface CreateFormState {
  name: string;
  description: string;
  useCustomRepo: boolean;
  repo_owner: string;
  repo_name: string;
  base_branch: string;
  repo_path: string;
}

/** The fields bound to a text input. */
type TextFieldKey = Exclude<keyof CreateFormState, 'useCustomRepo'>;

/** What buildPayload() hands the host: a request body, or a validation error. */
type BuildPayloadResult =
  | { payload: CreateTrajectRequest; error?: undefined }
  | { payload?: undefined; error: string };

function emptyForm(): CreateFormState {
  return {
    name: '',
    description: '',
    // When `useCustomRepo` is false the create call omits the repo
    // fields and the backend falls back to the default MinBZK repo -
    // existing trajects keep their behaviour unchanged.
    useCustomRepo: false,
    repo_owner: '',
    repo_name: '',
    base_branch: 'main',
    // Sub-path within the repo where regulation YAML files live. Empty
    // means "everything under repo root" - the right default for user
    // repos dedicated to regulations. Set to e.g. `regulation/nl` when
    // the YAMLs live in a sub-directory.
    repo_path: '',
  };
}

const form = ref(emptyForm());
const nameFieldEl = ref<NLDDTextField | null>(null);

// Showing the exact env-var name saves the operator from deriving it by hand.
// The derivation itself lives in lib/corpusAuth.js, next to the test that pins
// it to the backend.
const tokenEnvName = computed(() => {
  const owner = form.value.repo_owner.trim();
  const repo = form.value.repo_name.trim();
  if (!owner || !repo) return '';
  const authRef = deriveAuthRef(owner, repo);
  // Input of nothing but separators leaves no ref, and the backend rejects that
  // before the insert. Naming CORPUS_AUTH__TOKEN would send the operator off to
  // set a variable that resolves nothing.
  if (!authRef) return '';
  return tokenEnvNameFor(authRef);
});

// Complete enough to describe: the coordinates yield a token name. Checking the
// derived name rather than the raw fields also catches input that trims to
// something non-empty but carries no letters or digits ("..."), which leaves no
// ref and so no env-var to point the operator at.
const repoIsCompleet = computed(() => Boolean(tokenEnvName.value));

function reset(): void {
  form.value = emptyForm();
}

// The host focuses the first field once the sheet/page has opened.
function focus(): void {
  nameFieldEl.value?.focus?.();
}

// Build the request body. Only attach the repo fields when the toggle
// is on - leaving them out lets the backend pick the MinBZK default
// so existing flows are unchanged for users who don't need a custom
// repo. Returns { payload } on success or { error } when validation
// fails.
function buildPayload(): BuildPayloadResult {
  if (!form.value.name.trim()) {
    return { error: 'Naam is verplicht' };
  }
  // `scope` is vervallen als apart veld (zit inhoudelijk in de
  // beschrijving); de backend default 'm naar een lege string.
  const payload: CreateTrajectRequest = {
    name: form.value.name.trim(),
    description: form.value.description,
  };
  if (form.value.useCustomRepo) {
    const owner = form.value.repo_owner.trim();
    const repo = form.value.repo_name.trim();
    const branch = form.value.base_branch.trim();
    if (!owner || !repo || !branch) {
      return {
        error: 'Eigen repo: vul owner, repo en base-branch in (of zet de schakelaar uit).',
      };
    }
    payload.repo_owner = owner;
    payload.repo_name = repo;
    payload.base_branch = branch;
    // `repo_path` is optional; only attach when the user filled it in
    // so the backend keeps using its empty-string default ("repo root")
    // for personal regulation repos.
    const subpath = form.value.repo_path.trim();
    if (subpath) {
      payload.repo_path = subpath;
    }
  }
  return { payload };
}

defineExpose({ reset, buildPayload, focus });

// Input event handlers: NDD text-field/text-area dispatch on the bare
// <input>/<textarea> element, so target.value is set; some custom-element
// variants dispatch a custom event with detail.value. Read both to stay
// robust across NDD versions (matches the pattern in EditSheet.vue).
function bind(field: TextFieldKey) {
  return (event: Event) =>
    (form.value[field] =
      (event.target as HTMLInputElement | null)?.value ??
      (event as CustomEvent<{ value?: string } | null>).detail?.value ??
      form.value[field]);
}
</script>

<template>
  <!-- User-provided form mode: Vue bezit het <form>-element, nldd-form
       doet attribute-mirroring zonder children te verplaatsen (geen
       conflict met Vue's reconciliation). novalidate: validatie loopt via
       buildPayload, niet via de browser. -->
  <nldd-form novalidate label-alignment="right">
    <form @submit.prevent="emit('submit')">
      <nldd-form-field label="Naam">
        <nldd-text-field
          ref="nameFieldEl"
          size="md"
          required
          :value="form.name"
          @input="bind('name')($event)"
        ></nldd-text-field>
        <nldd-form-field-help-text>Korte herkenbare titel, bijv. 'Tariefswijziging 2026'.</nldd-form-field-help-text>
      </nldd-form-field>
      <nldd-form-field label="Beschrijving" optional>
        <nldd-multi-line-text-field
          size="md"
          resize="auto"
          :value="form.description"
          @input="bind('description')($event)"
        ></nldd-multi-line-text-field>
        <nldd-form-field-help-text>Bijvoorbeeld de reden van dit traject of welke wetten of onderwerpen onder dit traject vallen.</nldd-form-field-help-text>
      </nldd-form-field>

      <nldd-form-field>
        <nldd-switch-field
          label="Eigen GitHub-repo (i.p.v. standaard MinBZK-repo)"
          :checked="form.useCustomRepo ? true : undefined"
          @change="form.useCustomRepo = Boolean($event.detail?.checked)"
        ></nldd-switch-field>
        <nldd-form-field-help-text>
          De standaardrepo is publiek: wat je in dit traject bewerkt, is voor
          iedereen te zien. Kies een eigen repo als het werk nog niet openbaar mag
          zijn, of als de regelgeving bij je eigen organisatie thuishoort.
        </nldd-form-field-help-text>
      </nldd-form-field>

      <template v-if="form.useCustomRepo">
        <nldd-form-field label="Repo owner">
          <nldd-text-field
            size="md"
            required
            :value="form.repo_owner"
            @input="bind('repo_owner')($event)"
          ></nldd-text-field>
          <nldd-form-field-help-text>Organisatie of gebruiker op GitHub, bijv. 'MinBZK'.</nldd-form-field-help-text>
        </nldd-form-field>
        <nldd-form-field label="Repo">
          <nldd-text-field
            size="md"
            required
            :value="form.repo_name"
            @input="bind('repo_name')($event)"
          ></nldd-text-field>
          <nldd-form-field-help-text>Naam van de repository.</nldd-form-field-help-text>
        </nldd-form-field>
        <nldd-form-field label="Base branch">
          <nldd-text-field
            size="md"
            required
            :value="form.base_branch"
            @input="bind('base_branch')($event)"
          ></nldd-text-field>
          <nldd-form-field-help-text>Branch waar de traject-branch van afsplitst (vaak 'main').</nldd-form-field-help-text>
        </nldd-form-field>
        <nldd-form-field label="Subpath" optional>
          <nldd-text-field
            size="md"
            :value="form.repo_path"
            @input="bind('repo_path')($event)"
          ></nldd-text-field>
          <nldd-form-field-help-text>Submap met de regelwerken. Laat leeg voor repo-root.</nldd-form-field-help-text>
        </nldd-form-field>
      </template>

      <!-- Bij een eigen repo pas tonen zodra eigenaar en repository een naam
           opleveren: een zin met lege plekken erin leest als een fout. -->
      <nldd-form-field v-if="!form.useCustomRepo || repoIsCompleet">
        <nldd-rich-text>
          <p v-if="form.useCustomRepo">
            Bewerkingen worden gepusht naar
            <code>{{ form.repo_owner.trim() }}/{{ form.repo_name.trim() }}</code>
            (basis: <code>{{ form.base_branch.trim() || 'main' }}</code>).
            Daarvoor is schrijftoegang tot deze repo nodig: je beheerder zet de
            env-var <code>{{ tokenEnvName }}</code>, of, als schrijven met een
            eigen GitHub-account aanstaat, gebruikt de editor jouw gekoppelde
            account (Instellingen). Zonder een van beide krijg je een
            foutmelding bij aanmaken. Commits verschijnen onder je eigen naam.
          </p>
          <p v-else>
            Bewerkingen in dit traject worden gepusht naar een aparte branch op
            <code>MinBZK/regelrecht-corpus</code> (basis:
            <code>development</code>). Commits verschijnen onder je eigen naam
            (uit je SSO-account).
          </p>
        </nldd-rich-text>
      </nldd-form-field>

      <nldd-inline-dialog
        v-if="error"
        variant="alert"
        :text="error"
      ></nldd-inline-dialog>

      <nldd-activity-indicator
        v-if="busy"
        show-text
        text="Traject wordt aangemaakt en branch wordt op de remote gezet - dit kan even duren."
      ></nldd-activity-indicator>

      <nldd-form-actions>
        <nldd-button
          appearance="primary"
          size="md"
          width="full"
          :text="busy ? 'Bezig…' : 'Maak traject aan'"
          :disabled="busy || undefined"
          @click="emit('submit')"
        ></nldd-button>
      </nldd-form-actions>
    </form>
  </nldd-form>
</template>
