// De YAML achter een artikel: het juiste stuk uit het regelbestand, letterlijk.
import { expect, it } from 'vitest';
import { zetBron } from './bron.js';
import { yamlVanArtikel } from './sim.js';

const REGELS = `$id: voorbeeld
articles:
- number: '1.1'
  text: Eerste artikel.
  machine_readable:
    endpoint: art_1_1
- number: '2.1'
  text: |-
    Tweede artikel,
    over twee regels.
  machine_readable:
    endpoint: art_2_1
`;

zetBron({
  bestanden: {
    'regels/voorbeeld.yaml': REGELS,
    'sim.json.yaml': JSON.stringify({
      index: {
        'r|2.1': { vorm: 'GEGEVEN', law: 'voorbeeld', output: 'art_2_1', bestand: 'voorbeeld.yaml' },
        'r|3.1': { vorm: 'geen-tekst', law: null, output: null, bestand: null },
      },
    }),
  },
});

it('geeft het artikel met zijn endpoint, en niet het artikel ervoor', async () => {
  const y = await yamlVanArtikel('r|2.1');
  expect(y.bestand).toBe('voorbeeld.yaml');
  expect(y.yaml.startsWith("- number: '2.1'")).toBe(true);
  expect(y.yaml).toContain('over twee regels.');
  expect(y.yaml).not.toContain('Eerste artikel');
});

it('zegt waarom er geen YAML is', async () => {
  expect((await yamlVanArtikel('r|3.1')).reden).toMatch(/geen tekst/);
  expect((await yamlVanArtikel('r|9.9')).reden).toMatch(/geen YAML/);
});
