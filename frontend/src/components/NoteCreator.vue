<script setup lang="ts">
/**
 * NoteCreator - the note authoring form (RFC-018 write path, steps 3-5).
 *
 * Rendered by EditorView in the notes sheet, opened from the toolbar comment
 * button on an editor selection or from a note group's "Notitie toevoegen".
 * Receives the raw [start,end) code-point range the selection mapped to.
 * This component builds the TextQuoteSelector
 * (growing context until it is unique), lets the user pick a motivation and
 * fill the matching body, and emits a complete W3C Annotation object. It does
 * not persist - useDraftNotes (owned by EditorApp) does, so the new note
 * highlights live alongside committed ones.
 */
import { ref, computed, watch, onMounted } from 'vue';
import type { NLDDTextEditor } from '@nldd/design-system/text-editor';
import { buildSelector, cpToUtf16 } from '../composables/useTextSelection';
import type { NoteResolver } from '../composables/useTextSelection';
import type { Note } from '../composables/useNotes';
import { useAmbiguityVocabulary } from '../composables/useAmbiguityVocabulary';
import { documentsListUrl } from '../composables/corpusUrls';
import { apiFetchJson } from '../lib/apiFetch';
import { useAuth } from '../composables/useAuth';
import { quoteContext } from '../lib/quoteContext';
import QuotedFragment from './QuotedFragment.vue';

/** A [start, end) code-point range into the article text. */
interface CodePointRange {
  start: number;
  end: number;
}

/** The part of an article this component reads (law-format keys). */
interface LinkableArticle {
  number?: string | number;
  machine_readable?: {
    execution?: { output?: ({ name?: string } | null)[] | null } | null;
    definitions?: Record<string, unknown> | null;
  } | null;
}

/** A link target chosen in the form. */
interface NoteLink {
  type: 'element' | 'document';
  value: string;
  label: string;
}

/** The part of a W3C annotation body that prefill() reads. */
interface NoteBodyPart {
  purpose?: string;
  value?: string;
  source?: string;
}

/** A token-field `change` event: the selected values on `detail`. */
type ValuesEvent = Event & { detail?: { values?: string[] } | null };
/** A switch-field `change` event: the state on `detail` or on the target. */
type CheckedEvent = Event & { detail?: { checked?: boolean } | null };

const props = withDefaults(defineProps<{
  // Raw (code-point) char range into rawText, or null when closed.
  range?: CodePointRange | null;
  rawText?: string;
  lawId?: string;
  // The selected article object, for the linking target picker.
  article?: LinkableArticle | null;
  // Loaded WASM engine (resolveNote) for selector uniqueness validation.
  engine?: NoteResolver | null;
  // `valid_from` of the law version on screen, so uniqueness is validated in
  // the text being annotated instead of the newest loaded version. Empty
  // means "latest" (a law without valid_from).
  validFrom?: string;
  // Active traject ref. Required to surface the "Document" link mode -
  // without a traject there is no documents folder to pick from.
  trajectRef?: string;
  // When set, the form opens in EDIT mode pre-filled from this existing draft
  // note (its body/tag/link/workflow). The parent replaces the draft on save.
  initialNote?: Note | null;
}>(), {
  range: null,
  rawText: '',
  lawId: '',
  article: null,
  engine: null,
  validFrom: '',
  trajectRef: '',
  initialNote: null,
});

const isEditing = computed(() => !!props.initialNote);

const emit = defineEmits<{
  create: [note: Note, share: boolean];
  cancel: [];
}>();

// The comment field, autofocused when the form opens so the user can type at once.
const commentFieldEl = ref<NLDDTextEditor | null>(null);

