// De indeling waar de uploadbevestiging haar tekst en haar vinkje op baseert.
// Belangrijkste eigenschap: alles wat niet expliciet passthrough of
// deterministisch is, telt als "alleen met AI" - de onbekende kant valt naar de
// voorzichtige uitkomst, niet naar "zal wel goed zijn".
import { describe, it, expect } from 'vitest';
import {
  classifyUpload,
  extensionOf,
  PASSTHROUGH,
  DETERMINISTIC,
  LLM_ONLY,
  UNKNOWN,
} from './uploadFormats';

// Zoals de server 'm teruggeeft (afgeleid van de convertertabel in de pipeline).
const FORMATS = {
  passthrough: ['md', 'markdown'],
  deterministic: ['docx', 'odt', 'rtf', 'html', 'htm', 'epub', 'fb2', 'pdf'],
};

describe('extensionOf', () => {
  it('geeft de kleingeschreven extensie', () => {
    expect(extensionOf('Rapport.PDF')).toBe('pdf');
    expect(extensionOf('map/notitie.md')).toBe('md');
  });

  it('geeft leeg terug zonder extensie, en voor een dotfile', () => {
    expect(extensionOf('leesmij')).toBe('');
    expect(extensionOf('.gitignore')).toBe('');
    expect(extensionOf(null)).toBe('');
  });
});

describe('classifyUpload', () => {
  it('markdown gaat pass-through: opslaan, niets omzetten', () => {
    expect(classifyUpload('notitie.md', FORMATS)).toBe(PASSTHROUGH);
    expect(classifyUpload('notitie.markdown', FORMATS)).toBe(PASSTHROUGH);
  });

  it('formaten met converter kunnen zonder AI', () => {
    for (const name of ['rapport.docx', 'brief.odt', 'scan.pdf', 'pagina.html']) {
      expect(classifyUpload(name, FORMATS)).toBe(DETERMINISTIC);
    }
  });

  it('al het overige kan alleen met AI', () => {
    for (const name of ['brief.doc', 'aantekening.txt', 'plaat.png', 'deck.pptx', 'leesmij']) {
      expect(classifyUpload(name, FORMATS)).toBe(LLM_ONLY);
    }
  });

  it('zonder serverindeling is het antwoord onbekend, niet geraden', () => {
    expect(classifyUpload('rapport.docx', null)).toBe(UNKNOWN);
  });
});
