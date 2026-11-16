import { describe, it, expect } from 'vitest';
import { taskDetailRows } from './taskDetails';
import { makeRun } from '../test-utils/runFixtures';

const lawName = (id: string) => (id === 'wet_a' ? 'Alpha-wet' : id);
const labels = (rows: ReturnType<typeof taskDetailRows>) => rows.map((r) => r.label);
const value = (rows: ReturnType<typeof taskDetailRows>, label: string) => rows.find((r) => r.label === label)?.value;

const review = {
  id: 't1',
  task_type: 'job_review',
  created_at: '2026-11-12T10:12:05Z',
  payload: { law_id: 'wet_a', article: '3' },
};

describe('taskDetailRows', () => {
  it('names the law, the article and when the proposal arrived', () => {
    const rows = taskDetailRows(review, null, lawName);
    expect(labels(rows)).toEqual(['Regelwerk', 'Artikel', 'Voorgesteld']);
    expect(value(rows, 'Regelwerk')).toBe('Alpha-wet');
    expect(value(rows, 'Artikel')).toBe('Artikel 3');
  });

  it('says the whole regelwerk for a review without an article', () => {
    const rows = taskDetailRows({ ...review, payload: { law_id: 'wet_a' } }, null, lawName);
    expect(value(rows, 'Artikel')).toBe('Het hele regelwerk');
  });

  it('gives the whole path of a werkdocument', () => {
    const rows = taskDetailRows({ ...review, payload: { kind: 'document', target_path: 'mvt/concept.md' } }, null, lawName);
    expect(labels(rows)).toEqual(['Werkdocument', 'Voorgesteld']);
    expect(value(rows, 'Werkdocument')).toBe('mvt/concept.md');
  });

  it('adds what the run came to, leaving out what it does not know', () => {
    const run = makeRun({
      provider: 'claude',
      articles_total: 12,
      articles_with_machine_readable: 9,
      coverage_score: 0.5,
      tasks_open: 2,
      tasks_rejected: 1,
    });
    const rows = taskDetailRows(review, run, lawName);
    expect(labels(rows)).toEqual([
      'Regelwerk',
      'Artikel',
      'Voorgesteld',
      'Taalmodel',
      'Uitvoerbare artikelen',
      'Nieuw verrijkt',
      'Beoordeling',
    ]);
    expect(value(rows, 'Uitvoerbare artikelen')).toBe('9 van 12');
    expect(value(rows, 'Beoordeling')).toBe('1 afgewezen, 2 open');
  });

  it('gives a failure no article and no review, but its attempts', () => {
    const failed = { ...review, task_type: 'job_failed' };
    const rows = taskDetailRows(failed, makeRun({ status: 'failed', attempts: 3, tasks_open: 1 }), lawName);
    expect(labels(rows)).toEqual(['Regelwerk', 'Mislukt', 'Pogingen']);
    expect(value(rows, 'Pogingen')).toBe('3 van 3');
  });

  it('leaves out a moment it cannot read', () => {
    expect(labels(taskDetailRows({ ...review, created_at: 'nonsense' }, null, lawName))).toEqual(['Regelwerk', 'Artikel']);
  });
});
