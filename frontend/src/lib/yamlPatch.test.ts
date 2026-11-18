import { describe, it, expect } from 'vitest';
import * as jsyaml from 'js-yaml';
import { patchArticle } from './yamlPatch';

/** The patched text; fails the test when the patch was refused. */
function must(out: string | null): string {
  if (out == null) throw new Error('patchArticle returned null');
  return out;
}

type Law = { articles: { text?: string; machine_readable?: Record<string, any> }[] };
const load = (text: string) => jsyaml.load(text) as Law;

// Fictional law with the shapes hand- or agent-authored corpora carry and the
// old `yaml.dump` save destroyed: header comments, comments between keys,
// inline comments, a long unfolded line and literal text blocks.
const FIXTURE = `# Kopcommentaar: deze wet is fictief en dient alleen als testfixture.
# Tweede regel van de kop.
$id: test_commentaarwet
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
name: Testwet commentaarbehoud
articles:
  - number: '1'
    text: |
      Dit is een zeer lange regel die ruim voorbij de tachtig tekens loopt en niet mag worden omgevouwen door de editor.
    # Commentaar tussen text en machine_readable in artikel 1.
    machine_readable:
      definitions:
        drempel:
          value: 100 # inline commentaar
  - number: '2'
    text: |
      Artikel twee, met een regel.
    # Commentaar in artikel 2 dat moet blijven staan.
    machine_readable:
      execution:
        output:
          - name: uitkomst
            type: boolean
  - number: '3'
    text: Korte platte tekst.
`;

function linesOutside(src: string, out: string, needle: string) {
  // Everything before the first line containing `needle` must be identical.
  const idx = src.indexOf(needle);
  return [src.slice(0, idx), out.slice(0, idx)];
}

describe('patchArticle', () => {
  it('returns the input unchanged when nothing is changed', () => {
    expect(patchArticle(FIXTURE, '1', {})).toBe(FIXTURE);
  });

  it('changes only the text of the edited article and keeps every comment', () => {
    const out = must(patchArticle(FIXTURE, '2', { text: 'Artikel twee, gewijzigd.\n' }));
    expect(out).not.toBeNull();
    expect(out).toBe(
      FIXTURE.replace('      Artikel twee, met een regel.\n', '      Artikel twee, gewijzigd.\n'),
    );
  });

  it('re-renders only machine_readable of the edited article', () => {
    const out = must(patchArticle(FIXTURE, '1', {
      machine_readable: { definitions: { drempel: { value: 200 } } },
    }));
    expect(out).not.toBeNull();
    const [before, after] = linesOutside(FIXTURE, out, '    machine_readable:\n      definitions');
    expect(after).toBe(before);
    // Article 2 and 3, including their comments, are byte-identical.
    const tail = FIXTURE.slice(FIXTURE.indexOf("  - number: '2'"));
    expect(out.endsWith(tail)).toBe(true);
    expect(out).toContain('# Commentaar tussen text en machine_readable in artikel 1.');
    expect(load(out).articles[0]!.machine_readable!.definitions.drempel.value).toBe(200);
  });

  it('never refolds a long line elsewhere in the file', () => {
    const out = must(patchArticle(FIXTURE, '3', { text: 'Nieuwe korte tekst.' }));
    expect(out).toContain(
      'Dit is een zeer lange regel die ruim voorbij de tachtig tekens loopt en niet mag worden omgevouwen door de editor.',
    );
    expect(out).toBe(FIXTURE.replace('Korte platte tekst.', 'Nieuwe korte tekst.'));
  });

  it('removes machine_readable without touching the surrounding comments', () => {
    const out = must(patchArticle(FIXTURE, '2', { deleteMachineReadable: true }));
    expect(out).not.toBeNull();
    const parsed = load(out);
    expect(parsed.articles[1]!.machine_readable).toBeUndefined();
    expect(parsed.articles[1]!.text).toBe('Artikel twee, met een regel.\n');
    expect(out).toContain('# Commentaar in artikel 2 dat moet blijven staan.');
    expect(out.startsWith(FIXTURE.slice(0, FIXTURE.indexOf("  - number: '2'")))).toBe(true);
    expect(out.endsWith("  - number: '3'\n    text: Korte platte tekst.\n")).toBe(true);
  });

  it('adds machine_readable to an article that had none', () => {
    const out = must(patchArticle(FIXTURE, '3', {
      machine_readable: { definitions: { factor: { value: 2 } } },
    }));
    expect(out).not.toBeNull();
    expect(out.startsWith(FIXTURE.trimEnd())).toBe(true);
    const parsed = load(out);
    expect(parsed.articles[2]!.machine_readable!.definitions.factor.value).toBe(2);
    expect(parsed.articles[2]!.text).toBe('Korte platte tekst.');
  });

  it('keeps a multi-line text a literal block in the file indentation', () => {
    const out = must(patchArticle(FIXTURE, '1', { text: 'Regel een.\nRegel twee.\n' }));
    expect(out).toContain("  - number: '1'\n    text: |\n      Regel een.\n      Regel twee.\n");
    expect(load(out).articles[0]!.text).toBe('Regel een.\nRegel twee.\n');
  });

  it('follows a non-indented sequence style', () => {
    const flat = `$id: test_vlak
articles:
- number: '1'
  text: Oud.
  machine_readable:
    execution:
      output:
      - name: a
        type: number
`;
    const out = must(patchArticle(flat, '1', {
      machine_readable: { execution: { output: [{ name: 'b', type: 'number' }] } },
    }));
    expect(out).toBe(flat.replace('- name: a', '- name: b'));
  });

  it('returns null when the article does not exist or the YAML is broken', () => {
    expect(patchArticle(FIXTURE, '99', { text: 'x' })).toBeNull();
    expect(patchArticle('articles: [\n', '1', { text: 'x' })).toBeNull();
  });
});

describe('patchArticle with js-yaml values', () => {
  it('writes a Date from js-yaml back as a plain date', () => {
    const src = `$id: test_datum
articles:
  - number: '1'
    text: Tekst.
    machine_readable:
      definitions:
        peildatum:
          value: 2025-01-01
`;
    const mr = load(src).articles[0]!.machine_readable!;
    mr.definitions.einddatum = { value: new Date('2026-01-01T00:00:00Z') };
    const out = must(patchArticle(src, '1', { machine_readable: mr }));
    expect(out).toBe(`${src}        einddatum:\n          value: 2026-01-01\n`.replace('\n\n', '\n'));
  });
});

describe('patchArticle edge cases from review', () => {
  const src = `$id: test_rand
articles:
  - number: '1'
    text: a # houd mij
    machine_readable:
      definitions:
        x:
          value: 1
      execution:
        output:
          - name: uit
            type: number
`;

  it('writes an emptied map as {} rather than null', () => {
    const mr = load(src).articles[0]!.machine_readable!;
    mr.definitions = {};
    const out = must(patchArticle(src, '1', { machine_readable: mr }));
    expect(load(out).articles[0]!.machine_readable!.definitions).toEqual({});
    // The sibling block is untouched.
    expect(out).toContain('      execution:\n        output:\n          - name: uit\n');
  });

  it('keeps a trailing comment on its own value when a key is appended', () => {
    const plain = `$id: test_rand
articles:
  - number: '1'
    text: a # houd mij
`;
    const out = must(patchArticle(plain, '1', { machine_readable: { definitions: { y: { value: 2 } } } }));
    expect(out.startsWith(plain.trimEnd())).toBe(true);
    expect(out).toContain('    text: a # houd mij\n    machine_readable:\n');
  });
});