// The note's author is the signed-in user - id for traceability, name for
// display. Not a free-text choice.
const { person } = useAuth();
// One flat form: the user fills any combination of a comment, an action
// status, an ambiguity tag and a link. There is no motivation picker; the
// primary motivation is derived at save time from what was filled.
const commentText = ref('');
// Links attach the note to one or more targets: intra-law machine_readable
// elements and/or traject documents, mixed freely. Each becomes a
// `SpecificResource` linking body. Shape: { type: 'element'|'document', value,
// label }. A token-field adds them from one type-tagged, filterable list.
const links = ref<NoteLink[]>([]);
const documentOptions = ref<string[]>([]);
const documentsLoadError = ref<Error | null>(null);
const ambiguityTags = ref<string[]>([]);
// Task modelling with two switches instead of one status picker: whether this
// note is a task for the author, and (only then) whether it is done. Derived
// into the W3C workflow: not a task -> 'none'; task open -> 'open'; task done ->
// 'resolved'.
const isTask = ref(false);
const taskDone = ref(false);
const workflow = computed(() => (!isTask.value ? 'none' : taskDone.value ? 'resolved' : 'open'));
// Share the note with the traject on save (commit to the traject branch) instead
// of keeping it a private local draft. Default off - sharing is irreversible.
// Only offered when a traject is active (there is nowhere to share to otherwise).
const shareWithTraject = ref(false);

const { items: ambiguityItems } = useAmbiguityVocabulary();

// Ambiguity labels are multi-select via nldd-token-field: the whole vocabulary is
// slotted as options (the field hides already-picked ones) and it emits the full
// selected id list on every change.
function onTagsChange(e: Event) {
  ambiguityTags.value = (e as ValuesEvent).detail?.values ?? [];
}

// Lazy-load the traject's documents the first time the form opens with
// a traject in scope. Without a traject there is no documents folder to
// link into, so skip the fetch.
async function fetchDocumentOptions() {
  if (!props.trajectRef) return;
  documentsLoadError.value = null;
  try {
    const json = await apiFetchJson<{ documents?: { path: string }[] } | null>(documentsListUrl(props.trajectRef), {
      errorMessage: (status: number) => `HTTP ${status}`,
    });
    documentOptions.value = Array.isArray(json?.documents)
      ? json.documents.map((d) => d.path)
      : [];
  } catch (e) {
    // apiFetchJson rejects with an Error (ApiError, or a network TypeError).
    documentsLoadError.value = e as Error;
    documentOptions.value = [];
  }
}

// Flatten the article's machine_readable element names so a linking note can
// point at one. Outputs and definitions are the link-worthy elements (RFC-018
// linking example targets #hoogte_zorgtoeslag, an output).
const linkableElements = computed(() => {
  const mr = props.article?.machine_readable;
  if (!mr) return [];
  const names = new Set<string>();
  for (const o of mr.execution?.output ?? []) {
    if (o?.name) names.add(o.name);
  }
  for (const key of Object.keys(mr.definitions ?? {})) names.add(key);
  return [...names];
});

// One combined, type-tagged option list for the token-field: intra-law elements
// and traject documents. Each carries a unique key (`el:`/`doc:` prefix) so a
// selected value maps back to the right target, and an icon so the picker shows
// the kind. The token-field hides already-picked options itself.
const linkOptions = computed(() => {
  const els = linkableElements.value.map((el) => ({
    key: `el:${el}`, type: 'element' as const, value: el, label: el, icon: 'code',
  }));
  const docs = (props.trajectRef ? documentOptions.value : []).map((p) => ({
    key: `doc:${p}`, type: 'document' as const, value: p, label: p, icon: 'file',
  }));
  return [...els, ...docs];
});
// The token-field's values are the option keys; on change, map them back to the
// structured links, reusing existing entries so a link's label survives even if
// its option is no longer in the current list.
const linkValues = computed(() => links.value.map((l) => (l.type === 'element' ? 'el:' : 'doc:') + l.value));
function onLinksChange(e: Event) {
  const keys = (e as ValuesEvent).detail?.values ?? [];
  const existing = new Map(links.value.map((l) => [(l.type === 'element' ? 'el:' : 'doc:') + l.value, l]));
  links.value = keys
    .map((key) => {
      if (existing.has(key)) return existing.get(key);
      const opt = linkOptions.value.find((o) => o.key === key);
      return opt ? { type: opt.type, value: opt.value, label: opt.label } : null;
    })
    .filter((l): l is NoteLink => Boolean(l));
}

