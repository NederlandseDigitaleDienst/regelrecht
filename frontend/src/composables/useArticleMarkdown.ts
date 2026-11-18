/**
 * Article-text markdown pipeline for the read-only Tekst pane
 * (ArticleText.vue). Historically shared with the retired notes-on twin view
 * (#646); kept as its own module so any future second consumer renders the
 * law text identically - same list nesting, same paragraph breaks.
 *
 * marked v18 no longer sanitizes HTML in Markdown by default; harvested laws
 * could in principle carry arbitrary HTML, so the output is always run through
 * DOMPurify before it reaches the DOM (v-html or manual parsing).
 */
import { marked } from 'marked';
import DOMPurify from 'dompurify';

/** Raw article text -> sanitized HTML string. */
export function renderArticleHtml(text: string | null | undefined): string {
  if (!text) return '';
  // Without async extensions marked.parse returns a string synchronously.
  return DOMPurify.sanitize(marked.parse(text) as string);
}
