import { describe, expect, it } from 'vitest';
import { makeTranslator, matchesTranslated, parallelPairs, sameNumbers, textMatchesTranslated } from './translateText';

const tr = makeTranslator({
  dicts: {
    source: { tab: 'Regelwerken', count: 'Regelwerken, {n}', open: 'Open {law}', ok: 'Voldoet aan de voorwaarden' },
    local: { tab: 'Ruleworks', count: 'Ruleworks, {n}', open: 'Open {law}', ok: 'Meets the conditions' },
  },
  pairs: [['Huurtoeslag', 'Housing benefit']],
  humanize: (name) => ({ box1_inkomen: 'Box 1 income' } as Record<string, string>)[name] ?? null,
});

describe('a recorded text in another language', () => {
  it('goes through the key it came from', () => {
    expect(tr('Regelwerken')).toEqual(['Regelwerken', 'Ruleworks']);
    expect(tr('Huurtoeslag')).toContain('Housing benefit');
  });

  it('fills a template with what the recording had in its place', () => {
    expect(tr('Regelwerken, 79')).toContain('Ruleworks, 79');
    expect(tr('Open zvw')).toContain('Open zvw');
  });

  it('takes a readable field name back to its identifier', () => {
    expect(tr('Box1 inkomen')).toContain('Box 1 income');
  });

  it('keeps a name that has no translation, which reads the same everywhere', () => {
    expect(tr('Merijn')).toEqual(['Merijn']);
  });

  it('follows a renamed translation without anything stored about the old one', () => {
    const renamed = makeTranslator({ dicts: { source: { tab: 'Regelwerken' }, local: { tab: 'Rule sets' } } });
    expect(matchesTranslated('Rule sets', 'Regelwerken', renamed)).toBe(true);
    expect(matchesTranslated('Ruleworks', 'Regelwerken', renamed)).toBe(false);
  });
});

describe('text on screen compared with the recording', () => {
  it('reads an amount the same in either way of writing it', () => {
    expect(sameNumbers('Premie € 4.112,00', 'Premie €4,112.00')).toBe(true);
    expect(sameNumbers('Premie € 4.112,00', 'Premie €4,113.00')).toBe(false);
    expect(sameNumbers('€ 100', '€ 1.000')).toBe(false);
    expect(sameNumbers('€ 1.000', '€1,000')).toBe(true);
    expect(sameNumbers('1 januari', '10 januari')).toBe(false);
    expect(sameNumbers('1,8%', '18%')).toBe(false);
    expect(sameNumbers('1,8%', '1.8%')).toBe(true);
  });

  it('takes a card back piece by piece, since its whole has no translation', () => {
    expect(textMatchesTranslated(['Housing benefit', 'Meets the conditions', '18%'], 'HuurtoeslagVoldoet aan de voorwaarden18%', tr)).toBe(true);
    expect(textMatchesTranslated(['Housing benefit', 'Meets the conditions', '19%'], 'HuurtoeslagVoldoet aan de voorwaarden18%', tr)).toBe(false);
  });
});

describe('pairs from a document and its translation', () => {
  it('pairs every string that differs, at the same place', () => {
    const nl = { slides: [{ title: 'RegelRecht', subtitle: 'van wet naar werking' }], name: 'Merijn' };
    const en = { slides: [{ title: 'RegelRecht', subtitle: 'from law to effect' }], name: 'Merijn' };
    expect(parallelPairs(nl, en)).toEqual([['van wet naar werking', 'from law to effect']]);
  });
});

describe('text put together in code', () => {
  it('matches part by part when the whole has no translation', () => {
    const t = makeTranslator({ humanize: (name) => ({ winst_uit_onderneming: 'Business profit' } as Record<string, string>)[name] ?? null });
    expect(matchesTranslated('Business profit: €25,000.00 → **€0.00**', 'Winst uit onderneming: € 25.000,00 → **€ 0,00**', t)).toBe(true);
    expect(matchesTranslated('Business profit: €25,000.00 → **€1.00**', 'Winst uit onderneming: € 25.000,00 → **€ 0,00**', t)).toBe(false);
  });

  it('knows a Gherkin step by its templates, arguments moved around', () => {
    const t = makeTranslator({ templates: [['**{1}** wordt uitgevoerd voor **{0}**', 'I evaluate **{0}** of **{1}**']] });
    expect(t('**zorgtoeslagwet** wordt uitgevoerd voor **hoogte_toeslag**')).toContain('I evaluate **hoogte_toeslag** of **zorgtoeslagwet**');
  });
});