// Selector for the current selection, recomputed when the range changes.
// buildSelector grows prefix/suffix until the resolver finds it uniquely;
// `status` drives the warning shown below the quote.
// Without an article number a match cannot be checked against the selected
// article, so there is no selector (and nothing to save) until there is one.
const selectorResult = computed(() => {
  const articleNumber = props.article?.number;
  if (!props.range || !props.rawText || !props.engine || !props.lawId || articleNumber == null) {
    return null;
  }
  return buildSelector(
    props.rawText,
    props.range,
    props.lawId,
    props.engine,
    articleNumber,
    props.validFrom || null,
  );
});

const exact = computed(() => selectorResult.value?.exact ?? '');

// The referenced fragment with a little context, rendered the same way as in
// the note sheet and pane (see QuotedFragment). `range` is in code points.
const referencedFragment = computed(() => {
  if (!props.range || !props.rawText) return null;
  return quoteContext(
    props.rawText,
    cpToUtf16(props.rawText, props.range.start),
    cpToUtf16(props.rawText, props.range.end),
  );
});
const selectorStatus = computed(() => selectorResult.value?.status ?? null);
const selectorReason = computed(() => selectorResult.value?.reason ?? null);
const selectorSkipReason = computed(() => selectorResult.value?.skipReason ?? null);

// Individual "is this part filled?" checks, shared by canSave and the body /
// motivation builders. A link only counts once its target is actually chosen.
const hasComment = computed(() => commentText.value.trim().length > 0);
const hasTag = computed(() => ambiguityTags.value.length > 0);
const hasLink = computed(() => links.value.length > 0);

const canSave = computed(() => {
  if (!selectorResult.value) return false;
  if (selectorStatus.value !== 'found') return false; // ambiguous/orphaned: fix first
  // At least one piece of content - an action status alone isn't a note.
  return hasComment.value || hasTag.value || hasLink.value;
});

// The submit button stays enabled at all times; an invalid submit surfaces a
// banner instead of silently doing nothing. Clear it the moment the form
// becomes submittable so the warning never lingers after it's addressed.
const showValidationError = ref(false);
watch(canSave, (ok) => {
  if (ok) showValidationError.value = false;
});

// Rendered as sheet content (the parent sheet owns visibility). When a range is
// present, autofocus the comment field and refresh the documents list; clear the
// form when it goes away.
function applyRange(range: CodePointRange | null) {
  if (range) {
    requestAnimationFrame(() => commentFieldEl.value?.focus?.());
    fetchDocumentOptions().catch(() => {});
  } else {
    reset();
  }
}
watch(() => props.range, applyRange);
// The selected article can change while the form is open (tab click, browser
// back). `range` holds offsets into the OLD article's text; buildSelector
// would slice a garbage substring of the new text at stale offsets and could
// anchor a note on text the user never selected. Cancel the whole flow on any
// article/law switch instead. Keyed on lawId + article number, not object
// identity: a save/reload of the same article swaps the object but keeps the
// offsets valid, and must not slam the form shut mid-edit.
watch(
  () => [props.lawId, props.article?.number],
  ([lawId, nr], [prevLawId, prevNr]) => {
    if (String(lawId) !== String(prevLawId) || String(nr) !== String(prevNr)) {
      emit('cancel');
    }
  },
);
// v-if mounts this fresh when editing starts, so the range is already set - run
// the setup once on mount (the non-immediate watch alone would miss it).
onMounted(() => {
  applyRange(props.range);
  if (props.initialNote) prefill(props.initialNote);
});

