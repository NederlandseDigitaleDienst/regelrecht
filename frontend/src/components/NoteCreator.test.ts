import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import NoteCreator from './NoteCreator.vue';
import { useAuth } from '../composables/useAuth';
import type { Note } from '../composables/useNotes';
import type { BuiltSelector } from '../composables/useTextSelection';

// NoteCreator pulls in useAmbiguityVocabulary, which fetches the vocabulary
// file on first use. Stub fetch so the test does not hit the network (and
// does not leave an aborted request dangling at teardown).
beforeEach(() => {
  localStorage.clear();
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: true,
    text: async () =>
      'ambiguity:\n  - id: open-norm-partial\n    label: Open norm, deels ingevuld\n',
  });
});

// NoteCreator builds the W3C Annotation a draft becomes. The body shape is
// motivation-dependent and is the contract useDraftNotes exports and the
// resolver re-reads, so it is worth pinning per motivation.

const stubs = {
  'nldd-popover': {
    template: '<div><slot/></div>',
    methods: { showPopover() {}, hidePopover() {}, matches() { return false; } },
  },
  'nldd-inline-dialog': { template: '<div/>' },
  'nldd-segmented-control': { template: '<div><slot/></div>' },
  'nldd-segmented-control-item': { template: '<div/>' },
  'nldd-text-field': { template: '<input/>' },
  'nldd-button': {
    props: ['disabled'],
    template: '<button :disabled="disabled" @click="$emit(\'click\')"><slot/></button>',
  },
};

const RAW = 'Indien de normpremie voor een verzekerde';

/**
 * Setup state the tests set and read through `vm`. NoteCreator exposes
 * nothing, but test-utils proxies the setup state onto `vm`.
 */
interface NoteCreatorInternals {
  commentText: string;
  links: { type: 'element' | 'document'; value: string; label: string }[];
  ambiguityTags: string[];
  isTask: boolean;
  taskDone: boolean;
  shareWithTraject: boolean;
  canSave: boolean;
  selectorResult: BuiltSelector | null;
  save(): void;
}
const vmOf = (w: VueWrapper) => w.vm as unknown as NoteCreatorInternals;

/** The first `create` event's arguments (note, share), asserted to exist. */
function firstCreated(w: VueWrapper): [Note, boolean] {
  const event = w.emitted<[Note, boolean]>('create')?.[0];
  expect(event).toBeDefined();
  return event!; // asserted on the line above
}

function mountCreator(extraProps: Record<string, unknown> = {}, extraStubs: Record<string, unknown> = {}) {
  const engine = {
    resolveNote: () => ({
      status: 'found',
      matches: [{ article_number: '1', start: 10, end: 20 }],
    }),
  };
  return mount(NoteCreator, {
    props: {
      range: { start: 10, end: 20 }, // "normpremie"
      rawText: RAW,
      lawId: 'wet_op_de_zorgtoeslag',
      article: {
        number: '1',
        machine_readable: {
          execution: { output: [{ name: 'hoogte_zorgtoeslag' }] },
        },
      },
      engine,
      anchor: document.createElement('span'),
      ...extraProps,
    },
    global: { stubs: { ...stubs, ...extraStubs } },
  });
}

