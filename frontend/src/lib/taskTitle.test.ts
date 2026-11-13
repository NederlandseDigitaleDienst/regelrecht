import { describe, it, expect } from 'vitest';
import { taskTitle, runningTitle, taskRowTitle, taskRowLabel, runningRowTitle } from './taskTitle';

// De weergavenaam-resolver zoals de corpuslezer 'm levert.
const lawName = (id: string | null | undefined) =>
  id === 'wet_op_de_zorgtoeslag' ? 'Wet op de zorgtoeslag' : String(id);

describe('taskTitle', () => {
  it('schrijft een wet-review in de gebiedende wijs, met de weergavenaam', () => {
    const task = {
      task_type: 'job_review',
      title: 'Verrijking beoordelen: wet_op_de_zorgtoeslag',
      payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel verrijking van Wet op de zorgtoeslag');
  });

  it('noemt het artikel als de taak er een aanwijst', () => {
    const task = {
      task_type: 'job_review',
      title: 'Verrijking beoordelen: wet_op_de_zorgtoeslag artikel 2',
      payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1', article: '2' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel artikel 2 van Wet op de zorgtoeslag');
  });

  it('noemt geen artikel bij een mislukte verrijking - die geldt de hele wet', () => {
    const task = {
      task_type: 'job_failed',
      payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1', article: '2' },
    };
    expect(taskTitle(task, lawName)).toBe('Verrijking van Wet op de zorgtoeslag is mislukt');
  });

  it('schrijft een document-review in de gebiedende wijs', () => {
    const task = {
      task_type: 'job_review',
      title: 'Werkdocument beoordelen: mvt.md',
      payload: { kind: 'document', traject_ref: 't-1', target_path: 'mvt.md' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel werkdocument mvt.md');
  });

  it('kort een documentpad in tot de bestandsnaam', () => {
    const task = {
      task_type: 'job_review',
      payload: { kind: 'document', target_path: 'analyses/deel-2/rapport.md' },
    };
    expect(taskTitle(task, lawName)).toBe('Beoordeel werkdocument rapport.md');
  });

  // Een mislukking is geen opdracht: mededelend, niet gebiedend.
  it('meldt een mislukte verrijking, niet gebiedend', () => {
    const task = {
      task_type: 'job_failed',
      title: 'Verrijking mislukt: wet_op_de_zorgtoeslag',
      payload: { law_id: 'wet_op_de_zorgtoeslag', error: 'boom' },
    };
    expect(taskTitle(task, lawName)).toBe('Verrijking van Wet op de zorgtoeslag is mislukt');
  });

  it('meldt een mislukte conversie, ook zonder kind-veld in de payload', () => {
    const task = {
      task_type: 'job_failed',
      title: 'Conversie mislukt: bijlage.md',
      payload: { target_path: 'bijlage.md', error: 'boom' },
    };
    expect(taskTitle(task, lawName)).toBe('Conversie van bijlage.md is mislukt');
  });

  it('valt terug op de servertitel bij een onbekende payload-vorm', () => {
    const task = { task_type: 'job_review', title: 'Iets nieuws: xyz', payload: {} };
    expect(taskTitle(task, lawName)).toBe('Iets nieuws: xyz');
  });

  it('geeft een lege string als er niets te tonen valt', () => {
    expect(taskTitle({ payload: {} })).toBe('');
    expect(taskTitle(undefined)).toBe('');
  });

  it('valt zonder resolver terug op het rauwe law_id', () => {
    const task = { task_type: 'job_review', payload: { law_id: 'kieswet' } };
    expect(taskTitle(task)).toBe('Beoordeel verrijking van kieswet');
  });
});

describe('runningTitle', () => {
  it('schrijft een lopende verrijking als wachten, met de weergavenaam', () => {
    const job = { job_id: 'j1', job_type: 'enrich', law_id: 'wet_op_de_zorgtoeslag' };
    expect(runningTitle(job, lawName)).toBe('Wachten op verrijking van Wet op de zorgtoeslag');
  });

  it('schrijft een lopende conversie als wachten, met de bestandsnaam', () => {
    const job = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:t-1/analyses/rapport.md',
      target_path: 'analyses/rapport.md',
    };
    expect(runningTitle(job, lawName)).toBe('Wachten op conversie van rapport.md');
  });

  it('valt terug op een generieke naam als een conversie geen target_path heeft', () => {
    const job = { job_id: 'j3', job_type: 'document_convert' };
    expect(runningTitle(job, lawName)).toBe('Wachten op conversie van werkdocument');
  });

  it('schrijft een lopende wet-conversie als wachten, met de bestandsnaam', () => {
    const job = {
      job_id: 'j4',
      job_type: 'law_convert',
      target_path: 'uploads/zorgtoeslag.pdf',
    };
    expect(runningTitle(job, lawName)).toBe('Wachten op conversie van zorgtoeslag.pdf naar een regelwerk');
  });

  it('leaves the law out of the title when the job carries no law_id', () => {
    expect(runningTitle({ job_id: 'j6', job_type: 'enrich' })).toBe('Wachten op verrijking');
    expect(runningTitle({ job_id: 'j7', job_type: 'traject_harvest' }, () => '')).toBe(
      'Wachten op wet ophalen',
    );
  });

  it('schrijft een lopende traject-harvest als wachten, met het BWB-id', () => {
    const job = { job_id: 'j5', job_type: 'traject_harvest', law_id: 'BWBR0002399' };
    expect(runningTitle(job, lawName)).toBe('Wachten op wet ophalen van BWBR0002399');
  });
});

// Under a group heading the law is already named, so the row leaves it out.
describe('taskRowTitle', () => {
  const law = (payload: Record<string, unknown>, task_type = 'job_review') => ({
    task_type,
    title: 'server title',
    payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1', ...payload },
  });

  it('names only the article of a law review', () => {
    expect(taskRowTitle(law({ article: '2' }))).toBe('Beoordeel artikel 2');
  });

  it('says a review without an article covers the whole regelwerk', () => {
    expect(taskRowTitle(law({}))).toBe('Beoordeel het hele regelwerk');
  });

  it('reports a failed enrichment without the law', () => {
    expect(taskRowTitle(law({}, 'job_failed'))).toBe('Verrijking is mislukt');
  });

  it('keeps the file name for a werkdocument, which the heading does not give', () => {
    const doc = { task_type: 'job_review', payload: { kind: 'document', target_path: 'mvt/concept.md' } };
    expect(taskRowTitle(doc)).toBe('Beoordeel concept.md');
    const failed = { task_type: 'job_failed', payload: { target_path: 'mvt/concept.md' } };
    expect(taskRowTitle(failed)).toBe('Conversie van concept.md is mislukt');
  });

  it('falls back to the server title for an unknown payload', () => {
    expect(taskRowTitle({ task_type: 'job_review', title: 'Iets anders', payload: {} })).toBe('Iets anders');
  });
});

describe('runningRowTitle', () => {
  it('leaves the law out of a running enrichment', () => {
    expect(runningRowTitle({ job_type: 'enrich', law_id: 'wet_op_de_zorgtoeslag' })).toBe('Wachten op verrijking');
  });

  it('keeps the file name of a running conversion', () => {
    expect(runningRowTitle({ job_type: 'document_convert', law_id: 'doc:x/nota.md', target_path: 'nota.md' })).toBe(
      'Wachten op conversie van nota.md',
    );
  });
});

describe('taskRowLabel', () => {
  const lawName = (id: string) => (id === 'wet_op_de_zorgtoeslag' ? 'Wet op de zorgtoeslag' : id);
  const law = (payload: Record<string, unknown>, task_type = 'job_review') => ({
    task_type,
    payload: { law_id: 'wet_op_de_zorgtoeslag', ...payload },
  });

  // Each label starts with the row's visible title (WCAG 2.5.3).
  it('completes the row title with the law', () => {
    expect(taskRowLabel(law({ article: '2' }), lawName)).toBe('Beoordeel artikel 2 van Wet op de zorgtoeslag');
    expect(taskRowLabel(law({}), lawName)).toBe('Beoordeel het hele regelwerk van Wet op de zorgtoeslag');
    expect(taskRowLabel(law({}, 'job_failed'), lawName)).toBe('Verrijking is mislukt: Wet op de zorgtoeslag');
  });

  it('leaves a werkdocument row as it is: the file name is already in it', () => {
    const doc = { task_type: 'job_review', payload: { kind: 'document', target_path: 'nota.md' } };
    expect(taskRowLabel(doc, lawName)).toBe('Beoordeel nota.md');
  });
});

describe('runningRowTitle without a law heading', () => {
  it('keeps the full title of a harvest, which sits under no law', () => {
    expect(runningRowTitle({ job_type: 'traject_harvest', law_id: 'BWBR0002399' }, (id) => String(id))).toBe(
      'Wachten op wet ophalen van BWBR0002399',
    );
  });
});
