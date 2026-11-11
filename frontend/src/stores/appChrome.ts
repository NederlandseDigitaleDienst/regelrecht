import { shallowRef, onBeforeUnmount, type Ref } from 'vue';
import { defineStore } from 'pinia';
import type { OpenTab } from '../lib/openTabsStorage';
import type { SavedPr } from '../composables/useSavedPr';

// What the shell needs from a view's SearchPopover instance (its `defineExpose`).
interface SearchPopoverHandle {
  show: (anchorEl: EventTarget | null | undefined, initialSearch: string, event: Event | undefined) => unknown;
}

type SearchPopoverRef = Readonly<Ref<SearchPopoverHandle | null | undefined>>;

/** The editor's actions for the document tab bar. */
export interface TabActions {
  key: (tab: OpenTab) => string;
  displayName: (tab: OpenTab) => string;
  /** Resolves to false when the editor kept the open article (unsaved edits the user chose to keep). */
  select: (tab: OpenTab) => Promise<boolean>;
  /** Resolves to false when the editor kept the tab open, for the same reason. */
  close: (tab: OpenTab, next?: OpenTab | null) => Promise<boolean>;
  reorder: (fromIndex: number, toIndex: number) => void;
}

/** The article's pending-changes state, as the changes bar renders it. */
export interface EditorChanges {
  dirty: boolean;
  saving: boolean;
  canUndo: boolean;
  canRedo: boolean;
  review: boolean;
  reviewStatus: string | null;
  reviewVariant?: string | null;
  reviewCanRejectRest: boolean;
}

/** Callbacks for the changes bar; the shell calls each one optionally. */
export interface EditorActions {
  save?: () => unknown;
  discard?: () => unknown;
  undo?: () => unknown;
  redo?: () => unknown;
  reject?: () => unknown;
  rejectRest?: () => unknown;
}

export interface EditorChrome {
  pr?: SavedPr | null;
  tabs?: readonly OpenTab[] | null;
  activeTab?: OpenTab | null;
  trajectRef?: string | null;
}

