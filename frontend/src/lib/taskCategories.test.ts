import { describe, it, expect } from 'vitest';
import {
  taskContext,
  taskLawId,
  filterTasks,
  lawContexts,
  categoryCounts,
  isPrioriteit,
  jobContext,
  jobLawId,
  filterRunning,
  ALLE,
  WACHTEN,
  PRIORITEIT,
  WERKDOCUMENTEN,
  WET,
} from './taskCategories';

// De vier payload-vormen zoals de worker ze schrijft (worker.rs:1360/1491/
// 1537/1573) - niet verzonnen, want de hele classificatie hangt aan welke
// velden er wel en niet in zitten.
const docReview = {
  id: 'a',
  task_type: 'job_review',
  payload: { kind: 'document', traject_ref: 't-1', target_path: 'mvt.md' },
};
const convertFailed = {
  id: 'b',
  task_type: 'job_failed',
  // Let op: GEEN `kind` - dat is precies de asymmetrie die deze lib opvangt.
  payload: { traject_ref: 't-1', target_path: 'bijlage.md', error: 'boom' },
};
const lawReview = {
  id: 'c',
  task_type: 'job_review',
  payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1', yaml_path: 'x.yaml' },
};
const enrichFailed = {
  id: 'd',
  task_type: 'job_failed',
  payload: { law_id: 'participatiewet', traject_ref: 't-1', error: 'timeout' },
};

describe('taskContext', () => {
  it('leest het expliciete kind van een document-review', () => {
    expect(taskContext(docReview)).toBe(WERKDOCUMENTEN);
  });

  it('herkent een mislukte conversie ondanks het ontbrekende kind', () => {
    expect(taskContext(convertFailed)).toBe(WERKDOCUMENTEN);
  });

  it('deelt beide wet-taken bij wet in', () => {
    expect(taskContext(lawReview)).toBe(WET);
    expect(taskContext(enrichFailed)).toBe(WET);
  });

  it('geeft null bij een lege of onvolledige payload', () => {
    expect(taskContext({ payload: null })).toBeNull();
    expect(taskContext({ payload: { traject_ref: 't-1' } })).toBeNull();
    expect(taskContext(undefined)).toBeNull();
  });
});

describe('taskLawId', () => {
  it('levert het law_id van een wet-taak', () => {
    expect(taskLawId(lawReview)).toBe('wet_op_de_zorgtoeslag');
  });

  it('levert null voor een werkdocument-taak', () => {
    expect(taskLawId(docReview)).toBeNull();
    expect(taskLawId(convertFailed)).toBeNull();
  });
});

describe('filterTasks', () => {
  const all = [docReview, convertFailed, lawReview, enrichFailed];

  it('geeft bij alle de hele lijst terug', () => {
    expect(filterTasks(all, ALLE)).toHaveLength(4);
  });

  it('filtert op context', () => {
    expect(filterTasks(all, WERKDOCUMENTEN).map((t) => t.id)).toEqual(['a', 'b']);
    expect(filterTasks(all, WET).map((t) => t.id)).toEqual(['c', 'd']);
  });

  it('filtert prioriteit los van de contexten - beide mislukte vormen, ongeacht waar ze over gaan', () => {
    expect(filterTasks(all, PRIORITEIT).map((t) => t.id)).toEqual(['b', 'd']);
  });

  it('filtert op een individuele wet', () => {
    expect(filterTasks(all, WET, 'participatiewet').map((t) => t.id)).toEqual(['d']);
  });

  it('geeft niets terug voor een onbekende categorie', () => {
    expect(filterTasks(all, 'bestaat-niet')).toEqual([]);
    expect(filterTasks(all, null)).toEqual([]);
  });
});

describe('lawContexts', () => {
  const names: Record<string, string> = {
    wet_op_de_zorgtoeslag: 'Wet op de zorgtoeslag',
    participatiewet: 'Participatiewet',
    kieswet: 'Kieswet',
  };
  const nameFor = (id: string) => names[id] ?? id;

  it('telt per wet en sorteert op weergavenaam', () => {
    const contexts = lawContexts([lawReview, enrichFailed, lawReview], [], nameFor);
    // Participatiewet sorteert voor Wet op de zorgtoeslag.
    expect(contexts.map((c) => c.name)).toEqual(['Participatiewet', 'Wet op de zorgtoeslag']);
    const zorgtoeslag = contexts.find((c) => c.lawId === 'wet_op_de_zorgtoeslag');
    expect(zorgtoeslag).toBeDefined();
    expect(zorgtoeslag?.count).toBe(2);
  });

  it('negeert werkdocument-taken', () => {
    expect(lawContexts([docReview, convertFailed])).toEqual([]);
  });

  // Een verrijking die loopt gaat over die wet, ook al is er nog geen taak.
  it('maakt een wet met alleen een lopende verrijking toch een context', () => {
    const contexts = lawContexts([], [{ job_id: 'j1', job_type: 'enrich', law_id: 'kieswet' }], nameFor);
    expect(contexts).toEqual([{ lawId: 'kieswet', count: 1, name: 'Kieswet' }]);
  });

  it('telt taken en lopende jobs voor dezelfde wet bij elkaar op', () => {
    const contexts = lawContexts(
      [lawReview],
      [{ job_id: 'j1', job_type: 'enrich', law_id: 'wet_op_de_zorgtoeslag' }],
      nameFor,
    );
    expect(contexts).toEqual([
      { lawId: 'wet_op_de_zorgtoeslag', count: 2, name: 'Wet op de zorgtoeslag' },
    ]);
  });

  // De doc:-sleutel van een conversie is geen wet - anders stond er een
  // "doc:traject-1/mvt.md"-context in het panel.
  it('maakt van een lopende conversie geen wet-context', () => {
    const convertJob = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:traject-1/mvt.md',
      target_path: 'mvt.md',
    };
    expect(lawContexts([], [convertJob], nameFor)).toEqual([]);
  });
});

