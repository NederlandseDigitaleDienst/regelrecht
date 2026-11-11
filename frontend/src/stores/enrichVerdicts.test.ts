import { beforeEach, describe, expect, it } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useEnrichVerdictsStore } from './enrichVerdicts';

describe('useEnrichVerdictsStore', () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  it('keeps verdicts per enrichment, and clears one enrichment at a time', () => {
    const verdicts = useEnrichVerdictsStore();
    verdicts.record('job-1', 't1', 'approved', 'artikel: 1');
    verdicts.record('job-2', 't9', 'rejected', null);

    expect(verdicts.forJob('job-1')).toEqual({ t1: { action: 'approved', content: 'artikel: 1' } });
    verdicts.clear('job-1');
    expect(verdicts.forJob('job-1')).toEqual({});
    expect(verdicts.forJob('job-2')).toEqual({ t9: { action: 'rejected', content: null } });
  });

  it('carries the verdicts through a reload, by way of sessionStorage', () => {
    useEnrichVerdictsStore().record('job-1', 't1', 'approved', null);

    // A reload: a new Pinia, a new store, the same session.
    setActivePinia(createPinia());
    expect(useEnrichVerdictsStore().forJob('job-1')).toEqual({ t1: { action: 'approved', content: null } });
  });

  it('records nothing without an enrichment or a task', () => {
    const verdicts = useEnrichVerdictsStore();
    verdicts.record(null, 't1', 'approved', null);
    verdicts.record('job-1', undefined, 'approved', null);
    expect(verdicts.verdicts).toEqual({});
  });
});
