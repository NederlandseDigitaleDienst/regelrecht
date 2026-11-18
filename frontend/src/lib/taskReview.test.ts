import { describe, it, expect } from 'vitest';
import router from '../router';
import {
  reviewTarget,
  sameTraject,
  proposalDivergence,
  taskInTraject,
  jobInTraject,
  taskNamesTraject,
} from './taskReview';
import type { ReviewTarget } from './taskReview';

// Resolve a target that the test expects to exist; a null target fails here
// instead of reaching router.resolve.
function resolveTarget(target: ReviewTarget | null) {
  if (!target) throw new Error('reviewTarget returned null');
  return router.resolve(target);
}

describe('reviewTarget', () => {
  it('bouwt de editor-traject-route met ?task= voor een job_review-taak', () => {
    const task = {
      id: 't1',
      task_type: 'job_review',
      payload: { traject_ref: 'mijn-traject-1a2b3c4d', law_id: 'wet_op_de_zorgtoeslag' },
    };
    const target = reviewTarget(task);
    expect(target).not.toBeNull();
    const resolved = resolveTarget(target);
    expect(resolved.name).toBe('editor-traject');
    expect(resolved.params.trajectRef).toBe('mijn-traject-1a2b3c4d');
    expect(resolved.params.lawId).toBe('wet_op_de_zorgtoeslag');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/editor/wet_op_de_zorgtoeslag?task=t1',
    );
  });

  it('neemt het artikel uit de payload op in de editor-route', () => {
    const task = {
      id: 't1a',
      task_type: 'job_review',
      payload: {
        traject_ref: 'mijn-traject-1a2b3c4d',
        law_id: 'wet_op_de_zorgtoeslag',
        article: '2',
      },
    };
    const resolved = resolveTarget(reviewTarget(task));
    expect(resolved.params.articleNumber).toBe('2');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/editor/wet_op_de_zorgtoeslag/2?task=t1a',
    );
  });

  it('geeft null voor een taak zonder traject_ref in de payload', () => {
    expect(reviewTarget({ id: 't2', payload: { law_id: 'wet_op_de_zorgtoeslag' } })).toBeNull();
  });

  it('geeft null voor een taak zonder law_id in de payload', () => {
    expect(reviewTarget({ id: 't3', payload: { traject_ref: 'mijn-traject-1a2b3c4d' } })).toBeNull();
  });

  it('geeft null voor een taak zonder payload', () => {
    expect(reviewTarget({ id: 't4' })).toBeNull();
  });

  it('bouwt de werkdocumenten-route met ?task= voor een document-review-taak', () => {
    const task = {
      id: 't5',
      task_type: 'job_review',
      payload: {
        kind: 'document',
        traject_ref: 'mijn-traject-1a2b3c4d',
        target_path: 'bijv-rapport.md',
      },
    };
    const target = reviewTarget(task);
    expect(target).not.toBeNull();
    const resolved = resolveTarget(target);
    expect(resolved.name).toBe('werkdocumenten-traject');
    expect(resolved.params.trajectRef).toBe('mijn-traject-1a2b3c4d');
    expect(resolved.params.docPath).toBe('bijv-rapport.md');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/werkdocumenten/bijv-rapport.md?task=t5',
    );
  });

  it('geeft null voor een document-review-taak zonder target_path', () => {
    expect(
      reviewTarget({
        id: 't6',
        payload: { kind: 'document', traject_ref: 'mijn-traject-1a2b3c4d' },
      }),
    ).toBeNull();
  });

  it('geeft null voor een document-review-taak zonder traject_ref', () => {
    expect(
      reviewTarget({ id: 't7', payload: { kind: 'document', target_path: 'bijv-rapport.md' } }),
    ).toBeNull();
  });

  it('routeert een law_create-taak naar de editor-traject-route (wet-branch)', () => {
    const task = {
      id: 't8',
      task_type: 'job_review',
      payload: {
        kind: 'law_create',
        traject_ref: 'mijn-traject-1a2b3c4d',
        law_id: 'werkinstructie_toetsing',
      },
    };
    const target = reviewTarget(task);
    expect(target).not.toBeNull();
    const resolved = resolveTarget(target);
    expect(resolved.name).toBe('editor-traject');
    expect(resolved.params.lawId).toBe('werkinstructie_toetsing');
    expect(resolved.fullPath).toBe(
      '/trajecten/mijn-traject-1a2b3c4d/editor/werkinstructie_toetsing?task=t8',
    );
  });
});

