import { describe, it, expect } from 'vitest';
import { formatSeconds, progressRows, recentTools } from './jobProgress';

describe('formatSeconds', () => {
  it('reads as seconds, minutes or hours', () => {
    expect(formatSeconds(45)).toBe('45 s');
    expect(formatSeconds(725)).toBe('12 min 5 s');
    expect(formatSeconds(3780)).toBe('1 h 3 min');
  });
});

describe('progressRows', () => {
  const progress = {
    phase: 'generate',
    step: 2,
    total_steps: 4,
    worker: {
      call: 'translate',
      calls: 1,
      call_elapsed_secs: 734,
      idle_secs: 3,
      turns: 41,
      tool_calls: 63,
      last_tool: 'Edit laws/x/law.yaml',
      articles: 9,
      articles_with_machine_readable: 4,
      recent_tools: ['Read laws/x/law.yaml', 'Edit laws/x/law.yaml'],
    },
  };

  it('shows what the agent and the worker reported', () => {
    expect(progressRows(progress)).toEqual([
      ['Phase', 'generate (step 2 of 4)'],
      ['Agent call', 'translate'],
      ['Call running for', '12 min 14 s'],
      ['Last output', '3 s ago'],
      ['Articles with machine_readable', '4 / 9'],
      ['Turns', '41'],
      ['Tool calls', '63'],
      ['Last tool', 'Edit laws/x/law.yaml'],
    ]);
    expect(recentTools(progress)).toEqual(['Read laws/x/law.yaml', 'Edit laws/x/law.yaml']);
  });

  // On a stopped job the times were true at the last write only.
  it('leaves out the time rows once the job has stopped', () => {
    const labels = progressRows(progress, false).map(([label]) => label);
    expect(labels).not.toContain('Call running for');
    expect(labels).not.toContain('Last output');
    expect(labels).toContain('Turns');
  });

  it('numbers a later call', () => {
    const rows = progressRows({ worker: { call: 'schema', calls: 3 } });
    expect(rows).toEqual([['Agent call', 'schema (call 3)']]);
  });

  // Jobs from before the worker wrote its own part carry only the agent's note.
  it('handles a bare agent note and junk alike', () => {
    expect(progressRows({ phase: 'research' })).toEqual([['Phase', 'research']]);
    expect(progressRows(null)).toEqual([]);
    expect(progressRows('busy')).toEqual([]);
    expect(progressRows({ worker: { turns: 'many' } })).toEqual([]);
    expect(recentTools({ worker: { recent_tools: ['a', 3] } })).toEqual(['a']);
    expect(recentTools(undefined)).toEqual([]);
  });
});
