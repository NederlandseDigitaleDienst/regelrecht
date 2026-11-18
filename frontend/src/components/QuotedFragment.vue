<script setup lang="ts">
/**
 * QuotedFragment — the one place the "quoted fragment" pattern for notes lives.
 * Shows the annotated text in italic with a little sentence-aware context around
 * it, and ellipses (outside the italic) where the snippet was truncated. Used
 * wherever a note's referenced fragment appears — the note sheet, the Notities
 * pane and the NoteCreator — so a change to the pattern lands everywhere.
 *
 * Feed it a `quoteContext()` result; it renders nothing when there is no quote.
 */

/** The part of a quoteContext() result this component reads. */
interface QuotedFragmentParts {
  quote?: string;
  before?: string;
  after?: string;
  ellipsisBefore?: string;
  ellipsisAfter?: string;
}

withDefaults(defineProps<{
  // { quote, before, after, ellipsisBefore, ellipsisAfter } from quoteContext().
  fragment?: QuotedFragmentParts | null;
}>(), {
  fragment: null,
});
</script>

<template>
  <nldd-rich-text v-if="fragment && fragment.quote">
    <p>{{ fragment.ellipsisBefore }}{{ fragment.before }}<i>{{ fragment.quote }}</i>{{ fragment.after }}{{ fragment.ellipsisAfter }}</p>
  </nldd-rich-text>
</template>