// EDIT mode: seed the form fields from an existing note's W3C body so the user
// tweaks rather than retypes. Best-effort reverse of buildBody() - comment, tag,
// link (element vs document) and workflow.
function prefill(note: Note) {
  // Notes are not validated (see Note), so the body shape is assumed here.
  const body = note?.body as NoteBodyPart | (NoteBodyPart | null)[] | null | undefined;
  const bodies = Array.isArray(body) ? body : body ? [body] : [];
  commentText.value = bodies.find((b) => b?.purpose === 'commenting')?.value ?? '';
  ambiguityTags.value = bodies
    .filter((b): b is NoteBodyPart & { value: string } => b?.purpose === 'tagging' && !!b?.value)
    .map((b) => b.value);
  const docPrefix = 'regelrecht://doc/';
  links.value = bodies
    .filter((b): b is NoteBodyPart & { source: string } => b?.purpose === 'linking' && !!b?.source)
    .map((b): NoteLink => {
      const src = b.source;
      if (src.startsWith(docPrefix)) {
        const rest = src.slice(docPrefix.length);
        const slash = rest.indexOf('/');
        const path = slash >= 0 ? rest.slice(slash + 1) : '';
        return { type: 'document', value: path, label: path };
      }
      const el = (src.split('/').pop() ?? '').split('#')[0] ?? '';
      return { type: 'element', value: el, label: el };
    })
    .filter((l) => l.value);
  const wf = note?.workflow ?? 'none';
  isTask.value = wf === 'open' || wf === 'resolved';
  taskDone.value = wf === 'resolved';
}

// Toggling "task" off also clears "done" - a non-task has no status.
function onTaskChange(e: Event) {
  const event = e as CheckedEvent;
  isTask.value = event.detail?.checked ?? (event.target as { checked?: boolean } | null)?.checked ?? false;
  if (!isTask.value) taskDone.value = false;
}

function reset() {
  commentText.value = '';
  links.value = [];
  ambiguityTags.value = [];
  isTask.value = false;
  taskDone.value = false;
  shareWithTraject.value = false;
  showValidationError.value = false;
}

function cancel() {
  reset();
  emit('cancel');
}

function save() {
  if (!canSave.value) {
    showValidationError.value = true;
    return;
  }
  showValidationError.value = false;
  const note: Note = {
    type: 'Annotation',
    motivation: primaryMotivation(),
    target: {
      source: `regelrecht://${props.lawId}`,
      // canSave is false without a selector result, so it is set here.
      selector: selectorResult.value!.selector,
    },
    body: buildBody(),
  };
  const p = person.value;
  if (p) {
    // `/auth/status` returns the Keycloak subject as `sub` (there is no `id`
    // field); it is the stable per-user identity, so a committed note records
    // its author via creator.id = sub (creator.name is the display name).
    note.creator = { id: p.sub, name: p.name };
  }
  // Action status is optional; only attach a concrete open/resolved value.
  if (workflow.value !== 'none') {
    note.workflow = workflow.value;
  }
  note.__draft = true;
  // Capture the share intent before reset() clears it; the parent shares the
  // note (commits it to the traject) when true, else keeps it a private draft.
  const share = shareWithTraject.value;
  reset();
  emit('create', note, share);
}

// No type picker any more: the primary W3C motivation reflects the main piece
// of content the note carries, in priority order comment > tag > link.
function primaryMotivation() {
  if (hasComment.value) return 'commenting';
  if (hasTag.value) return 'tagging';
  if (hasLink.value) return 'linking';
  return 'commenting';
}