// De twee vormen zoals RunningTaskJob ze levert (tasks.rs:128).
const enrichJob = { job_id: 'j1', job_type: 'enrich', law_id: 'kieswet' };
const convertJob = {
  job_id: 'j2',
  job_type: 'document_convert',
  // Synthetische sleutel (corpus_handlers.rs:2943) - géén wet.
  law_id: 'doc:traject-1/nota-van-wijziging.md',
  target_path: 'nota-van-wijziging.md',
};

describe('jobContext', () => {
  it('deelt een lopende verrijking bij haar wet in', () => {
    expect(jobContext(enrichJob)).toBe(WET);
    expect(jobLawId(enrichJob)).toBe('kieswet');
  });

  it('deelt een lopende conversie bij werkdocumenten in, ondanks het law_id-veld', () => {
    expect(jobContext(convertJob)).toBe(WERKDOCUMENTEN);
    // Het doc:-law_id mag nooit als wet naar buiten lekken.
    expect(jobLawId(convertJob)).toBeNull();
  });

  // A law conversion carries a synthetic `lawdoc:` key and a harvest the BWB id
  // of a law not yet in the corpus; neither names a law to file it under.
  it('files a law conversion and a harvest under no context', () => {
    const lawConvert = { job_id: 'j3', job_type: 'law_convert', law_id: 'lawdoc:t-abcd1234/voorstel.pdf' };
    const harvest = { job_id: 'j4', job_type: 'traject_harvest', law_id: 'BWBR0002399' };
    expect(jobContext(lawConvert)).toBeNull();
    expect(jobLawId(lawConvert)).toBeNull();
    expect(jobContext(harvest)).toBeNull();
    expect(lawContexts([], [lawConvert, harvest])).toEqual([]);
  });

  it('overleeft een lege job', () => {
    expect(jobContext(undefined)).toBeNull();
    expect(jobContext({})).toBeNull();
    expect(jobLawId(undefined)).toBeNull();
  });
});

describe('filterRunning', () => {
  const jobs = [enrichJob, convertJob];

  it('geeft bij wachten en alle alle lopende jobs', () => {
    expect(filterRunning(jobs, WACHTEN)).toHaveLength(2);
    expect(filterRunning(jobs, ALLE)).toHaveLength(2);
  });

  it('geeft een lopende conversie aan de werkdocumenten-context', () => {
    expect(filterRunning(jobs, WERKDOCUMENTEN).map((j) => j.job_id)).toEqual(['j2']);
  });

  it('geeft een lopende verrijking aan haar eigen wet-context', () => {
    expect(filterRunning(jobs, WET, 'kieswet').map((j) => j.job_id)).toEqual(['j1']);
    expect(filterRunning(jobs, WET, 'andere_wet')).toEqual([]);
  });

  it('houdt lopende jobs buiten prioriteit - er valt niets te doen', () => {
    expect(filterRunning(jobs, PRIORITEIT)).toEqual([]);
  });
});

describe('isPrioriteit', () => {
  // Vandaag is prioriteit precies "de job is mislukt" - er is nog geen
  // deadlineveld op tasks. Beide mislukte vormen tellen, ongeacht context.
  it('rekent mislukte taken tot prioriteit', () => {
    expect(isPrioriteit(convertFailed)).toBe(true);
    expect(isPrioriteit(enrichFailed)).toBe(true);
  });

  it('rekent open reviews niet tot prioriteit', () => {
    expect(isPrioriteit(docReview)).toBe(false);
    expect(isPrioriteit(lawReview)).toBe(false);
  });

  it('overleeft een lege taak', () => {
    expect(isPrioriteit(undefined)).toBe(false);
    expect(isPrioriteit({})).toBe(false);
  });
});

describe('categoryCounts', () => {
  const all = [docReview, convertFailed, lawReview, enrichFailed];

  // Wetten tellen niet mee: die hebben geen verzamel-ingang, elke wet telt
  // voor zichzelf via lawContexts.
  it('telt de vaste ingangen', () => {
    expect(categoryCounts(all)).toEqual({
      [ALLE]: 4,
      [PRIORITEIT]: 2,
      [WACHTEN]: 0,
      [WERKDOCUMENTEN]: 2,
    });
  });

  it('telt lopende jobs mee in alle, zodat die ingang niet minder toont dan hij belooft', () => {
    const counts = categoryCounts(all, [{ job_id: 'j1' }, { job_id: 'j2' }]);
    expect(counts[ALLE]).toBe(6);
    expect(counts[WACHTEN]).toBe(2);
    // Contexten blijven over taken gaan.
    expect(counts[WERKDOCUMENTEN]).toBe(2);
  });

  it('overleeft een lege lijst', () => {
    expect(categoryCounts([])).toEqual({
      [ALLE]: 0,
      [PRIORITEIT]: 0,
      [WACHTEN]: 0,
      [WERKDOCUMENTEN]: 0,
    });
  });
});
