import { describe, it, expect } from 'vitest';
import { groupTasks, listByAge } from './taskGroups';

const review = (id: string, lawId: string, article?: string) => ({
  id,
  task_type: 'job_review',
  payload: { law_id: lawId, ...(article === undefined ? {} : { article }) },
});
const failed = (id: string, lawId: string) => ({ id, task_type: 'job_failed', payload: { law_id: lawId } });
const doc = (id: string, path: string) => ({ id, task_type: 'job_review', payload: { kind: 'document', target_path: path } });
const names: Record<string, string> = { wet_a: 'Alpha-wet', wet_b: 'Bèta-wet', wet_z: 'Zeta-wet' };
const nameFor = (id: string) => names[id] ?? id;

const ids = (groups: ReturnType<typeof groupTasks>) => groups.map((g) => g.tasks.map((t) => t.id));

describe('groupTasks', () => {
  it('returns no groups for no work', () => {
    expect(groupTasks([], [], nameFor)).toEqual([]);
  });

  it('puts Werkdocumenten first and the laws after it by name', () => {
    const groups = groupTasks([review('b', 'wet_b', '1'), doc('d', 'x.md'), review('a', 'wet_a', '1')], [], nameFor);
    expect(groups.map((g) => g.name)).toEqual(['Werkdocumenten', 'Alpha-wet', 'Bèta-wet']);
    expect(groups.map((g) => g.key)).toEqual(['werkdocumenten', 'wet:wet_a', 'wet:wet_b']);
  });

  it('sorts articles numerically, with the whole regelwerk first', () => {
    const groups = groupTasks(
      [review('10', 'wet_a', '10'), review('2', 'wet_a', '2'), review('1a', 'wet_a', '1a'), review('all', 'wet_a'), review('1', 'wet_a', '1')],
      [],
      nameFor,
    );
    expect(ids(groups)).toEqual([['all', '1', '1a', '2', '10']]);
  });

  it('sorts werkdocumenten by file name, not by folder', () => {
    const groups = groupTasks([doc('z', 'a-map/zeta.md'), doc('b', 'z-map/beta.md')], [], nameFor);
    expect(ids(groups)).toEqual([['b', 'z']]);
  });

  it('puts a failure first in its group, ahead of what would sort before it', () => {
    const failedDoc = { id: 'zf', task_type: 'job_failed', payload: { target_path: 'zeta.md' } };
    const groups = groupTasks([doc('a', 'alfa.md'), failedDoc], [], nameFor);
    expect(ids(groups)).toEqual([['zf', 'a']]);
  });

  it('puts a group with a failure first in the list', () => {
    const groups = groupTasks([review('a1', 'wet_a', '1'), review('z1', 'wet_z', '1'), failed('zf', 'wet_z')], [], nameFor);
    expect(groups.map((g) => g.name)).toEqual(['Zeta-wet', 'Alpha-wet']);
    expect(ids(groups)).toEqual([['zf', 'z1'], ['a1']]);
  });

  it('files running jobs under their context, also as the only work for a law', () => {
    const groups = groupTasks(
      [review('a1', 'wet_a', '1')],
      [
        { job_id: 'j1', job_type: 'enrich', law_id: 'wet_b' },
        { job_id: 'j2', job_type: 'document_convert', law_id: 'doc:t/x.md', target_path: 'x.md' },
        { job_id: 'j3', job_type: 'enrich', law_id: 'wet_a' },
      ],
      nameFor,
    );
    expect(groups.map((g) => [g.name, g.running.map((j) => j.job_id)])).toEqual([
      ['Werkdocumenten', ['j2']],
      ['Alpha-wet', ['j3']],
      ['Bèta-wet', ['j1']],
    ]);
  });

  it('keeps work without a recognisable context in a last group', () => {
    const groups = groupTasks([{ id: 'u', task_type: 'job_review', payload: {} }, review('a1', 'wet_a', '1')], [], nameFor);
    expect(groups.map((g) => g.key)).toEqual(['wet:wet_a', 'overig']);
    expect(groups[1]).toMatchObject({ context: null, lawId: null, name: 'Overig' });
  });

  it('keeps the server order for ties', () => {
    const groups = groupTasks([review('first', 'wet_a', '3'), review('second', 'wet_a', '3')], [], nameFor);
    expect(ids(groups)).toEqual([['first', 'second']]);
  });
});

describe('listByAge', () => {
  const task = (id: string, created_at?: string) => ({ id, task_type: 'job_review', payload: {}, created_at });
  const job = (job_id: string, created_at?: string) => ({ job_id, job_type: 'enrich', law_id: 'wet_a', created_at });

  it('puts the oldest first, comparing instants rather than strings', () => {
    // As strings, '…00.5Z' sorts after '…01Z'; as instants it is earlier.
    const { tasks } = listByAge(
      [task('new', '2026-11-16T10:00:01Z'), task('half', '2026-11-16T10:00:00.5Z'), task('old', '2026-11-15T10:00:00Z')],
      [],
    );
    expect(tasks.map((t) => t.id)).toEqual(['old', 'half', 'new']);
  });

  it('orders the running jobs oldest first too, apart from the tasks', () => {
    const { tasks, running } = listByAge(
      [task('t', '2026-11-16T00:00:00Z')],
      [job('j2', '2026-11-14T00:00:00Z'), job('j1', '2026-11-13T00:00:00Z')],
    );
    expect(tasks.map((t) => t.id)).toEqual(['t']);
    expect(running.map((j) => j.job_id)).toEqual(['j1', 'j2']);
  });

  it('puts an item without a readable timestamp last, keeping the server order among them', () => {
    const { tasks } = listByAge([task('none'), task('bad', 'not a date'), task('dated', '2026-11-16T00:00:00Z')], []);
    expect(tasks.map((t) => t.id)).toEqual(['dated', 'none', 'bad']);
  });
});
