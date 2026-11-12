<script setup lang="ts">
import { ref, reactive } from 'vue';
import type { Annotation, NLDDTextEditor, TextEditorState } from '@nldd/design-system/text-editor';

/** Detail of `annotation-click` (not typed by the design system). */
interface AnnotationClickDetail {
  ids: string[];
  rect: DOMRect;
}

const props = withDefaults(defineProps<{
  // Only its presence is read.
  article?: object | null;
  editable?: boolean;
  /**
   * Markdown source, v-model-bound to the parent editor state. The
   * nldd-text-editor works in Markdown directly: its `value` is the Markdown
   * document (annotation sentinels stripped), so this round-trips
   * Markdown ↔ Markdown with no HTML intermediate - unlike the previous
   * Tiptap editor, raw HTML in the source is preserved as literal text.
   */
  modelValue?: string;
  /**
   * Annotations to overlay, in the DS shape { id, start, end, quote } with
   * UTF-16 offsets into the (clean) Markdown. The editor maps them through
   * edits; read them back with getAnnotations() on save.
   */
  annotations?: Annotation[];
}>(), {
  article: null,
  editable: false,
  modelValue: '',
  annotations: () => [],
});

const emit = defineEmits<{
  'update:modelValue': [value: string];
  focus: [];
  'annotation-click': [ids: string[], rect: DOMRect];
}>();

const editorRef = ref<NLDDTextEditor | null>(null);

// Reactive mirror of the editor's toolbar state, fed by the
// `state-change` event. The parent's header toolbar reads
// activeFormats/canUndo/canRedo off the exposed refs to drive the format
// segmented-controls and the article-level undo/redo bar.
const activeFormats = reactive({
  bold: false,
  italic: false,
  bulletList: false,
  orderedList: false,
  // Whether the current selection can nest deeper (a preceding sibling to nest
  // under) / outdent (already nested). Drive the toolbar's indent buttons.
  canIndent: false,
  canOutdent: false,
});
const canUndo = ref(false);
const canRedo = ref(false);
// Drives the toolbar "comment" button: authoring needs a non-empty selection.
const selectionEmpty = ref(true);

function onState(e: Event) {
  const s = (e as CustomEvent<TextEditorState>).detail;
  activeFormats.bold = s.active.bold;
  activeFormats.italic = s.active.italic;
  activeFormats.bulletList = s.active.bulletList;
  activeFormats.orderedList = s.active.orderedList;
  activeFormats.canIndent = s.canIndent;
  activeFormats.canOutdent = s.canOutdent;
  canUndo.value = s.canUndo;
  canRedo.value = s.canRedo;
  selectionEmpty.value = s.empty;
}

function onAnnotationClick(e: Event) {
  const detail = (e as CustomEvent<AnnotationClickDetail>).detail;
  emit('annotation-click', detail.ids, detail.rect);
}

// The current selection as { start, end, quote, empty } in clean UTF-16 offsets
// (the anchor for a new note), and the live edit-mapped annotations (for save).
function getSelection() {
  return editorRef.value?.getSelection() ?? { start: 0, end: 0, quote: '', empty: true };
}
function getAnnotations() {
  return editorRef.value?.getAnnotations() ?? [];
}

// The editor renumbers ordered lists, and in an article each numbered
// paragraph ("1.", "2.") reads as a list of one. So when it takes in a text
// from us (another article, a discard) it turns "2." into "1." and reports
// that as `input`, as if someone typed it. That echo is no edit: passed on,
// it would mark an article nobody touched as changed, save the renumbered
// text with the next save, and - arriving a tick late - land on the next
// article's text. An input that differs from our value only in list numbers
// is that echo, so it is dropped. A user cannot change only a number anyway:
// the editor renumbers it straight back.
const LIST_MARKER = /^(\s*(?:>\s*)*)\d+([.)] )/gm;
function withoutListNumbers(text: string): string {
  return text.replace(LIST_MARKER, '$1#$2');
}
function isRenumberingEcho(value: string): boolean {
  return value !== props.modelValue && withoutListNumbers(value) === withoutListNumbers(props.modelValue);
}

// The element sets `value` before dispatching `input`, so `e.target.value` is
// the fresh (sentinel-stripped) Markdown. The DS editor guards a value that
// merely mirrors its own document, so binding `:value="modelValue"` back in
// doesn't churn the cursor on each keystroke.
function onInput(e: Event) {
  const value = (e.target as NLDDTextEditor).value;
  if (isRenumberingEcho(value)) return;
  emit('update:modelValue', value);
}

// focusin bubbles and is composed, so it crosses the shadow boundary; the
// parent uses it to route the changes-bar undo/redo to the focused pane.
function onFocusIn() {
  emit('focus');
}

function toggleBold() {
  editorRef.value?.toggleBold();
}
function toggleItalic() {
  editorRef.value?.toggleItalic();
}
// Lists are an exclusive picker (none/bullet/ordered): setList replaces any
// existing list, so toggling the active type strips it and switching between
// types converts in one step.
function toggleBulletList() {
  editorRef.value?.setList(activeFormats.bulletList ? 'none' : 'bullet');
}
function toggleOrderedList() {
  editorRef.value?.setList(activeFormats.orderedList ? 'none' : 'ordered');
}
function indent() {
  editorRef.value?.indent();
}
function outdent() {
  editorRef.value?.outdent();
}
function undo() {
  editorRef.value?.undo();
}
function redo() {
  editorRef.value?.redo();
}
function clearHistory() {
  editorRef.value?.clearHistory();
}

// Same exposed surface the parent's header toolbar and changes bar consumed
// from the old Tiptap editor, so EditorView needs no changes.
defineExpose({
  activeFormats,
  toggleBold,
  toggleItalic,
  toggleBulletList,
  toggleOrderedList,
  indent,
  outdent,
  canUndo,
  canRedo,
  undo,
  redo,
  clearHistory,
  selectionEmpty,
  getSelection,
  getAnnotations,
});
</script>

<template>
  <!-- Empty state as the component root (no wrappers), so it lands as a direct
       child of the pane's nldd-simple-section and centers like the other panes
       (ArticleText does the same). -->
  <nldd-inline-dialog v-if="!article" text="Geen artikel geselecteerd"></nldd-inline-dialog>
  <div v-else class="article-text-editor" data-testid="article-text-editor">
    <nldd-text-editor
      ref="editorRef"
      embedded
      :rows="8"
      resize="auto"
      annotatable
      :value="modelValue"
      :annotations="annotations"
      :readonly="!editable"
      accessible-label="Artikeltekst"
      @input="onInput"
      @focusin="onFocusIn"
      @state-change="onState"
      @annotation-click="onAnnotationClick"
    ></nldd-text-editor>
  </div>
</template>

<style scoped>
.article-text-editor {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
</style>
