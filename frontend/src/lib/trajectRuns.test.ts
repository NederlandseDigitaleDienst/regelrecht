import { describe, it, expect } from 'vitest';
import {
  coverageText,
  durationText,
  isActive,
  reviewSummary,
  runOutcome,
  runStatusText,
  runSupportingText,
  runTitle,
  splitRuns,
} from './trajectRuns';
import { makeRun } from '../test-utils/runFixtures';

const lawName = (id: string) => (id === 'wet_op_de_zorgtoeslag' ? 'Wet op de zorgtoeslag' : id);

describe('runTitle', () => {
  it('names an enrichment by its law', () => {
    expect(runTitle(makeRun(), lawName)).toBe('Verrijking van Wet op de zorgtoeslag');
  });

  it('names a harvest by the name it was requested under, else the BWB id', () => {
    expect(runTitle(makeRun({ job_type: 'traject_harvest', law_id: 'BWBR0018451', law_name: 'Zorgtoeslagwet' }))).toBe(
      'Ophalen van Zorgtoeslagwet',
    );
    expect(runTitle(makeRun({ job_type: 'traject_harvest', law_id: 'BWBR0018451' }))).toBe('Ophalen van BWBR0018451');
  });

  it('names a conversion by its file, never by the synthetic law key', () => {
    expect(runTitle(makeRun({ job_type: 'document_convert', law_id: 'doc:t-abcd1234/mvt/nota.md', target_path: 'mvt/nota.md' }))).toBe(
      'Conversie van nota.md',
    );
    expect(runTitle(makeRun({ job_type: 'law_convert', law_id: 'lawdoc:t/voorstel.pdf', target_path: 'voorstel.pdf' }))).toBe(
      'Conversie van voorstel.pdf naar een regelwerk',
    );
  });
});

describe('runStatusText', () => {
  it('says a pending run that failed before is retrying, not just queued', () => {
    expect(runStatusText(makeRun({ status: 'pending', attempts: 0 }))).toBe('In de wachtrij');
    expect(runStatusText(makeRun({ status: 'pending', attempts: 1 }))).toBe('Nieuwe poging (2 van 3)');
  });

  it('names the other states', () => {
    expect(runStatusText(makeRun({ status: 'processing' }))).toBe('Bezig');
    expect(runStatusText(makeRun({ status: 'completed' }))).toBe('Klaar');
    expect(runStatusText(makeRun({ status: 'failed' }))).toBe('Mislukt');
  });
});

describe('runOutcome', () => {
  it('gives an enrichment its coverage and its review', () => {
    const run = makeRun({ articles_total: 12, articles_with_machine_readable: 9, tasks_approved: 2, tasks_open: 1 });
    expect(runOutcome(run)).toBe('9 van 12 artikelen uitvoerbaar · 2 overgenomen, 1 open');
  });

  it('says nothing yet while a run is going', () => {
    expect(runOutcome(makeRun({ status: 'processing' }))).toBeNull();
  });

  it('says a failure failed, without the error', () => {
    expect(runOutcome(makeRun({ status: 'failed', error: 'a long technical message' }))).toBe('Mislukt');
  });
});

describe('reviewSummary', () => {
  it('lists only the statuses that occur', () => {
    expect(reviewSummary(makeRun({ tasks_rejected: 1, tasks_dismissed: 2 }))).toBe('1 afgewezen, 2 als gedaan gemarkeerd');
    expect(reviewSummary(makeRun())).toBeNull();
  });
});

describe('coverageText', () => {
  it('rounds to a percentage and leaves out a missing score', () => {
    expect(coverageText(0.754)).toBe('75% van de artikelen die nog niet uitvoerbaar waren');
    expect(coverageText(null)).toBeNull();
  });
});

describe('durationText', () => {
  it('measures a finished run from start to end', () => {
    expect(durationText(makeRun())).toBe('12 min');
  });

  it('measures a running run up to now', () => {
    const now = new Date('2026-11-12T11:03:05Z').getTime();
    expect(durationText(makeRun({ status: 'processing', completed_at: null }), now)).toBe('1 u 3 min');
  });

  it('has nothing to measure before the start', () => {
    expect(durationText(makeRun({ started_at: null }))).toBeNull();
  });

  // Its started_at is the start of the attempt that failed; the wait for the
  // next one is no running time.
  it('does not time a run waiting for its next attempt', () => {
    expect(durationText(makeRun({ status: 'pending', attempts: 1, completed_at: null }))).toBeNull();
  });
});

describe('runSupportingText', () => {
  it('joins outcome, moment and requester', () => {
    const text = runSupportingText(makeRun({ job_type: 'document_convert', target_path: 'nota.md' }));
    expect(text.startsWith('Klaar · ')).toBe(true);
    expect(text.endsWith(' · Alice Voorbeeld')).toBe(true);
  });
});

describe('splitRuns', () => {
  it('splits running from done and keeps the order', () => {
    const runs = [
      makeRun({ job_id: 'a', status: 'processing' }),
      makeRun({ job_id: 'b' }),
      makeRun({ job_id: 'c', status: 'pending' }),
      makeRun({ job_id: 'd', status: 'failed' }),
    ];
    const { active, done } = splitRuns(runs);
    expect(active.map((r) => r.job_id)).toEqual(['a', 'c']);
    expect(done.map((r) => r.job_id)).toEqual(['b', 'd']);
    expect(isActive(at(active, 0))).toBe(true);
  });
});

function at<T>(list: readonly T[], i: number): T {
  const item = list[i];
  if (item === undefined) throw new Error(`no item at index ${i}`);
  return item;
}
