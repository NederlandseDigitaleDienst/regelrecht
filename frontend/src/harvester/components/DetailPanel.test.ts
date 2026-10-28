import { mount } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import DetailPanel from './DetailPanel.vue';

// The sheet teleports to body; render it in place so the rows are in w.html().
const global = {
  config: { compilerOptions: { isCustomElement: (tag: string) => tag.startsWith('nldd-') } },
  stubs: { Teleport: true },
};

describe('DetailPanel', () => {
  it('shows attempts as "n / max" when the job has them', () => {
    const w = mount(DetailPanel, {
      props: { job: { id: 'j1', status: 'failed', attempts: 2, max_attempts: 3 }, isOpen: true },
      global,
    });
    expect(w.html()).toContain('2 / 3');
  });

  // OverviewView opens the panel with a recent-failure row when fetching the
  // full job fails; that row has no attempts. A missing field is left out,
  // like every other row, instead of reading "undefined / undefined".
  it('leaves out the attempts row when the job has no attempts', () => {
    const w = mount(DetailPanel, {
      props: { job: { id: 'j1', status: 'failed', law_id: 'wet_a' }, isOpen: true },
      global,
    });
    expect(w.html()).toContain('wet_a');
    expect(w.html()).not.toContain('Attempts');
    expect(w.html()).not.toContain('undefined');
  });

  it('shows live progress for a running job', () => {
    const w = mount(DetailPanel, {
      props: {
        job: {
          id: 'j1',
          status: 'processing',
          progress: { worker: { call: 'translate', turns: 5, recent_tools: ['Read a.yaml'] } },
        },
        isOpen: true,
      },
      global,
    });
    expect(w.html()).toContain('Progress');
    expect(w.html()).not.toContain('Last progress');
    expect(w.html()).toContain('translate');
    expect(w.html()).toContain('Read a.yaml');
  });

  // A failed job keeps what it was doing when it stopped.
  it('keeps the last progress of a job that has stopped', () => {
    const w = mount(DetailPanel, {
      props: {
        job: { id: 'j1', status: 'failed', progress: { worker: { turns: 41 } } },
        isOpen: true,
      },
      global,
    });
    expect(w.html()).toContain('Last progress');
    expect(w.html()).toContain('41');
  });

  it('shows no progress section without progress', () => {
    const w = mount(DetailPanel, {
      props: { job: { id: 'j1', status: 'pending' }, isOpen: true },
      global,
    });
    expect(w.find('[data-testid="job-progress"]').exists()).toBe(false);
  });
});