// A note can combine parts, so the body is whatever was filled: the comment,
// the ambiguity tag and/or the link. One part returns a single Body; several
// return an array - both are valid per the annotation schema.
function buildBody() {
  const bodies = [];
  if (hasComment.value) {
    bodies.push({
      type: 'TextualBody',
      value: commentText.value.trim(),
      purpose: 'commenting',
      format: 'text/plain',
      language: 'nl',
    });
  }
  for (const id of ambiguityTags.value) {
    bodies.push({ type: 'TextualBody', value: id, purpose: 'tagging' });
  }
  // One SpecificResource linking body per chosen target. Document URI:
  // `regelrecht://doc/<traject>/<pad>` (the value already holds the in-traject
  // relative path, e.g. `mvt/concept.md`; inner slashes are kept verbatim).
  // Element URI: `regelrecht://<lawId>/<el>#<el>`.
  for (const link of links.value) {
    bodies.push({
      type: 'SpecificResource',
      source:
        link.type === 'document'
          ? `regelrecht://doc/${props.trajectRef}/${link.value}`
          : `regelrecht://${props.lawId}/${link.value}#${link.value}`,
      purpose: 'linking',
    });
  }
  return bodies.length === 1 ? bodies[0] : bodies;
}

// One sharp line per failure reason - what went wrong + the single most
// useful fix. No bullet wall: the reason from buildSelector already
// distinguishes "too common" from "not located", so the message can be
// precise instead of listing every possibility.
const statusInfo = computed(() => {
  if (!selectorStatus.value || selectorStatus.value === 'found') return null;
  switch (selectorReason.value) {
    case 'too-common':
      return {
        title: 'Te algemeen',
        lead: 'Dit fragment komt te vaak voor om aan één plek te koppelen. Selecteer een langere, kenmerkende zinsnede.',
      };
    case 'mis-anchor':
      return {
        title: 'Niet eenduidig',
        lead: 'Dit fragment is hier niet uniek. Breid de selectie uit met de omringende woorden.',
      };
    case 'not-searched':
      // status 'skipped': the resolver hit one of its bounds and never
      // searched. Saying "niet teruggevonden" here would be untrue (nothing
      // was established) and the lidnummer advice would be misdirected.
      // Which bound was hit decides the advice: only 'quote_too_long' is
      // something the author fixes by shortening the selection.
      return selectorSkipReason.value === 'search_budget'
        ? {
            title: 'Niet naar gezocht',
            lead: 'De wettekst is te groot om deze selectie in terug te zoeken. Korter selecteren helpt hier niet.',
          }
        : {
            title: 'Niet naar gezocht',
            lead: 'De selectie is te lang om terug te zoeken. Maak de selectie korter.',
          };
    default: // 'not-found'
      return {
        title: 'Niet teruggevonden',
        lead: 'Selecteer geen lidnummer ("1.") of opsommingsteken mee en blijf binnen één lid.',
      };
  }
});
</script>