describe('NoteCreator', () => {
  it('builds a commenting note with a TextualBody', async () => {
    const w = mountCreator();
    await nextTick();
    vmOf(w).commentText = 'uitleg over de normpremie';
    await nextTick();
    vmOf(w).save();
    const [note] = firstCreated(w);
    expect(note.motivation).toBe('commenting');
    expect(note.target?.selector?.exact).toBe('normpremie');
    expect(note.body).toEqual({
      type: 'TextualBody',
      value: 'uitleg over de normpremie',
      purpose: 'commenting',
      format: 'text/plain',
      language: 'nl',
    });
    expect(note.__draft).toBe(true);
  });

  it('builds a linking note with a SpecificResource body per linked target', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    // One element link and one document link -> two SpecificResource bodies.
    vmOf(w).links = [
      { type: 'element', value: 'hoogte_zorgtoeslag', label: 'hoogte_zorgtoeslag' },
      { type: 'document', value: 'mvt/concept.md', label: 'mvt/concept.md' },
    ];
    await nextTick();
    vmOf(w).save();
    const [note] = firstCreated(w);
    expect(note.motivation).toBe('linking');
    expect(note.body).toEqual([
      {
        type: 'SpecificResource',
        source: 'regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag#hoogte_zorgtoeslag',
        purpose: 'linking',
      },
      {
        type: 'SpecificResource',
        source: 'regelrecht://doc/t-123/mvt/concept.md',
        purpose: 'linking',
      },
    ]);
  });

  it('maps the task switches to the workflow and reveals "done" only for a task', async () => {
    const w = mountCreator();
    await nextTick();
    // "Afgehandeld" is hidden until the note is a task.
    expect(w.find('[data-testid="note-task-done"]').exists()).toBe(false);
    vmOf(w).commentText = 'x';
    vmOf(w).isTask = true;
    vmOf(w).taskDone = true;
    await nextTick();
    expect(w.find('[data-testid="note-task-done"]').exists()).toBe(true);
    vmOf(w).save();
    expect(firstCreated(w)[0].workflow).toBe('resolved');
  });

  it('combines a comment, ambiguity tag and open action into one note', async () => {
    const w = mountCreator();
    await nextTick();
    vmOf(w).commentText = 'is dit een open norm?';
    vmOf(w).ambiguityTags = ['open-norm-partial'];
    vmOf(w).isTask = true; // a task, not done -> workflow 'open'
    await nextTick();
    vmOf(w).save();
    const [note] = firstCreated(w);
    expect(note.workflow).toBe('open');
    expect(Array.isArray(note.body)).toBe(true);
    expect((note.body as unknown[])[1]).toEqual({
      type: 'TextualBody',
      value: 'open-norm-partial',
      purpose: 'tagging',
    });
  });

  it('builds a tagging note with multiple ambiguity labels', async () => {
    const w = mountCreator();
    await nextTick();
    vmOf(w).ambiguityTags = ['missing-document', 'open-norm-partial'];
    await nextTick();
    vmOf(w).save();
    const [note] = firstCreated(w);
    expect(note.body).toEqual([
      { type: 'TextualBody', value: 'missing-document', purpose: 'tagging' },
      { type: 'TextualBody', value: 'open-norm-partial', purpose: 'tagging' },
    ]);
  });

  it('does not save while the selector is ambiguous', async () => {
    const w = mountCreator({
      engine: {
        resolveNote: () => ({ status: 'ambiguous', matches: [{}, {}] }),
      },
    });
    await nextTick();
    vmOf(w).commentText = 'iets';
    await nextTick();
    expect(vmOf(w).canSave).toBe(false);
    vmOf(w).save();
    expect(w.emitted('create')).toBeUndefined();
  });

  it('does not search the law or warn about the selection without an article', async () => {
    // Without an article there is no article number to check a match
    // against, so every resolver match would read as a mis-anchor.
    const resolveNote = vi.fn(() => ({
      status: 'found',
      matches: [{ article_number: '1', start: 10, end: 20 }],
    }));
    const w = mountCreator({ article: null, engine: { resolveNote } });
    await nextTick();
    expect(resolveNote).not.toHaveBeenCalled();
    expect(w.find('[data-testid="note-creator-status"]').exists()).toBe(false);
    vmOf(w).commentText = 'iets';
    await nextTick();
    vmOf(w).save();
    expect(w.emitted('create')).toBeUndefined();
  });

  it('hides the whole form when the selection is unusable', async () => {
    const w = mountCreator({
      engine: {
        resolveNote: () => ({ status: 'orphaned', matches: [] }),
      },
    });
    await nextTick();
    // Only the warning - no type picker, no save. The user dismisses by
    // clicking outside (no in-form cancel button anymore).
    expect(w.find('[data-testid="note-creator-status"]').exists()).toBe(true);
    expect(w.find('[data-testid="note-save"]').exists()).toBe(false);
    expect(w.find('[data-testid="note-comment-text"]').exists()).toBe(false);
    expect(w.find('[data-testid="note-cancel"]').exists()).toBe(false);
  });

  it('tells the user a skipped search was not searched, unlike an orphaned one', async () => {
    // The resolver refused the scan (quote over the fuzzy budget). The user
    // must see something different from "Niet teruggevonden": nothing was
    // established about the text, and the lidnummer advice that belongs to
    // a failed search would be misdirected here.
    const skipped = mountCreator({
      engine: {
        resolveNote: () => ({
          status: 'skipped',
          matches: [],
          skip_reason: 'quote_too_long',
        }),
      },
    });
    const orphaned = mountCreator({
      engine: {
        resolveNote: () => ({ status: 'orphaned', matches: [] }),
      },
    });
    await nextTick();

    const skippedBanner = skipped.find('[data-testid="note-creator-status"]');
    const orphanedBanner = orphaned.find('[data-testid="note-creator-status"]');
    expect(skippedBanner.exists()).toBe(true);
    expect(skippedBanner.attributes('text')).toBe('Niet naar gezocht');
    expect(orphanedBanner.attributes('text')).toBe('Niet teruggevonden');
    expect(skippedBanner.attributes('text')).not.toBe(
      orphanedBanner.attributes('text'),
    );
    expect(skippedBanner.attributes('supporting-text')).not.toContain(
      'lidnummer',
    );
    // Still no save path: a skipped selection anchors nothing.
    expect(skipped.find('[data-testid="note-save"]').exists()).toBe(false);
  });

  it('does not blame the selection length when the law text drained the budget', async () => {
    // 'search_budget' means the law was too large or too repetitive to scan,
    // which shortening the selection cannot fix. Telling the author to shorten
    // sends them into the same skip again with no clue why.
    const budget = mountCreator({
      engine: {
        resolveNote: () => ({
          status: 'skipped',
          matches: [],
          skip_reason: 'search_budget',
        }),
      },
    });
    const tooLong = mountCreator({
      engine: {
        resolveNote: () => ({
          status: 'skipped',
          matches: [],
          skip_reason: 'quote_too_long',
        }),
      },
    });
    await nextTick();

    const budgetBanner = budget.find('[data-testid="note-creator-status"]');
    const tooLongBanner = tooLong.find('[data-testid="note-creator-status"]');
    expect(budgetBanner.attributes('text')).toBe('Niet naar gezocht');
    expect(budgetBanner.attributes('supporting-text')).not.toBe(
      tooLongBanner.attributes('supporting-text'),
    );
    expect(budgetBanner.attributes('supporting-text')).not.toContain(
      'te lang',
    );
    expect(tooLongBanner.attributes('supporting-text')).toContain('te lang');
  });

  it('shows the full form once the selection resolves uniquely', async () => {
    const w = mountCreator(); // engine returns a matching unique result
    await nextTick();
    expect(w.find('[data-testid="note-creator-status"]').exists()).toBe(false);
    expect(w.find('[data-testid="note-save"]').exists()).toBe(true);
  });

  it('sets the creator to the signed-in user (id + name)', async () => {
    const { person } = useAuth();
    const w = mountCreator();
    await nextTick();
    // `/auth/status` returns the identity as `sub` (no `id` field); creator.id
    // is derived from it.
    person.value = { sub: 'u1', name: 'J. de Vries', email: 'j.devries@example.org', roles: [] };
    vmOf(w).commentText = 'x';
    await nextTick();
    vmOf(w).save();
    const [note] = firstCreated(w);
    expect(note.creator).toEqual({ id: 'u1', name: 'J. de Vries' });
    person.value = null; // reset shared module state for other tests
  });

  it('emits the share intent as the second create arg (default off)', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    vmOf(w).commentText = 'x';
    await nextTick();
    vmOf(w).save();
    expect(firstCreated(w)[1]).toBe(false);

    const w2 = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    vmOf(w2).commentText = 'x';
    vmOf(w2).shareWithTraject = true;
    await nextTick();
    vmOf(w2).save();
    expect(firstCreated(w2)[1]).toBe(true);
  });

  it('shows the irreversible-share warning only when the switch is on', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    expect(w.find('[data-testid="note-share-warning"]').exists()).toBe(false);
    vmOf(w).shareWithTraject = true;
    await nextTick();
    expect(w.find('[data-testid="note-share-warning"]').exists()).toBe(true);
  });

  it('the share switch change event drives the ref and the warning', async () => {
    const w = mountCreator({ trajectRef: 't-123' });
    await nextTick();
    w.find('[data-testid="note-share"]').element.dispatchEvent(
      new CustomEvent('change', { detail: { checked: true }, bubbles: true, composed: true }),
    );
    await nextTick();
    expect(vmOf(w).shareWithTraject).toBe(true);
    expect(w.find('[data-testid="note-share-warning"]').exists()).toBe(true);
  });

  // The open form holds `range` offsets into the article it was opened on. If
  // the user switches article (tab click, browser back) while it is open,
  // buildSelector would slice the NEW article's text at those stale offsets
  // and could anchor a note on text the user never selected. The form must
  // cancel itself on any article/law switch (the guard the retired
  // AnnotatedText view carried as must-fix 2c).
  it('emits cancel when the article changes underneath the open form', async () => {
    const w = mountCreator();
    await nextTick();
    // Premise check: the form is genuinely open against article 1.
    expect(vmOf(w).selectorResult?.status).toBe('found');
    expect(w.emitted('cancel')).toBeUndefined();
    await w.setProps({
      article: { number: '2', machine_readable: null },
    });
    expect(w.emitted('cancel')).toHaveLength(1);
  });

  it('emits cancel when the law changes underneath the open form', async () => {
    const w = mountCreator();
    await nextTick();
    await w.setProps({ lawId: 'participatiewet' });
    expect(w.emitted('cancel')).toHaveLength(1);
  });

  it('does not cancel when the same article object is merely replaced (reload after save)', async () => {
    const w = mountCreator();
    await nextTick();
    // Same law, same article number, new object identity: offsets stay valid.
    await w.setProps({
      article: {
        number: '1',
        machine_readable: {
          execution: { output: [{ name: 'hoogte_zorgtoeslag' }] },
        },
      },
    });
    expect(w.emitted('cancel')).toBeUndefined();
  });
});
