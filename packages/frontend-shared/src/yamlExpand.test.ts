import { describe, expect, it } from 'vitest';
import { initiallyOpen, nodeAt, nodePaths, onPath, type OpenQuery } from './yamlExpand';

// De paden zoals ze voor de zorgtoeslagwet in demo-config.yaml staan.
const paths = [
  'articles.*.machine_readable.execution.input.is_verzekerde.source',
  'articles.*.machine_readable.execution.output',
  'articles.*.machine_readable.execution.actions.voldoet_aan_voorwaarden.value.conditions.0',
  'articles.*.machine_readable.execution.actions.voldoet_aan_voorwaarden.value.conditions.1',
];

const open = (path: string, extra: Partial<OpenQuery> = {}) => initiallyOpen({ path, depth: path.split('.').length, all: null, paths, ...extra });

describe('de voorbereide stand van de YAML-boom', () => {
  it('toont de artikelen, maar klapt ze niet zelf open', () => {
    expect(open('articles')).toBe(true);
    expect(open('articles.2')).toBe(false);
    expect(open('articles.4')).toBe(false);
  });

  it('legt de weg naar een geconfigureerd pad klaar, zodat één klik op het artikel volstaat', () => {
    expect(open('articles.2.machine_readable')).toBe(true);
    expect(open('articles.2.machine_readable.execution')).toBe(true);
    expect(open('articles.2.machine_readable.execution.input')).toBe(true);
    expect(open('articles.2.machine_readable.execution.input.is_verzekerde')).toBe(true);
    expect(open('articles.2.machine_readable.execution.input.is_verzekerde.source')).toBe(true);
    expect(open('articles.2.machine_readable.execution.output')).toBe(true);
  });

  it('laat de buren dicht, zodat het publiek op de goede regels landt', () => {
    expect(open('articles.2.machine_readable.execution.input.inkomen')).toBe(false);
    expect(open('articles.2.machine_readable.execution.parameters')).toBe(false);
    expect(open('articles.2.machine_readable.execution.actions.hoogte_toeslag')).toBe(false);
    expect(open('articles.2.machine_readable.definitions')).toBe(false);
    expect(open('articles.2.legal_basis')).toBe(false);
  });

  it('geldt voor elk artikel, want `*` matcht het nummer', () => {
    expect(open('articles.4.machine_readable.execution.output')).toBe(true);
    expect(open('articles.4.machine_readable.execution.parameters')).toBe(false);
  });

  it('houdt de wortel open en volgt de knoppen alles-open en alles-dicht', () => {
    expect(initiallyOpen({ path: 'articles', depth: 0, all: null, paths })).toBe(true);
    expect(open('articles.2', { all: true })).toBe(true);
    expect(open('articles.2.machine_readable', { all: false })).toBe(false);
  });

  it('komt zonder geconfigureerde paden niet verder dan de artikelenlijst', () => {
    expect(initiallyOpen({ path: 'articles', depth: 1, all: null, paths: [] })).toBe(true);
    expect(initiallyOpen({ path: 'articles.2.machine_readable', depth: 3, all: null, paths: [] })).toBe(false);
  });
});

// De precariobelasting configureert ondiepe paden: die eindigen op het blok
// zelf in plaats van diep in een berekening (demo-config.yaml).
const ondiep = [
  'articles.*.machine_readable.execution.input',
  'articles.*.machine_readable.execution.output',
  'articles.*.machine_readable.execution.actions',
];

describe('een ondiep geconfigureerd pad klapt niet zijn hele deelboom uit', () => {
  const open = (path: string) => initiallyOpen({ path, depth: path.split('.').length, all: null, paths: ondiep });
  const base = 'articles.1.machine_readable.execution';

  it('opent de weg ernaartoe en het blok zelf', () => {
    expect(open('articles.1.machine_readable')).toBe(true);
    expect(open(base)).toBe(true);
    expect(open(`${base}.actions`)).toBe(true);
    expect(open(`${base}.input`)).toBe(true);
  });

  it('laat alles ónder dat blok dicht, ook diep weg', () => {
    expect(open(`${base}.actions.bedrag`)).toBe(false);
    expect(open(`${base}.actions.bedrag.value`)).toBe(false);
    expect(open(`${base}.actions.bedrag.value.conditions.0`)).toBe(false);
    expect(open(`${base}.input.tarief.source`)).toBe(false);
  });
});

