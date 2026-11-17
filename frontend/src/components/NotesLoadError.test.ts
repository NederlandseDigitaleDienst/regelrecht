import { describe, it, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { ref, h, defineComponent } from 'vue';

// Integration through the real useNotes: the engine throw must end up as a
// visible dialog, not only as a filled `error` ref. The premise is the
// schema-drift case: the viewed version failed to load into the engine, so
// resolveNotes throws the version error (a WASM throw is a bare string).
const engineCalls = vi.hoisted((): { lawId: string; validFrom?: string }[] => []);
const RESOLVE_ERROR =
  "law 'wet_kapot' has no loaded version with valid_from 2024-01-01; the viewed version may have failed to load";

vi.mock('../composables/useEngine', () => ({
  useEngine: () => ({
    initEngine: async () => ({
      resolveNotes(lawId: string, _yaml: string, validFrom?: string) {
        engineCalls.push({ lawId, validFrom });
        if (lawId === 'wet_kapot') throw RESOLVE_ERROR;
        return [];
      },
    }),
    loadDependency: async () => {},
  }),
}));

vi.mock('../lib/apiFetch', () => ({
  apiFetch: async () => ({
    status: 200,
    // One note, so there is something for the engine to resolve.
    text: async () => 'annotations:\n  - type: Annotation\n',
  }),
}));

import { useNotes } from '../composables/useNotes';
import NotesLoadError from './NotesLoadError.vue';

/** Harness: real useNotes wired to the dialog, exactly as EditorView does. */
function mountWithLaw(lawId: string) {
  const Harness = defineComponent({
    setup() {
      const { error } = useNotes(
        ref(lawId),
        ref({ number: '1' }),
        ref(null),
        ref('2024-01-01'),
      );
      // useNotes types `error` as unknown (whatever it caught); the resolver
      // throws an Error or, from WASM, a bare string.
      return () => h(NotesLoadError, { error: error.value as Error | string | null });
    },
  });
  return mount(Harness);
}

describe('NotesLoadError', () => {
  it('shows the resolver failure to the user instead of an empty pane', async () => {
    const wrapper = mountWithLaw('wet_kapot');
    await vi.waitFor(() => {
      expect(wrapper.find('[data-testid="notes-load-error"]').exists()).toBe(true);
    });
    const dialog = wrapper.find('[data-testid="notes-load-error"]');
    expect(dialog.attributes('text')).toBe('Notities laden mislukt');
    expect(dialog.attributes('supporting-text')).toContain('valid_from 2024-01-01');
  });

  it('renders nothing when the resolve succeeds', async () => {
    const wrapper = mountWithLaw('wet_notes_ok');
    await vi.waitFor(() => {
      expect(engineCalls.some((c) => c.lawId === 'wet_notes_ok')).toBe(true);
    });
    expect(wrapper.find('[data-testid="notes-load-error"]').exists()).toBe(false);
  });

  it('resolves the committed sidecar against the viewed law version', async () => {
    // The valid_from of the version on screen must reach resolveNotes;
    // without it the sidecar resolves against the newest loaded version.
    mountWithLaw('wet_notes_vf');
    await vi.waitFor(() => {
      expect(engineCalls.some((c) => c.lawId === 'wet_notes_vf')).toBe(true);
    });
    const call = engineCalls.find((c) => c.lawId === 'wet_notes_vf');
    expect(call).toBeDefined();
    expect(call?.validFrom).toBe('2024-01-01');
  });
});