<template>
  <nldd-container v-if="range" padding="16" data-testid="note-creator">
      <!-- Referenced fragment, shown the same way as everywhere else. -->
      <QuotedFragment :fragment="referencedFragment" />

      <!-- Unusable selection (ambiguous/orphaned): show just the warning; the
           user dismisses by clicking outside. The full form appears once the
           selection resolves uniquely. -->
      <template v-if="statusInfo">
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-banner
          variant="warning"
          :text="statusInfo.title"
          :supporting-text="statusInfo.lead"
          data-testid="note-creator-status"
        ></nldd-banner>
      </template>

      <template v-else>
        <!-- Submit with nothing filled surfaces this banner instead of a
             disabled button, so the primary action always stays clickable. -->
        <template v-if="showValidationError">
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-banner
            variant="critical"
            text="Vul iets in of maak een keuze om een notitie toe te voegen."
            data-testid="note-validation-error"
          ></nldd-banner>
        </template>

        <nldd-spacer size="16"></nldd-spacer>
        <!-- 1. Opmerking (comment body). Label lives in the placeholder +
             accessible-label to save vertical space in the popover. -->
        <nldd-form-field>
          <nldd-text-editor
            ref="commentFieldEl"
            :rows="3"
            resize="auto"
            :value="commentText"
            placeholder="Opmerking"
            accessible-label="Opmerking"
            data-testid="note-comment-text"
            @input="commentText = $event.target?.value ?? $event.detail?.value ?? commentText"
          ></nldd-text-editor>
        </nldd-form-field>

        <!-- 2. Ambiguïteit-labels (multi-select): a token-field whose options are
             the vocabulary; chosen labels show as dismissible tokens. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-field label="Labels">
          <nldd-token-field
            accessible-label="Labels"
            placeholder="Label toevoegen…"
            data-testid="note-ambiguity-field"
            .values="ambiguityTags"
            @change="onTagsChange"
          >
            <nldd-menu>
              <nldd-menu-item
                v-for="t in ambiguityItems"
                :key="t.id"
                :text="t.label"
                :value="t.id"
              ></nldd-menu-item>
            </nldd-menu>
          </nldd-token-field>
        </nldd-form-field>

        <!-- 3. Koppel aan. A token-field over one type-tagged, filterable list of
             elements and traject documents; chosen targets show as tokens. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-field label="Koppel aan">
          <nldd-token-field
            accessible-label="Koppel aan element of document"
            placeholder="Element of document toevoegen…"
            data-testid="note-link-field"
            .values="linkValues"
            @change="onLinksChange"
          >
            <nldd-menu>
              <nldd-menu-item
                v-for="opt in linkOptions"
                :key="opt.key"
                :text="opt.label"
                :value="opt.key"
                :icon="opt.icon"
              ></nldd-menu-item>
            </nldd-menu>
          </nldd-token-field>
          <span v-if="documentsLoadError" class="nc-hint">
            Documenten kunnen niet worden geladen ({{ documentsLoadError.message }}).
          </span>
        </nldd-form-field>

        <!-- 4. Taak + delen. The three switches share one form-field, separated
             by small spacers: task-for-myself, (when a task) done, and (with an
             active traject) share the note - sharing is irreversible. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-field>
          <nldd-switch-field
            label="Taak voor mezelf"
            :checked="isTask"
            data-testid="note-task"
            @change="onTaskChange($event)"
          ></nldd-switch-field>
          <template v-if="isTask">
            <nldd-spacer size="4"></nldd-spacer>
            <nldd-switch-field
              label="Taak is afgehandeld"
              :checked="taskDone"
              data-testid="note-task-done"
              @change="taskDone = $event.detail?.checked ?? $event.target?.checked ?? false"
            ></nldd-switch-field>
          </template>
          <template v-if="trajectRef">
            <nldd-spacer size="4"></nldd-spacer>
            <nldd-switch-field
              label="Deel notitie met anderen in traject"
              :checked="shareWithTraject"
              data-testid="note-share"
              @change="shareWithTraject = $event.detail?.checked ?? $event.target?.checked ?? false"
            ></nldd-switch-field>
            <template v-if="shareWithTraject">
              <nldd-spacer size="4"></nldd-spacer>
              <nldd-banner
                variant="warning"
                data-testid="note-share-warning"
                text="Delen kan niet ongedaan worden gemaakt"
                supporting-text="De notitie kan daarna niet meer privé worden gemaakt of gewijzigd."
              ></nldd-banner>
            </template>
          </template>
        </nldd-form-field>

        <!-- Author is the signed-in user; no field. Full-width primary action;
             the user cancels by clicking outside the popover. The button never
             disables - an empty submit shows the banner above instead. -->
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-form-actions>
          <nldd-button
            size="md"
            appearance="primary"
            width="full"
            :text="isEditing ? 'Werk notitie bij' : 'Voeg notitie toe'"
            data-testid="note-save"
            @click="save"
          ></nldd-button>
        </nldd-form-actions>
      </template>
    </nldd-container>
</template>

<style scoped>
.nc-hint {
  font-size: 0.74rem;
  opacity: 0.6;
}
</style>