// The shell's chrome, as a store the whole editor shares.
//
// The Bibliotheek/Editor switch keeps the shell (toolbars, tab-bar,
// settings menu) mounted and only swaps the nested <router-view>. A few
// toolbar bits are view-specific - the search trigger (both views), the
// federated "PR #N" write-back indicator and the document-tab-bar (editor
// only). Rather than teleport those into the shell's toolbars - which
// `nldd-toolbar` rejects, since slotted items must be its *direct*
// children and teleport would reorder them - the active view publishes
// them here and the shell renders everything itself, in the original
// order.
export const useAppChromeStore = defineStore('appChrome', () => {
  // --- Search (both views) ---
  // The toolbar search button/field lives in the shell; the SearchPopover it
  // opens stays in each view (its `@select-law` handling differs). The view
  // registers its popover ref; the shell calls openSearch with the click/key
  // event so the popover anchors to the bar control. The ref is a component
  // handle, not state, so it stays in this closure and out of the store's
  // state (and out of devtools).
  let popoverRef: SearchPopoverRef | null = null;

  /** Register the view's search popover. Call it during component setup: the registration ends when that component unmounts. */
  function registerSearchPopover(ref_: SearchPopoverRef): void {
    popoverRef = ref_;
    // Clear on unmount, but only if this view still owns the registration. The
    // entering view registers in its setup before the leaving view unmounts, so
    // guarding on identity avoids nulling a registration that already moved on -
    // making the cleanup safe even if the route topology changes that ordering.
    onBeforeUnmount(() => {
      if (popoverRef === ref_) popoverRef = null;
    });
  }

  function openSearch(e?: Event, initialSearch = ''): void {
    popoverRef?.value?.show(e?.currentTarget, initialSearch, e);
  }

  /**
   * Opens the search popover against `anchor` rather than an event's target. The
   * toolbar's overflow menu needs this: its `select` arrives on the original
   * menu item, which sits hidden in its collapsed toolbar item.
   */
  function openSearchAt(anchor: Element | null): void {
    popoverRef?.value?.show(anchor, '', undefined);
  }

  /**
   * Spotlight-style: any printable single-character keystroke on the bar's
   * search-field opens the popover with that character as the initial query.
   * Modifier-combos (Ctrl-A, Cmd-V, …), Tab, Enter, arrows fall through.
   */
  function onBarSearchKeydown(e: KeyboardEvent): void {
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      openSearch(e, e.key);
    }
  }

  // --- Editor-only chrome ---
  // Federated write-back indicator ({ number, url } | null).
  const lastSavedPr = shallowRef<SavedPr | null>(null);
  // Open document tabs + the active one, plus the actions to drive them. The
  // editor keeps these in sync via a watchEffect; the shell renders the
  // `document-tabs` pane only while `documentTabs` is non-empty, so the
  // library never shows an empty tab bar.
  const documentTabs = shallowRef<readonly OpenTab[]>([]);
  const activeDocumentTab = shallowRef<OpenTab | null>(null);
  const tabActions = shallowRef<TabActions | null>(null);
  // The traject ref the current `documentTabs` belong to, published in the SAME
  // update as the tabs. The shell keys its `<nldd-document-tab-bar>` rebuild on
  // this (not on its own `activeTrajectRef`, which flips a tick earlier than the
  // tabs and would rebuild the bar against the previous traject's set, leaving a
  // spooktab). A rebuild tears down the bar's overflow menu, ResizeObserver and
  // every element reference it holds.
  const documentTabsTrajectRef = shallowRef<string | null>(null);
  // Article-level pending-changes bar (Wijzigingenbalk). The editor publishes
  // the dirty/saving/undo state reactively and registers the action callbacks;
  // the shell renders the bar while there are unsaved changes.
  // `review` puts the bar in proposal-review mode: it then shows even when the
  // panes are not dirty (a proposal that seeded nothing visible is still a
  // decision waiting to be made) and it carries a Verwerp action next to Opslaan.
  // `reviewStatus`/`reviewVariant` feed the status bar the shell renders above
  // `main`: it belongs to the bar-split-view, not to the editor's own content
  // flow, so it lines up with the tab bar and the changes bar instead of
  // scrolling with the panes.
  // `reviewCanRejectRest` says whether more than one undecided part is left;
  // the bar then offers "finish, don't take the rest", so an enrichment can be
  // finished without going through every part.
  const editorChanges = shallowRef<EditorChanges | null>(null);
  const editorActions = shallowRef<EditorActions | null>(null);

  // --- Library-only chrome ---
  // Whether the library has nothing curated yet (no favorites, no traject edits,
  // no open law). While true, the shell shows the just-in-time search coach-mark
  // on the toolbar search field; the library body itself stays blank.
  const libraryEmpty = shallowRef(false);

  // Published reactively by the library view (and reset when it unmounts) so the
  // shell can gate the search coach-mark on an empty library.
  function setLibraryEmpty(empty: boolean | null | undefined): void {
    libraryEmpty.value = !!empty;
  }

  function setEditorChrome({ pr, tabs, activeTab, trajectRef }: EditorChrome): void {
    lastSavedPr.value = pr ?? null;
    documentTabs.value = tabs ?? [];
    activeDocumentTab.value = activeTab ?? null;
    documentTabsTrajectRef.value = trajectRef ?? null;
  }

  function registerTabActions(actions: TabActions | null): void {
    tabActions.value = actions;
  }

  // Reactive snapshot of the article's pending-changes state. Called from a
  // watchEffect in the editor so a new object is published whenever dirty,
  // saving or undo-availability changes, re-rendering the bar.
  function setEditorChanges(state: EditorChanges | null | undefined): void {
    editorChanges.value = state ?? null;
  }

  // Stable action callbacks for the changes bar; registered once.
  function registerEditorActions(actions: EditorActions | null | undefined): void {
    editorActions.value = actions ?? null;
  }

  // Called when the editor view unmounts so the shell drops the editor-only
  // chrome (PR badge, document tabs, changes bar) while the library is mounted.
  function clearEditorChrome(): void {
    lastSavedPr.value = null;
    documentTabs.value = [];
    activeDocumentTab.value = null;
    documentTabsTrajectRef.value = null;
    tabActions.value = null;
    editorChanges.value = null;
    editorActions.value = null;
  }

  return {
    lastSavedPr,
    documentTabs,
    activeDocumentTab,
    documentTabsTrajectRef,
    tabActions,
    editorChanges,
    editorActions,
    libraryEmpty,
    registerSearchPopover,
    openSearch,
    openSearchAt,
    onBarSearchKeydown,
    setLibraryEmpty,
    setEditorChrome,
    registerTabActions,
    setEditorChanges,
    registerEditorActions,
    clearEditorChrome,
  };
});
