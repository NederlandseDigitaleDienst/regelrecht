// The row actions of the harvester table report back through a design-system
// notification, never through alert(), which blocks the page until clicked away.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import type { LawEntry } from '../composables/useLawEntries';

const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a: unknown[]) => apiFetch(...a) }));
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }));
const show = vi.fn();
vi.mock('@nldd/design-system/notification', () => ({ NLDDNotification: { show: (...a: unknown[]) => show(...a) } }));

const { default: RowActions } = await import('./RowActions.vue');

const ROW: LawEntry = {
  law_id: 'BWBR0000001',
  law_name: 'Voorbeeldwet',
  slug: null,
  status: 'harvested',
  harvest_job_id: null,
  enrich_job_id: null,
  coverage_score: null,
  harvest_fail_count: 0,
  enrich_fail_count: 0,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
};

async function choose(text: string) {
  const wrapper = mount(RowActions, { props: { row: ROW } });
  await wrapper.find(`nldd-menu-item[text="${text}"]`).trigger('click');
  await flushPromises();
  return wrapper;
}

describe('RowActions feedback', () => {
  beforeEach(() => {
    vi.stubGlobal('alert', vi.fn());
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    apiFetch.mockReset();
    show.mockReset();
  });

  it('reports a created harvest job as a success', async () => {
    apiFetch.mockResolvedValue(new Response(JSON.stringify({ job_id: 'job-1' }), { status: 201 }));
    const wrapper = await choose('Harvest');
    expect(show).toHaveBeenCalledWith({ variant: 'success', text: 'Created harvest job', supportingText: 'job-1' });
    expect(wrapper.emitted('action-complete')).toHaveLength(1);
    expect(window.alert).not.toHaveBeenCalled();
  });

  it('warns when a job is already running, with the server text', async () => {
    apiFetch.mockResolvedValue(new Response('Al bezig', { status: 409 }));
    await choose('Enrich');
    expect(show).toHaveBeenCalledWith({ variant: 'warning', text: 'Not enriched', supportingText: 'Al bezig' });
  });

  it('reports a failure as critical, so it stays until dismissed', async () => {
    apiFetch.mockRejectedValue(new Error('netwerk weg'));
    const wrapper = await choose('Harvest');
    expect(show).toHaveBeenCalledWith({ variant: 'critical', text: 'Harvest failed', supportingText: 'netwerk weg' });
    expect(wrapper.emitted('action-complete')).toBeUndefined();
  });
});
