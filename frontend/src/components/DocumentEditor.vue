<script setup lang="ts">
/**
 * DocumentEditor - the active-document editor, driven by a useDocumentsManager
 * instance passed in as `manager`. Renders:
 *  - a sticky top toolbar: document name + rename/delete menu (start), and the
 *    Save button + revert menu (end) shown only while there are unsaved changes;
 *  - the hybrid Markdown editor body;
 *  - a sticky bottom toolbar: the full text-editor formatting palette from the
 *    DS "Mixed" story, shown whenever the editor body is available;
 *  - a rename sheet, and modal dialogs for save/conflict/delete failures.
 *
 * The formatting toolbar (page footer) and the editor (page body) sit in
 * different page slots; the imported toolbar wiring resolves the editor via
 * their common nldd-page ancestor and reflects its state onto the controls.
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { Ref, WatchSource } from 'vue';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { NLDDTextField } from '@nldd/design-system/text-field';
import {
  reconcile,
  onListChange,
  onHeadingSelect,
  onLink,
  onIndent,
  onOutdent,
  onUndo,
  onRedo,
  onCopy,
  onCut,
  onPaste,
  runCommand,
  onToolbarState,
  attachOverflowSelectListener,
} from '../lib/editorToolbar';
import { paneChromeVisible } from '../constants';
import type { useDocumentsManager } from '../composables/useDocumentsManager';

type DocumentsManager = ReturnType<typeof useDocumentsManager>;

const props = defineProps<{
  // `& object` keeps the runtime prop check at `Object`: the compiler cannot
  // resolve ReturnType and would otherwise drop the check.
  manager: DocumentsManager & object;
}>();
const emit = defineEmits<{
  deleted: [];
  back: [];
  saved: [path: string | null];
}>();

const m = props.manager;
const {
  currentPath,
  currentBody,
  hasChanges,
  docLoading,
  docError,
  saving,
  saveError,
  conflict,
  deletedRemotely,
  creating,
  titleDraft,
  titleError,
  pendingDeletePath,
  deleteNotice,
  displayTitle,
  onBodyInput,
  onTitleInput,
  validateRename,
  handleSave,
  undoChanges,
  overwriteServer,
  reloadCurrent,
  askDelete,
  cancelDelete,
  confirmDelete,
} = m;

// Document name shown in the top toolbar title.
const docName = computed(() => displayTitle(currentPath.value) || 'Document');

// A load failure leaves nothing to edit, so it replaces the whole body as an
// inline dialog. Save / conflict / delete failures are modals and a
// deleted-on-server notice is a banner (below); a 'draft-present' docError is
// not surfaced (the top Save button signals unsaved changes).
const blockingError = computed(() => {
  if (docError.value && docError.value.kind !== 'draft-present') {
    return { text: docError.value.message, supporting: null };
  }
  return null;
});

// The formatting footer follows the shared pane-loading strategy
// (paneChromeVisible), so it makes the same call as every other pane rather
// than its own: with SHOW_PANE_CHROME_WHILE_LOADING on (constants.js) it stays
// up while a document loads or is created, and flipping that one switch off
// collapses this pane - like all of them - to a bare "Document laden" indicator
// with no title, actions or footer. The footer also hides on a load error
// (nothing to format then), whatever the switch says.
const showFormattingToolbar = computed(
  () => !blockingError.value && paneChromeVisible(docLoading.value || creating.value),
);

// The formatting toolbar sits in the page footer and the editor in the page
// body (different page slots), so the two-way state sync listens on their
// common nldd-page ancestor. The overflow menu-items are cloned into
// document.body (losing their listeners); a single delegated `select` listener
// runs their action.
const editorSectionEl = ref<HTMLElement | null>(null);
let statePage: Element | null = null;
onMounted(() => {
  attachOverflowSelectListener();
  statePage = editorSectionEl.value?.closest('nldd-page') ?? null;
  statePage?.addEventListener('state-change', onToolbarState);
});
onUnmounted(() => {
  statePage?.removeEventListener('state-change', onToolbarState);
});

// --- Saving ---
// The top-bar Save (and Ctrl/Cmd+S) only persist the body; renaming happens in
// the sheet. Reset titleDraft to the current name first so a name left half-
// edited in a dismissed rename sheet can never trigger an accidental rename.
// Emits 'saved' on success - the save-success hook a document-task review
// (useDocumentTaskReview, wired in LibraryView.vue) uses to resolve the task
// as approved, mirroring how EditorView's law-review approves after its own
// save succeeds. The payload is the path that was under review WHEN THE SAVE
// STARTED (captured before `handleSave`, which can move `currentPath` for a
// rename) - LibraryView's `onDocSaved` gates on that path matching the
// task's `target_path`, not on wherever the document ended up.
async function saveDocument() {
  if (saving.value || !currentPath.value) return false;
  const savedPath = currentPath.value;
  titleDraft.value = displayTitle(currentPath.value);
  const ok = await handleSave();
  if (ok) emit('saved', savedPath);
  return ok;
}

// Exposed so the parent's leave-guard can offer a "save and close" action
// (returns whether the save succeeded, so the parent only leaves on success).
defineExpose({ saveDocument });

function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault();
    saveDocument();
  }
}

// --- Rename sheet ---
const renameSheetEl = ref<NLDDSheet | null>(null);
const renameFieldEl = ref<NLDDTextField | null>(null);
async function openRename() {
  titleDraft.value = displayTitle(currentPath.value);
  titleError.value = null;
  renameSheetEl.value?.show?.();
  // The sheet moves focus to its own dialog on open, so focus the name field on
  // the next tick to win it back (nldd-text-field delegates focus to its input).
  await nextTick();
  renameFieldEl.value?.focus?.();
}
// Also emits 'saved' (see `saveDocument` above): renaming still commits
// whatever body was in the editor, so it must still trip the review-approve
// hook when that body is the one under review - even though the document
// may end up at a different path than `target_path`, `savedPath` (captured
// before the rename) is what LibraryView's gate compares against.
async function saveRename() {
  // Optimistic: a valid name closes the sheet immediately and commits in the
  // background (the rename is two slow GitHub writes). An invalid/duplicate name
  // keeps the sheet open with the error. handleSave updates the name optimistically
  // and rolls back + surfaces a save error if the server rejects it.
  if (!validateRename()) return;
  const savedPath = currentPath.value;
  renameSheetEl.value?.hide?.();
  if (await handleSave()) emit('saved', savedPath);
}

// --- Delete ---
async function onConfirmDelete() {
  const wasOpenDocument = await confirmDelete();
  if (wasOpenDocument) emit('deleted');
}

// --- Imperative dialogs: show/hide a modal from a reactive source ---
function bindModal(source: WatchSource<unknown>, modalRef: Ref<NLDDModalDialog | null>) {
  watch(source, async (v) => {
    await nextTick();
    const el = modalRef.value;
    if (!el) return;
    if (v) el.show?.();
    else el.hide?.();
  });
}

const deleteModalEl = ref<NLDDModalDialog | null>(null);
bindModal(pendingDeletePath, deleteModalEl);

// A failed save or delete has one way out, so it is a critical notification
// (it stays until dismissed) rather than a dialog with nothing to choose.
function dismissSaveError() {
  saveError.value = null;
}

const conflictModalEl = ref<NLDDModalDialog | null>(null);
bindModal(conflict, conflictModalEl);
function dismissConflict() {
  conflict.value = null;
}
function conflictReload() {
  conflict.value = null;
  reloadCurrent();
}
// Async + emits 'saved' on success (see `saveDocument` above): "Lokaal
// overschrijven" commits the in-editor body via a force-write, bypassing
// `handleSave`/`saveDocument` entirely - without this the review-approve
// hook would never fire for a conflict resolved this way.
async function conflictOverwrite() {
  conflict.value = null;
  const savedPath = currentPath.value;
  const result = await overwriteServer();
  if (result?.ok) emit('saved', savedPath);
}

function dismissDeleteNotice() {
  deleteNotice.value = null;
}
</script>

<template>
  <!-- Sticky top toolbar: document name + actions menu (start), and Save +
       revert (end) shown only while there are unsaved changes. -->
  <nldd-container slot="header" padding-inline="12" padding-top="12" sm-padding-inline="8" sm-padding-top="8">
    <nldd-toolbar size="md">
        <!-- Back to the document list, shown only while the sidebar is stacked
             away. --context-back-button-display comes from the split-view pane
             ('none' when the sidebar is visible) - the same signal nldd-top-title-bar
             uses. :not([hidden]) yields to the toolbar's own overflow hiding. -->
        <nldd-toolbar-item slot="start" class="wd-back">
          <nldd-icon-button icon="chevron-left" text="Terug naar werkdocumenten" tooltip-timing="never" @click="$emit('back')"></nldd-icon-button>
          <nldd-menu-item slot="overflow" icon="chevron-left" text="Terug naar werkdocumenten" @select="$emit('back')"></nldd-menu-item>
        </nldd-toolbar-item>
        <!-- Document title with an integrated xs action button; its rename/delete
             menu is teleported to body and anchored to it by id. -->
        <nldd-toolbar-title v-if="paneChromeVisible(docLoading || creating)" slot="center" align="center" :text="docName">
          <!-- Same icon as the row in the werkdocumenten list, so the title
               echoes what you clicked. Decorative: the name is right beside it. -->
          <nldd-icon slot="media" icon="text-document" size="20"></nldd-icon>
          <nldd-icon-button
            slot="action"
            id="document-actions-btn"
            size="xs"
            icon="chevron-down"
            text="Documentacties"
            tooltip-timing="never"
            popovertarget="document-actions-menu"
          ></nldd-icon-button>
        </nldd-toolbar-title>

        <!-- Save + revert appear only while there are unsaved changes. -->
        <nldd-toolbar-item v-if="hasChanges" slot="end">
          <nldd-button
            appearance="primary"
            size="md"
            text="Opslaan"
            :loading="saving || undefined"
            @click="saveDocument"
          ></nldd-button>
          <nldd-menu-item slot="overflow" icon="save" text="Opslaan" :disabled="saving || undefined" @select="saveDocument"></nldd-menu-item>
        </nldd-toolbar-item>
        <nldd-toolbar-item v-if="hasChanges" slot="end">
          <nldd-icon-button size="md" icon="more" text="Meer" tooltip-timing="never">
            <nldd-menu slot="popup">
              <nldd-menu-item text="Maak alle wijzigingen ongedaan" icon="undo" @click="undoChanges"></nldd-menu-item>
            </nldd-menu>
          </nldd-icon-button>
          <nldd-menu-item slot="overflow" text="Maak alle wijzigingen ongedaan" icon="undo" @select="undoChanges"></nldd-menu-item>
        </nldd-toolbar-item>
      </nldd-toolbar>
    </nldd-container>

  <nldd-simple-section ref="editorSectionEl" width="800px" @keydown="handleKeydown">
      <nldd-activity-indicator v-if="docLoading || creating" text="Document laden" show-text></nldd-activity-indicator>
      <!-- A load failure leaves nothing useful behind it, so it replaces the body. -->
      <nldd-inline-dialog
        v-else-if="blockingError"
        variant="alert"
        :text="blockingError.text"
        :supporting-text="blockingError.supporting || undefined"
      ></nldd-inline-dialog>
      <template v-else>
        <!-- Temporary: deleted-on-server stays a banner above the editor for now. -->
        <nldd-banner v-if="deletedRemotely" variant="warning" :text="deletedRemotely"></nldd-banner>
        <!-- Hybrid Markdown editor: live-styled source, no separate preview. -->
        <nldd-text-editor
          embedded
          :rows="12"
          resize="auto"
          :value="currentBody"
          accessible-label="Document-inhoud (markdown)"
          placeholder="# Titel"
          @input="onBodyInput"
        ></nldd-text-editor>
      </template>
  </nldd-simple-section>

  <!-- Sticky bottom toolbar: the full text-editing palette, shown whenever the
       editor body is available. Formatting toolbar from the DS "Mixed" story;
       controls are uncontrolled and onToolbarState reflects the editor state. -->
  <nldd-container v-if="showFormattingToolbar" slot="footer" padding-inline="12" padding-top="0" padding-bottom="12" sm-padding-inline="8" sm-padding-bottom="8">
    <nldd-toolbar size="md">
      <!-- Overflow priority (high stays longest, low overflows first):
           text-formatting > block-type > list > indent > link > quote >
           code > history > clipboard. Set explicitly so the collapse order is
           independent of the start/end visual split. -->
      <nldd-toolbar-item slot="start" label="Nadruk" priority="9">
        <nldd-segmented-control
          data-group="inline"
          type="checkbox"
          variant="icon"
          accessible-label="Nadruk"
          @change="(e: Event) => reconcile(e.currentTarget, ['bold', 'italic', 'strikethrough'], (e as CustomEvent<{ values: string[] }>).detail.values)"
        >
          <nldd-segmented-control-item value="bold" text="Vet" icon="bold"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="italic" text="Cursief" icon="italic"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="strikethrough" text="Doorhalen" icon="strikethrough"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-menu-group slot="overflow" text="Nadruk">
          <nldd-menu-item type="checkbox" value="bold" text="Vet" icon="bold"></nldd-menu-item>
          <nldd-menu-item type="checkbox" value="italic" text="Cursief" icon="italic"></nldd-menu-item>
          <nldd-menu-item type="checkbox" value="strikethrough" text="Doorhalen" icon="strikethrough"></nldd-menu-item>
        </nldd-menu-group>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Code" priority="3">
        <nldd-toggle-button data-group="code" variant="icon" icon="code" accessible-label="Code" @change="(e: Event) => runCommand(e, 'inlineCode')"></nldd-toggle-button>
        <nldd-menu-item slot="overflow" type="checkbox" value="inlineCode" text="Code" icon="code"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Link" priority="5">
        <nldd-toggle-button data-group="link" variant="icon" icon="link" accessible-label="Link" @change="onLink"></nldd-toggle-button>
        <nldd-menu-item slot="overflow" type="checkbox" value="link" text="Link" icon="link"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Citaat" priority="4">
        <nldd-toggle-button data-group="quote" variant="icon" icon="text-quote" accessible-label="Citaat" @change="(e: Event) => runCommand(e, 'quote')"></nldd-toggle-button>
        <nldd-menu-item slot="overflow" type="checkbox" value="quote" text="Citaat" icon="text-quote"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Lijst" priority="7">
        <nldd-segmented-control data-group="list" type="radio" variant="icon" value="none" accessible-label="Lijst" @change="onListChange">
          <nldd-segmented-control-item value="none" text="Geen lijst" icon="minus"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="bullet" text="Opsomming" icon="bullet-list"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="numbered" text="Genummerd" icon="numbered-list"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-menu-group slot="overflow" text="Lijst">
          <nldd-menu-item type="radio" value="list:none" text="Geen lijst" icon="minus"></nldd-menu-item>
          <nldd-menu-item type="radio" value="list:bullet" text="Opsomming" icon="bullet-list"></nldd-menu-item>
          <nldd-menu-item type="radio" value="list:numbered" text="Genummerd" icon="numbered-list"></nldd-menu-item>
        </nldd-menu-group>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Inspringen" priority="6">
        <nldd-button-bar data-group="indent">
          <nldd-icon-button icon="indent-increase" text="Meer inspringen" @click="onIndent"></nldd-icon-button>
          <nldd-button-bar-divider></nldd-button-bar-divider>
          <nldd-icon-button icon="indent-decrease" text="Minder inspringen" @click="onOutdent"></nldd-icon-button>
        </nldd-button-bar>
        <nldd-menu-group slot="overflow" text="Inspringen">
          <nldd-menu-item value="indent" text="Meer inspringen" icon="indent-increase"></nldd-menu-item>
          <nldd-menu-item value="outdent" text="Minder inspringen" icon="indent-decrease"></nldd-menu-item>
        </nldd-menu-group>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Tekststijl" priority="8">
        <nldd-button data-group="heading" expandable text="Paragraaf">
          <nldd-menu slot="popup" @select="onHeadingSelect">
            <nldd-menu-item type="radio" value="0" text="Paragraaf" selected></nldd-menu-item>
            <nldd-menu-divider></nldd-menu-divider>
            <nldd-menu-item type="radio" value="1" text="Heading 1"></nldd-menu-item>
            <nldd-menu-item type="radio" value="2" text="Heading 2"></nldd-menu-item>
            <nldd-menu-item type="radio" value="3" text="Heading 3"></nldd-menu-item>
            <nldd-menu-item type="radio" value="4" text="Heading 4"></nldd-menu-item>
            <nldd-menu-item type="radio" value="5" text="Heading 5"></nldd-menu-item>
            <nldd-menu-item type="radio" value="6" text="Heading 6"></nldd-menu-item>
            <nldd-menu-divider></nldd-menu-divider>
            <nldd-menu-item type="radio" value="codeblock" text="Codeblok"></nldd-menu-item>
          </nldd-menu>
        </nldd-button>
        <!-- In the overflow menu the text styles collapse into a submenu (a
             nested nldd-menu) instead of a flat labelled group. -->
        <nldd-menu-item slot="overflow" text="Tekststijl">
          <nldd-menu>
            <nldd-menu-item type="radio" value="heading:0" text="Paragraaf"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:1" text="Heading 1"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:2" text="Heading 2"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:3" text="Heading 3"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:4" text="Heading 4"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:5" text="Heading 5"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:6" text="Heading 6"></nldd-menu-item>
            <nldd-menu-item type="radio" value="heading:codeblock" text="Codeblok"></nldd-menu-item>
          </nldd-menu>
        </nldd-menu-item>
      </nldd-toolbar-item>
      <!-- Clipboard + history sit apart on the right. -->
      <nldd-toolbar-item slot="end" label="Klembord" priority="1">
        <nldd-button-bar data-group="clipboard">
          <nldd-icon-button icon="copy" text="Kopieer" @click="onCopy"></nldd-icon-button>
          <nldd-button-bar-divider></nldd-button-bar-divider>
          <nldd-icon-button icon="cut" text="Knip" @click="onCut"></nldd-icon-button>
          <nldd-button-bar-divider></nldd-button-bar-divider>
          <nldd-icon-button icon="paste" text="Plak" @click="onPaste"></nldd-icon-button>
        </nldd-button-bar>
        <nldd-menu-group slot="overflow" text="Klembord">
          <nldd-menu-item value="copy" text="Kopieer" icon="copy"></nldd-menu-item>
          <nldd-menu-item value="cut" text="Knip" icon="cut"></nldd-menu-item>
          <nldd-menu-item value="paste" text="Plak" icon="paste"></nldd-menu-item>
        </nldd-menu-group>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="end" label="Geschiedenis" priority="2">
        <nldd-button-bar data-group="history">
          <nldd-icon-button icon="undo" text="Maak ongedaan" @click="onUndo"></nldd-icon-button>
          <nldd-button-bar-divider></nldd-button-bar-divider>
          <nldd-icon-button icon="redo" text="Voer opnieuw uit" @click="onRedo"></nldd-icon-button>
        </nldd-button-bar>
        <nldd-menu-group slot="overflow" text="Geschiedenis">
          <nldd-menu-item value="undo" text="Maak ongedaan" icon="undo"></nldd-menu-item>
          <nldd-menu-item value="redo" text="Voer opnieuw uit" icon="redo"></nldd-menu-item>
        </nldd-menu-group>
      </nldd-toolbar-item>
    </nldd-toolbar>
  </nldd-container>

  <!-- Document-actions menu, anchored by id to the title's action button. -->
  <Teleport to="body">
    <nldd-menu id="document-actions-menu" anchor="document-actions-btn">
      <nldd-menu-item text="Naam wijzigen" icon="edit" @click="openRename"></nldd-menu-item>
      <nldd-menu-divider></nldd-menu-divider>
      <nldd-menu-item text="Document verwijderen" icon="delete" destructive @click="askDelete(currentPath)"></nldd-menu-item>
    </nldd-menu>
  </Teleport>

  <!-- Rename in a sheet. nldd-form wraps a native <form> (framework-friendly
       mode); the submit button drives it via form association, and errors
       surface in the form-field's validation list. -->
  <Teleport to="body">
    <nldd-sheet ref="renameSheetEl">
      <nldd-page>
        <nldd-top-title-bar slot="header" text="Naam wijzigen" dismiss-text="Annuleer"></nldd-top-title-bar>
        <nldd-simple-section>
          <nldd-form>
            <form @submit.prevent="saveRename">
              <nldd-form-field label="Documentnaam">
                <nldd-form-field-help-text>Geen hoofdletters of spaties: die worden automatisch omgezet naar kleine letters en koppeltekens.</nldd-form-field-help-text>
                <nldd-text-field
                  ref="renameFieldEl"
                  :value="titleDraft"
                  :invalid="titleError ? true : undefined"
                  :unmet="titleError ? 'rename-error' : undefined"
                  accessible-label="Documentnaam"
                  placeholder="documentnaam"
                  @input="onTitleInput"
                ></nldd-text-field>
                <nldd-validation-list>
                  <nldd-validation-item id="rename-error">{{ titleError }}</nldd-validation-item>
                </nldd-validation-list>
              </nldd-form-field>
              <nldd-form-actions>
                <nldd-button-group orientation="vertical">
                  <nldd-button appearance="primary" type="submit" text="Opslaan" :loading="saving || undefined"></nldd-button>
                </nldd-button-group>
              </nldd-form-actions>
            </form>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

  <!-- Action-failure dialogs (shown imperatively from reactive state). -->
  <Teleport to="body">
    <nldd-notification v-if="saveError" variant="critical" text="Opslaan mislukt" :supporting-text="saveError.message || undefined" @dismiss="dismissSaveError"></nldd-notification>

    <nldd-modal-dialog ref="conflictModalEl" variant="alert" text="Document is gewijzigd" :supporting-text="conflict || undefined" @close="dismissConflict">
      <nldd-button slot="actions" text="Server-versie laden" @click="conflictReload"></nldd-button>
      <nldd-button slot="actions" appearance="primary" text="Lokaal overschrijven" @click="conflictOverwrite"></nldd-button>
    </nldd-modal-dialog>

    <nldd-notification v-if="deleteNotice" variant="critical" text="Verwijderen mislukt" :supporting-text="deleteNotice" @dismiss="dismissDeleteNotice"></nldd-notification>

    <nldd-modal-dialog
      ref="deleteModalEl"
      variant="alert"
      :text="pendingDeletePath ? `${displayTitle(pendingDeletePath)} verwijderen?` : 'Document verwijderen?'"
      supporting-text="Het document wordt definitief uit het traject verwijderd. Dit kan niet ongedaan worden gemaakt."
      @close="cancelDelete"
    >
      <nldd-button slot="actions" appearance="primary" text="Behoud document" @click="cancelDelete"></nldd-button>
      <nldd-button slot="actions" appearance="destructive" text="Verwijder" @click="onConfirmDelete"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>

<style scoped>
/* The back item shows only when the split-view pane has dropped its hide-back
   state (the sidebar is stacked away); while the sidebar is visible the pane
   sets --context-back-button-display: none. :not([hidden]) yields to the
   toolbar's own overflow hiding. */
.wd-back:not([hidden]) {
  display: var(--context-back-button-display, inline-flex);
}
</style>