describe('onPath', () => {
  it('herkent het knooppunt zelf en de knopen erboven', () => {
    expect(onPath('a.b.c', 'a.b')).toBe(true);
    expect(onPath('a.b.c', 'a.b.c')).toBe(true);
  });

  it('wijst af wat eronder ligt: daar houdt het pad op', () => {
    expect(onPath('a.b.c', 'a.b.c.d')).toBe(false);
  });

  it('wijst een buur af', () => {
    expect(onPath('a.b.c', 'a.x')).toBe(false);
    expect(onPath('a.b.c', 'a.b.x')).toBe(false);
  });

  it('matcht elk segment op `*`', () => {
    expect(onPath('a.*.c', 'a.7.c')).toBe(true);
    expect(onPath('a.*.c', 'a.7.x')).toBe(false);
  });
});

// Zoals folded_paths voor de zorgtoeslagwet: actions en de berekening daarin
// klapt de presentator zelf open, maar wat eronder ligt staat al klaar.
describe('een ingeklapte knoop op de weg begint dicht, met zijn inhoud voorbereid', () => {
  const zorgPaths = [
    'articles.*.machine_readable.execution.parameters',
    'articles.*.machine_readable.execution.actions.voldoet_aan_voorwaarden.value.conditions.0',
    'articles.*.machine_readable.execution.actions.hoogte_toeslag.value.cases.0.then.value.cases.0.then.cases.0.when.subject.values',
  ];
  const folded = [
    'articles.*.machine_readable.execution.actions',
    'articles.*.machine_readable.execution.actions.hoogte_toeslag',
  ];
  const open = (path: string) => initiallyOpen({ path, depth: path.split('.').length, all: null, paths: zorgPaths, folded });
  const actions = 'articles.2.machine_readable.execution.actions';

  it('laat actions en hoogte_toeslag dicht beginnen', () => {
    expect(open(actions)).toBe(false);
    expect(open(`${actions}.hoogte_toeslag`)).toBe(false);
  });

  it('zet wat eronder ligt klaar voor de klik', () => {
    expect(open(`${actions}.voldoet_aan_voorwaarden`)).toBe(true);
    expect(open(`${actions}.voldoet_aan_voorwaarden.value.conditions.0`)).toBe(true);
    expect(open(`${actions}.hoogte_toeslag.value.cases.0.then`)).toBe(true);
    expect(open(`${actions}.hoogte_toeslag.value.cases.0.then.value.cases.0.then.cases.0.when.subject.values`)).toBe(true);
  });

  it('matcht alleen de knoop zelf, niet wat erboven of eronder ligt', () => {
    expect(open('articles.2.machine_readable.execution')).toBe(true);
    expect(open('articles.2.machine_readable.execution.parameters')).toBe(true);
    expect(open(`${actions}.hoogte_toeslag.value.cases.0.then.value.cases.0.when`)).toBe(false);
    expect(open(`${actions}.hoogte_toeslag.value.cases.1`)).toBe(false);
  });

  it('wijkt voor alles-open', () => {
    expect(initiallyOpen({ path: actions, depth: 4, all: true, paths: zorgPaths, folded })).toBe(true);
  });
});

describe('een artikel dat zelf als pad genoemd is', () => {
  const paths = ['articles.1', 'articles.*.machine_readable.execution.output'];
  const open = (path: string) => initiallyOpen({ path, depth: path.split('.').length, all: null, paths });

  it('staat meteen open, de andere artikelen niet', () => {
    expect(open('articles.1')).toBe(true);
    expect(open('articles.2')).toBe(false);
  });
});

describe('de knopen van een document', () => {
  const doc = { articles: [{ number: '2.3', execution: { actions: [{ output: 'x', value: { conditions: ['$a', '$b'] } }] } }] };

  it('noemt elk pad zoals YamlNode het labelt', () => {
    expect(nodePaths(doc)).toContain('articles.2_3.execution.actions.x.value.conditions.1');
  });

  it('vindt de knoop op een pad, en niets op een pad dat er niet is', () => {
    expect(nodeAt(doc, 'articles.2_3.execution.actions.x.value.conditions.1')).toBe('$b');
    expect(nodeAt(doc, '')).toBe(doc);
    expect(nodeAt(doc, 'articles.9')).toBeUndefined();
  });
});