describe('sameTraject', () => {
  it('herkent hetzelfde traject', () => {
    expect(sameTraject('mijn-traject-1a2b3c4d', 'mijn-traject-1a2b3c4d')).toBe(true);
  });

  it('onderscheidt twee trajecten', () => {
    expect(sameTraject('mijn-traject-1a2b3c4d', 'ander-traject-9f8e7d6c')).toBe(false);
  });

  // Het slug-deel komt uit de naam van het moment van aanmaken; hernoemen
  // verandert het zonder dat het traject een ander traject wordt. De backend
  // zoekt alleen op de 8-hex-staart op, dus dit moet meebewegen - anders
  // verdwijnt een klaarstaand voorstel uit de pane zodra iemand zijn traject
  // hernoemt.
  it('negeert het cosmetische slug-deel, zodat hernoemen niets breekt', () => {
    expect(sameTraject('oude-naam-1a2b3c4d', 'nieuwe-naam-1a2b3c4d')).toBe(true);
  });

  it('vergelijkt de hex-staart hoofdletterongevoelig', () => {
    expect(sameTraject('traject-1A2B3C4D', 'traject-1a2b3c4d')).toBe(true);
  });

  // Een ref zonder herkenbare staart hoort bij geen enkel traject; "matcht met
  // alles" zou precies de kaping terugbrengen die de traject-check afsluit.
  it('matcht niets bij een ontbrekende of vormloze ref', () => {
    expect(sameTraject(null, 'mijn-traject-1a2b3c4d')).toBe(false);
    expect(sameTraject('mijn-traject-1a2b3c4d', undefined)).toBe(false);
    expect(sameTraject('', '')).toBe(false);
    expect(sameTraject('geen-hex-staart', 'geen-hex-staart')).toBe(false);
  });
});

describe('proposalDivergence', () => {
  it('vindt het enige afwijkende artikel als target zonder hidden changes', () => {
    const current = [{ number: '1', text: 'oud' }];
    const proposed = [{ number: '1', text: 'nieuw' }];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toEqual(proposed[0]);
    expect(result.hiddenChanges).toBe(false);
  });

  it('geeft geen target als niets afwijkt', () => {
    const current = [{ number: '1', text: 'zelfde' }];
    const proposed = [{ number: '1', text: 'zelfde' }];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toBeNull();
    expect(result.hiddenChanges).toBe(false);
  });

  it('markeert een tweede afwijkend artikel als hidden change', () => {
    const current = [
      { number: '1', text: 'oud-1' },
      { number: '2', text: 'oud-2' },
    ];
    const proposed = [
      { number: '1', text: 'nieuw-1' },
      { number: '2', text: 'nieuw-2' },
    ];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toEqual(proposed[0]);
    expect(result.hiddenChanges).toBe(true);
  });

  it('markeert een voorgesteld artikel dat de huidige wet niet heeft als hidden change', () => {
    const current = [{ number: '1', text: 'oud' }];
    const proposed = [
      { number: '1', text: 'oud' },
      { number: '2', text: 'nieuw artikel' },
    ];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toBeNull();
    expect(result.hiddenChanges).toBe(true);
  });

  it('markeert een artikel-verwijdering (huidige wet heeft artikel 2, voorstel niet) als hidden change', () => {
    const current = [
      { number: '1', text: 'blijft' },
      { number: '2', text: 'wordt verwijderd' },
    ];
    const proposed = [{ number: '1', text: 'blijft' }];
    const result = proposalDivergence(current, proposed);
    expect(result.target).toBeNull();
    expect(result.hiddenChanges).toBe(true);
  });

  it('behandelt lege/ontbrekende input als geen divergentie', () => {
    expect(proposalDivergence(undefined, undefined)).toEqual({
      target: null,
      hiddenChanges: false,
    });
    expect(proposalDivergence([], [])).toEqual({ target: null, hiddenChanges: false });
  });

  it('pakt het aangewezen artikel, ook als een eerder artikel ook afwijkt', () => {
    const current = [
      { number: '1', text: 'oud-1' },
      { number: '2', text: 'oud-2' },
    ];
    const proposed = [
      { number: '1', text: 'nieuw-1' },
      { number: '2', text: 'nieuw-2' },
    ];
    const result = proposalDivergence(current, proposed, '2');
    expect(result.target).toEqual(proposed[1]);
    // Artikel 1 hangt aan zijn eigen taak, dus het is hier geen verborgen
    // wijziging waar de banner naar hoeft te verwijzen.
    expect(result.hiddenChanges).toBe(false);
  });

  it('geeft geen target als het aangewezen artikel niet afwijkt', () => {
    const current = [{ number: '1', text: 'zelfde' }];
    const proposed = [{ number: '1', text: 'zelfde' }];
    expect(proposalDivergence(current, proposed, '1')).toEqual({
      target: null,
      hiddenChanges: false,
    });
  });

  it('geeft geen target als het aangewezen artikel in de wet of het voorstel ontbreekt', () => {
    const current = [{ number: '1', text: 'oud' }];
    const proposed = [{ number: '1', text: 'nieuw' }];
    expect(proposalDivergence(current, proposed, '9')).toEqual({
      target: null,
      hiddenChanges: false,
    });
    expect(proposalDivergence([], proposed, '1')).toEqual({
      target: null,
      hiddenChanges: false,
    });
  });

  it('vergelijkt het nummer als tekst, zodat een numeriek payload-artikel raak is', () => {
    const current = [{ number: '2', text: 'oud' }];
    const proposed = [{ number: 2, text: 'nieuw' }];
    expect(proposalDivergence(current, proposed, 2).target).toEqual(proposed[0]);
  });
});

