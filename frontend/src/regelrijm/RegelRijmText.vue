<script setup lang="ts">
// Shows a RegelRijm reading. The segments come from frontend-shared's
// renderer; this only gives each token a class. The text itself, including
// its line breaks and tab indentation, is the renderer's.
//
// With `interactive`, a folded part opens one level deeper on a click
// (`open`), a name in `navigable` follows to the rule that computes it
// (`follow`), and an operation word whose part is in `selectable` selects
// that part (`select`, with its path). A name in `blocks` is computed from
// other values and reads as a design-system token instead of a word. A name
// in `hints` gets a tooltip with that text, saying where its value comes from.
//
// Nothing is told apart by colour alone: operation words are bold, a name
// without a term keeps its brackets, a computed name is a token, and an
// operation word that can be clicked shows an underline on hover or focus.
import { computed } from 'vue';
import { pathKey } from '@regelrecht/frontend-shared/regelrijm/index';
import type { Path, Segment, Token } from '@regelrecht/frontend-shared/regelrijm/index';

const props = withDefaults(defineProps<{
  segments: readonly Segment[];
  interactive?: boolean;
  navigable?: ReadonlySet<string>;
  blocks?: ReadonlySet<string>;
  selectable?: ReadonlySet<string>;
  hints?: ReadonlyMap<string, string>;
}>(), {
  interactive: false,
  navigable: () => new Set(),
  blocks: () => new Set(),
  selectable: () => new Set(),
  hints: () => new Map(),
});
const emit = defineEmits<{
  open: [seg: Token];
  follow: [seg: Token];
  select: [path: Path];
}>();

/** One line of the reading: its depth and the pieces on it. */
interface Line {
  depth: number;
  items: Segment[];
}

// The renderer's lines, each with its depth (the leading tabs). A line is its
// own block with a hanging indent, so in a narrow pane a line that wraps
// stays under its own depth instead of starting at the left edge again.
const lines = computed(() => {
  let last: Line = { depth: 0, items: [] };
  const out: Line[] = [last];
  for (const seg of props.segments) {
    if (typeof seg !== 'string') {
      last.items.push(seg);
      continue;
    }
    seg.split('\n').forEach((part, i) => {
      let text = part;
      if (i > 0) {
        // /^\t*/ matches every string, if only with nothing.
        const depth = text.match(/^\t*/)![0].length;
        text = text.slice(depth);
        last = { depth, items: [] };
        out.push(last);
      }
      if (text) last.items.push(text);
    });
  }
  return out;
});

const isName = (seg: Token) => seg.tk === 'name' || seg.tk === 'term';

function action(seg: Segment): (() => void) | null {
  if (!props.interactive || typeof seg === 'string') return null;
  if (seg.tk === 'fold') return () => emit('open', seg);
  if (isName(seg) && seg.path?.[0] !== 'output' && seg.ref !== undefined && props.navigable.has(seg.ref)) return () => emit('follow', seg);
  const path = seg.path;
  if (seg.tk === 'keyword' && path && props.selectable.has(pathKey(path))) return () => emit('select', path);
  return null;
}

function isBlock(seg: Segment): boolean {
  return typeof seg !== 'string' && isName(seg) && seg.path?.[0] !== 'output' && seg.ref !== undefined && props.blocks.has(seg.ref);
}

function hint(seg: Segment): string | undefined {
  return typeof seg !== 'string' && isName(seg) && seg.path?.[0] !== 'output' && seg.ref !== undefined ? props.hints.get(seg.ref) : undefined;
}

function activate(e: Event, seg: Segment) {
  const run = action(seg);
  if (!run) return;
  e.preventDefault();
  run();
}

function label(seg: Token): string {
  return seg.tk === 'term' ? seg.text.slice(1, -1) : seg.text;
}

// In a token the block is the boundary, so the brackets go.
function blockText(seg: Token): string {
  return label(seg).replace(/^\[(.*)\]$/s, '$1');
}

// An operation word that opens its part reads as text, not as a link: only on
// hover or focus does it show it can be clicked. A link is for going elsewhere.
function actionClass(seg: Token): string | null {
  if (!action(seg)) return null;
  return seg.tk === 'keyword' ? 'rr-select' : 'rr-action';
}

function testid(seg: Token): string | undefined {
  if (!action(seg)) return isBlock(seg) ? `regelrijm-block-${seg.ref}` : undefined;
  if (seg.tk === 'fold') return `regelrijm-open-${seg.number}`;
  // A keyword only has an action when it has a path (see action()).
  if (seg.tk === 'keyword') return `regelrijm-select-${pathKey(seg.path!)}`;
  return `regelrijm-follow-${seg.ref}`;
}

const CLASSES: Partial<Record<Token['tk'], string>> = {
  keyword: 'rr-keyword',
  term: 'rr-term',
  name: 'rr-untranslated',
  literal: 'rr-literal',
  fold: 'rr-fold',
  letter: 'rr-letter',
  hole: 'rr-problem',
  error: 'rr-problem',
};

// A name with a hint has a tooltip instead; a title would show it twice.
function title(seg: Token): string | undefined {
  if (hint(seg)) return undefined;
  if (seg.tk === 'name') return `${seg.ref} — nog geen term`;
  if (seg.tk === 'term') return seg.ref;
  if (seg.tk === 'fold') return `Onderdeel ${seg.number}, ingeklapt${props.interactive ? ' — klik om uit te klappen' : ''}`;
  if (seg.tk === 'letter') return `Zie ${seg.letter} onder ‘waarbij’`;
  if (seg.tk === 'keyword' && action(seg)) return 'Klik om dit onderdeel te openen';
  return undefined;
}
</script>

