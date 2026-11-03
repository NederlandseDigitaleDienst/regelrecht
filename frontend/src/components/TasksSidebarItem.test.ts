import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';

// Same mocking pattern as TasksListPane.test.js - route useTasks.js's network
// leg through a controllable spy.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a: unknown[]) => apiFetch(...a) }));

describe('TasksSidebarItem', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
  });

  // Rows go into the mocked JSON response as-is, so they stay untyped here.
  async function mountItem(tasks: unknown[], running: unknown[] = [], props: { current?: boolean; trajectRef?: string } = {}) {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks, open_count: tasks.length, running }),
    });
    const { default: TasksSidebarItem } = await import('./TasksSidebarItem.vue');
    const wrapper = mount(TasksSidebarItem, { props: { trajectRef: 'traject-abcd1234', ...props } });
    // Flush useTasks' deferred initial load (see comment in useTasks.js) -
    // same flush sequence as TasksListPane.test.js's mountPane().
    await wrapper.vm.$nextTick();
    await Promise.resolve();
    await wrapper.vm.$nextTick();
    return wrapper;
  }

  const REVIEW_TASK = { id: 't1', task_type: 'job_review', title: 'x' };
  const FAILED_TASK = {
    id: 't2',
    task_type: 'job_failed',
    title: 'Conversie mislukt: bijlage.md',
    payload: { error: 'boom' },
  };

  it('toont geen badge zonder open taken en zonder lopende jobs', async () => {
    const wrapper = await mountItem([], []);
    expect(wrapper.find('nldd-badge').exists()).toBe(false);
  });

  // De badge telt prioriteit, niet alle open taken: een stapel reviews die kan
  // wachten mag geen rood alarm geven.
  it('toont geen aantal-badge voor open taken zonder prioriteit', async () => {
    const wrapper = await mountItem([REVIEW_TASK, { ...REVIEW_TASK, id: 't9' }], []);
    expect(wrapper.find('nldd-badge').exists()).toBe(false);
  });

  it('toont de aantal-badge met het prioriteit-aantal', async () => {
    const wrapper = await mountItem([REVIEW_TASK, FAILED_TASK], []);
    const badge = wrapper.get('nldd-badge');
    // Alleen de mislukte taak telt, de review niet.
    expect(badge.attributes('number')).toBe('1');
    expect(badge.attributes('color')).toBeUndefined();
  });

  it('counts only the priority tasks of this traject', async () => {
    const elsewhere = { ...FAILED_TASK, id: 't3', traject_id: 'ffff0000-0000-4000-8000-000000000000' };
    const here = { ...FAILED_TASK, id: 't4', traject_id: 'abcd1234-0000-4000-8000-000000000000' };
    const wrapper = await mountItem([elsewhere, here], []);
    expect(wrapper.get('nldd-badge').attributes('number')).toBe('1');
  });

  it('laat prioriteit voorgaan op het lopende-signaal', async () => {
    const wrapper = await mountItem([FAILED_TASK], [{ job_id: 'j1', law_id: 'test_wet' }]);
    const badge = wrapper.get('nldd-badge');
    expect(badge.attributes('number')).toBe('1');
    expect(badge.attributes('color')).toBeUndefined();
  });

  it('toont geen badge zonder prioriteit, ook niet met een lopende job', async () => {
    // Een lopende taak (Wachten op) is geen alarm - geen stille stip meer.
    const wrapper = await mountItem([], [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }]);
    expect(wrapper.find('nldd-badge').exists()).toBe(false);
  });

  it('toont geen badge bij open taken zonder prioriteit, ook met een lopende job', async () => {
    const wrapper = await mountItem([REVIEW_TASK], [{ job_id: 'j1', law_id: 'test_wet' }]);
    expect(wrapper.find('nldd-badge').exists()).toBe(false);
  });

  it('markeert het item als current op de taken-route', async () => {
    const wrapper = await mountItem([], [], { current: true });
    expect(wrapper.get('nldd-list-item').attributes('current')).toBeDefined();
  });
});
