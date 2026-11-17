import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import { computed, ref } from 'vue';
import ScenarioStatusMenuItems from './ScenarioStatusMenuItems.vue';
import type { LawScenarios } from '../scenario/useLawScenarios';
import type { ScenarioLine } from '../scenario/statusSummary';

// The parts of an open file the menu reads.
function fakeFile(filename: string, { dirty = false } = {}) {
  return { file: { filename }, featureName: ref('Basis'), dirty: ref(dirty), error: ref(null) };
}

function fakeScenarios(files: { filename: string; dirty?: boolean; attention: ScenarioLine[]; passed: ScenarioLine[] }[]): LawScenarios {
  // Only the fields the menu reads; the rest of LawScenarios is not used here.
  return {
    files: computed(() => files.map((f) => ({ file: fakeFile(f.filename, f), mismatch: false, attention: f.attention, passed: f.passed }))),
    loading: ref(false),
    loadingText: ref(''),
    listError: ref(null),
    environmentError: ref(null),
  } as unknown as LawScenarios;
}

const items = (w: ReturnType<typeof mount>) =>
  w.findAll('nldd-menu-item').map((i) => [i.attributes('text'), i.attributes('details') ?? null]);

describe('ScenarioStatusMenuItems', () => {
  const scenarios = fakeScenarios([{
    filename: 'basis.feature',
    attention: [
      { index: 1, name: 'Geen recht', status: 'failed' },
      { index: 2, name: 'Zonder verwachting', status: 'no-expectations' },
    ],
    passed: [{ index: 0, name: 'Heeft recht', status: 'passed' }],
  }]);

  it('lists what needs attention, and the passed ones in a submenu', () => {
    const w = mount(ScenarioStatusMenuItems, { props: { scenarios } });
    expect(w.find('nldd-menu-group').attributes('text')).toBe('Basis');
    expect(items(w)).toEqual([
      ['Geen recht', 'Mislukt'],
      ['Zonder verwachting', 'Geen verwachting'],
      ['1 geslaagd', null],
      ['Heeft recht', 'Geslaagd'],
    ]);
    // The passed scenario sits in the submenu of the "geslaagd" item.
    const passed = w.find('[data-testid="scenario-menu-basis.feature-passed"]');
    expect(passed.find('nldd-menu nldd-menu-item').attributes('text')).toBe('Heeft recht');
  });

  it('opens the chosen scenario', async () => {
    const w = mount(ScenarioStatusMenuItems, { props: { scenarios } });
    await w.find('[data-testid="scenario-menu-basis.feature-1"]').trigger('select');
    await w.find('[data-testid="scenario-menu-basis.feature-passed"] nldd-menu-item').trigger('select');
    expect(w.emitted('open')).toEqual([['basis.feature', 'Geen recht'], ['basis.feature', 'Heeft recht']]);
  });

  it('says when a file runs with unsaved edits', () => {
    const w = mount(ScenarioStatusMenuItems, {
      props: { scenarios: fakeScenarios([{ filename: 'basis.feature', dirty: true, attention: [], passed: [] }]) },
    });
    const note = w.find('nldd-menu-item');
    expect(note.attributes('text')).toBe('Rekent met niet-opgeslagen wijzigingen');
    expect(note.attributes('disabled')).toBeDefined();
  });
});