describe('taskInTraject', () => {
  const HERE = 'kimi-test-2a72312c';

  it('decides on traject_id when the task has it', () => {
    expect(taskInTraject({ traject_id: '2a72312c-67a3-4527-9033-8f4dd52b1c82' }, HERE)).toBe(true);
    expect(taskInTraject({ traject_id: '729b827f-797b-43b4-9ec7-9713eb579ce3' }, HERE)).toBe(false);
  });

  it('lets traject_id win over a payload ref that says otherwise', () => {
    const task = {
      traject_id: '729b827f-797b-43b4-9ec7-9713eb579ce3',
      payload: { traject_ref: HERE },
    };
    expect(taskInTraject(task, HERE)).toBe(false);
  });

  it('falls back to payload.traject_ref, matching on the id part only', () => {
    expect(taskInTraject({ payload: { traject_ref: 'old-name-2a72312c' } }, HERE)).toBe(true);
    expect(taskInTraject({ payload: { traject_ref: 'voorbeeld-729b827f' } }, HERE)).toBe(false);
  });

  it('keeps a task that names no traject visible, so it can still be dismissed', () => {
    expect(taskInTraject({ payload: { law_id: 'test_wet' } }, HERE)).toBe(true);
  });

  it('matches nothing without a traject to compare with', () => {
    expect(taskInTraject({ traject_id: '2a72312c-67a3-4527-9033-8f4dd52b1c82' }, null)).toBe(false);
    expect(taskInTraject({ traject_id: '2a72312c-67a3-4527-9033-8f4dd52b1c82' }, 'foo')).toBe(false);
  });

  it('ignores case on both sides', () => {
    expect(taskInTraject({ traject_id: '2A72312C-67A3-4527-9033-8F4DD52B1C82' }, 'Kimi-Test-2A72312C')).toBe(true);
  });

  it('falls back to the payload when traject_id is too short to compare', () => {
    expect(taskInTraject({ traject_id: 'abc', payload: { traject_ref: 'x-2a72312c' } }, HERE)).toBe(true);
  });
});

describe('taskNamesTraject', () => {
  it('is true for a task with a traject_id or a payload ref with an id part', () => {
    expect(taskNamesTraject({ traject_id: '2a72312c-67a3-4527-9033-8f4dd52b1c82' })).toBe(true);
    expect(taskNamesTraject({ payload: { traject_ref: 'x-2a72312c' } })).toBe(true);
  });

  it('is false for a task that names no traject', () => {
    expect(taskNamesTraject({ payload: { law_id: 'test_wet' } })).toBe(false);
    expect(taskNamesTraject({ payload: { traject_ref: 't-1' } })).toBe(false);
  });
});

describe('jobInTraject', () => {
  it('matches a running job on the id part of its traject_ref', () => {
    expect(jobInTraject({ traject_ref: 'old-name-2a72312c' }, 'kimi-test-2a72312c')).toBe(true);
    expect(jobInTraject({ traject_ref: 'voorbeeld-729b827f' }, 'kimi-test-2a72312c')).toBe(false);
    expect(jobInTraject({ traject_ref: null }, 'kimi-test-2a72312c')).toBe(true);
  });
});