<template>
  <div class="rr-text" lang="nl" data-testid="regelrijm-text">
    <div v-for="(line, l) in lines" :key="l" class="rr-line" :style="{ '--rr-depth': line.depth }"><template v-for="(seg, i) in line.items" :key="i"><template v-if="typeof seg === 'string'">{{ seg }}</template><nldd-tooltip v-else-if="hint(seg)" :text="hint(seg)" placement="top" :data-testid="`regelrijm-hint-${seg.ref}`"><nldd-token
      v-if="isBlock(seg)"
      :text="blockText(seg)"
      :role="action(seg) ? 'button' : undefined"
      :tabindex="action(seg) ? 0 : undefined"
      :data-testid="testid(seg)"
      :class="{ 'rr-block-action': action(seg) }"
      @click="activate($event, seg)"
      @keydown.enter="activate($event, seg)"
      @keydown.space="activate($event, seg)"
    ></nldd-token><span
      v-else
      :class="[CLASSES[seg.tk], actionClass(seg)]"
      :role="action(seg) ? 'button' : undefined"
      tabindex="0"
      :data-testid="testid(seg)"
      @click="activate($event, seg)"
      @keydown.enter="activate($event, seg)"
      @keydown.space="activate($event, seg)"
    >{{ label(seg) }}</span></nldd-tooltip><nldd-token
      v-else-if="isBlock(seg)"
      :text="blockText(seg)"
      :title="title(seg)"
      :role="action(seg) ? 'button' : undefined"
      :tabindex="action(seg) ? 0 : undefined"
      :data-testid="testid(seg)"
      :class="{ 'rr-block-action': action(seg) }"
      @click="activate($event, seg)"
      @keydown.enter="activate($event, seg)"
      @keydown.space="activate($event, seg)"
    ></nldd-token><span
      v-else
      :class="[CLASSES[seg.tk], actionClass(seg)]"
      :title="title(seg)"
      :role="action(seg) ? 'button' : undefined"
      :tabindex="action(seg) ? 0 : undefined"
      :data-testid="testid(seg)"
      @click="activate($event, seg)"
      @keydown.enter="activate($event, seg)"
      @keydown.space="activate($event, seg)"
    >{{ label(seg) }}</span></template></div>
  </div>
</template>

<style scoped>
/* Custom CSS (reported): the design system has no component for running
   text with inline token styling. Each line of the renderer is a block,
   indented by its depth, with a hanging indent for the part that wraps;
   pre-wrap keeps the renderer's spacing within a line. Every word is in the content colour; what kind of word it is shows
   without colour: operation words are bold, a computed name is an
   nldd-token. An operation word that opens its part stays plain and shows
   an underline only on hover or focus; anything else clickable gets a plain
   underline in the link colour. Clickable words take the link cursor and
   focus ring tokens. */
.rr-text {
  white-space: pre-wrap;
  line-height: 1.75;
  overflow-wrap: anywhere;
  color: var(--semantics-content-color, inherit);
}
.rr-line {
  padding-inline-start: calc(var(--rr-depth, 0) * 1em + 1em);
  text-indent: -1em;
}
/* text-indent is inherited; a token or tooltip must not shift its own text. */
.rr-line > * {
  text-indent: 0;
}
/* nldd-tooltip is display: contents, so the line breaks and tabs in its
   shadow template would show under pre-wrap. As an inline flex box with
   normal white space it drops text that is only white space, and the name
   stays in the line; the tooltip itself sits in the top layer. */
.rr-text nldd-tooltip {
  display: inline-flex;
  white-space: normal;
  vertical-align: baseline;
  max-width: 100%;
  min-width: 0;
}
/* A long name never runs out of the pane: the token shrinks and ends in an
   ellipsis (its own styling); the tooltip still names the whole term. Its
   shadow template has line breaks too, so white space is normal inside it,
   and a little room above and below keeps stacked tokens apart. */
.rr-text nldd-token {
  max-width: 100%;
  vertical-align: baseline;
  white-space: normal;
  margin-block: 3px;
}
.rr-keyword,
.rr-letter {
  font-weight: 700;
}
.rr-fold {
  font-style: italic;
}
.rr-action {
  cursor: var(--semantics-controls-link-cursor, pointer);
  color: var(--semantics-links-color, inherit);
  text-decoration: underline;
  text-underline-offset: 3px;
}
.rr-action:hover {
  color: var(--semantics-links-is-hovered-color, inherit);
}
.rr-select {
  cursor: var(--semantics-controls-link-cursor, pointer);
}
.rr-select:hover,
.rr-select:focus-visible {
  text-decoration: underline;
  text-underline-offset: 3px;
}
.rr-action:focus-visible,
.rr-select:focus-visible,
.rr-block-action:focus-visible {
  outline: var(--semantics-focus-ring-outline);
  outline-offset: var(--semantics-focus-ring-outline-offset);
  box-shadow: var(--semantics-focus-ring-box-shadow);
}
.rr-block-action {
  cursor: var(--semantics-controls-link-cursor, pointer);
}
.rr-problem {
  font-weight: 700;
  color: var(--semantics-content-critical-color, inherit);
}
</style>
