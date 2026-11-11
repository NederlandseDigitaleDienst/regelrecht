import { afterEach, describe, it, expect, vi } from 'vitest';
import { ref } from 'vue';
import { useNotes, useResolvedDraftNotes } from './useNotes';
import type { Note, ResolvedNote } from './useNotes';
import type { MatchResult } from './useTextSelection';

// Capture what useResolvedDraftNotes hands the WASM engine. The mock replaces
// the singleton engine bootstrap.
const engineCalls = vi.hoisted(
  () => [] as { lawId: string; validFrom: string | undefined }[],
);
// What the mocked engine returns from resolveNotes; per-test.
const resolveNotesResult = vi.hoisted(() => ({ value: [] as ResolvedNote[] }));
const resolveNotesCalls = vi.hoisted(() => ({ count: 0 }));
// The notes document the API answers with; per-test.
const notesDocument = vi.hoisted(() => ({
  value: 'annotations:\n  - type: Annotation\n',
}));
vi.mock('./useEngine', () => ({
  useEngine: () => ({
    initEngine: async () => ({
      resolveNote(lawId: string, _selector: unknown, validFrom: string | undefined) {
        engineCalls.push({ lawId, validFrom });
        return {
          status: 'found',
          matches: [{ article_number: '1', start: 0, end: 3 }],
        };
      },
      resolveNotes: () => {
        resolveNotesCalls.count += 1;
        return resolveNotesResult.value;
      },
    }),
    loadDependency: async () => {},
  }),
}));

vi.mock('../lib/apiFetch', () => ({
  apiFetch: async () => ({ status: 200, text: async () => notesDocument.value }),
}));

describe('useNotes issues', () => {
  // The fixtures carry only the MatchResult fields the issue text reads.
  async function issuesFor(lawId: string, match: Pick<MatchResult, 'status' | 'matches' | 'skip_reason'>) {
    resolveNotesResult.value = [{ note: { id: 'n1' }, match, error: null }];
    const { issues } = useNotes(
      ref(lawId),
      ref({ number: '1' }),
      ref(null),
      ref('2024-01-01'),
    );
    await vi.waitFor(() => expect(issues.value).toHaveLength(1));
    return issues.value[0]!.reason;
  }

  it('blames the quote length only when the quote length caused the skip', async () => {
    // 'skipped' has three causes; "kort het citaat in" fixes exactly one of
    // them. On a drained scan budget that advice is wrong: the author
    // shortens the quote, nothing changes, and the real cause stays hidden.
    const quote = await issuesFor('wet_skip_quote', {
      status: 'skipped',
      matches: [],
      skip_reason: 'quote_too_long',
    });
    expect(quote).toContain('citaat te lang');
    expect(quote).toContain('kort het citaat in');

    const budget = await issuesFor('wet_skip_budget', {
      status: 'skipped',
      matches: [],
      skip_reason: 'search_budget',
    });
    expect(budget).not.toContain('kort het citaat in');
    expect(budget).toContain('niet volledig doorzocht');
  });

  it('keeps a skipped note distinguishable from an orphaned one', async () => {
    const orphaned = await issuesFor('wet_orphaned', {
      status: 'orphaned',
      matches: [],
    });
    expect(orphaned).toContain('orphaned');
    expect(orphaned).not.toContain('niet naar gezocht');
  });
});

describe('useNotes without notes', () => {
  afterEach(() => {
    notesDocument.value = 'annotations:\n  - type: Annotation\n';
  });

  it('resolves an empty notes document to no notes, without the engine', async () => {
    // The API answers a law without notes with an empty document (200), not
    // a 404. That costs no engine start.
    notesDocument.value = '$schema: https://example.org/schema.json\nannotations: []\n';
    resolveNotesCalls.count = 0;
    const { loading, notesForArticle } = useNotes(
      ref('wet_zonder_notities'),
      ref({ number: '1' }),
      ref(null),
      ref('2024-01-01'),
    );
    await vi.waitFor(() => expect(loading.value).toBe(false));
    expect(notesForArticle.value).toEqual([]);
    expect(resolveNotesCalls.count).toBe(0);
  });
});

describe('useResolvedDraftNotes', () => {
  it('resolves drafts against the viewed law version, not the newest', async () => {
    // The engine holds every version of the law; the composable must pass
    // the viewed version's valid_from through so a draft highlights in the
    // text on screen instead of the newest loaded version.
    engineCalls.length = 0;
    const drafts = ref<Note[]>([{ target: { selector: { exact: 'abc' } } }]);
    const lawId = ref('wet_x');
    const selectedArticle = ref({ number: '1' });
    const trajectRef = ref(null);
    const validFrom = ref('2024-01-01');
    const { draftNotesForArticle } = useResolvedDraftNotes(
      drafts,
      lawId,
      selectedArticle,
      trajectRef,
      validFrom,
    );
    await vi.waitFor(() => expect(engineCalls).toHaveLength(1));
    expect(engineCalls[0]).toEqual({ lawId: 'wet_x', validFrom: '2024-01-01' });
    expect(draftNotesForArticle.value).toHaveLength(1);
  });

  it('passes undefined (= latest) when the law has no valid_from', async () => {
    engineCalls.length = 0;
    const drafts = ref<Note[]>([{ target: { selector: { exact: 'abc' } } }]);
    useResolvedDraftNotes(drafts, ref('wet_x'), ref({ number: '1' }), ref(null), ref(null));
    await vi.waitFor(() => expect(engineCalls).toHaveLength(1));
    expect(engineCalls[0]).toBeDefined();
    expect(engineCalls[0]!.validFrom).toBeUndefined();
  });
});
