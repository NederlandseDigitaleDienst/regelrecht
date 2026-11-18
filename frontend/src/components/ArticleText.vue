<script setup lang="ts">
import { computed } from 'vue';
import { renderArticleHtml } from '../composables/useArticleMarkdown';

/** The part of an article this component reads. */
interface ArticleWithText {
  text?: string | null;
}

const props = withDefaults(defineProps<{
  article?: ArticleWithText | null;
  raw?: boolean;
  // Center the rich-text column. Opt-in so the library reading view can
  // center while the editor's read-only pane stays left-aligned.
  centered?: boolean;
}>(), {
  article: null,
  raw: false,
  centered: false,
});

// marked + DOMPurify pipeline (useArticleMarkdown): same list nesting and
// paragraph breaks as the editable Tekst pane, sanitized before v-html.
const html = computed(() => renderArticleHtml(props.article?.text || ''));

const paragraphs = computed(() => {
  if (!props.article?.text) return [];
  return props.article.text.split(/\n\n+/);
});
</script>

<template>
  <template v-if="article">
    <nldd-rich-text v-if="raw" :centered="centered || undefined">
      <p v-for="(p, i) in paragraphs" :key="i">{{ p }}</p>
    </nldd-rich-text>
    <nldd-rich-text v-else :centered="centered || undefined" v-html="html"></nldd-rich-text>
  </template>
  <nldd-inline-dialog v-else text="Geen artikel geselecteerd"></nldd-inline-dialog>
</template>
