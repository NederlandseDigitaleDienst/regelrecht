import { describe, it, expect } from 'vitest';
import { budgetSignaal, draagkrachtmetingIsKeuze, verleggingEffect, heroOnderregel } from '../src/lib/burgerTekst.js';

/**
 * De zinnen over peiljaarverlegging moeten volgen uit de doorrekening, niet
 * uit een algemene aanname. Deze test legt de drie gevallen vast die de
 * simulatie over de zeven persona's oplevert.
 */
const zonder = { maandbedrag: 16424, einde: '2040', kwijt: 0 };

describe('verleggingEffect', () => {
  it('zegt dat het niets oplevert als er niets verandert', () => {
    const r = verleggingEffect(zonder, { ...zonder }, true);
    expect(r.soort).toBe('geen');
    expect(r.zin).toContain('levert je nu niets op');
  });

  it('wijst op de draagkrachtmeting als die ontbreekt', () => {
    const r = verleggingEffect(zonder, { ...zonder }, false);
    expect(r.soort).toBe('geen');
    // B1: het woord "draagkrachtmeting" staat er niet meer; de zin verwijst
    // naar wat het betekent, namelijk dat DUO naar je inkomen kijkt.
    expect(r.zin).toContain('naar je inkomen');
  });

  it('meldt de langere looptijd als de einddatum opschuift (Priya)', () => {
    const met = { maandbedrag: 5155, einde: '2041', kwijt: 2417587 };
    const r = verleggingEffect(zonder, met, true);
    expect(r.soort).toBe('langer');
    expect(r.zin).toContain('2041');
    expect(r.zin).toContain('2040');
    // De geruststelling hoort erbij: in totaal betaal je niet meer.
    expect(r.zin).toContain('niet meer');
  });

  it('meldt de hogere kwijtschelding bij gelijke einddatum (Yusuf)', () => {
    const basis = { maandbedrag: 2755, einde: '2061', kwijt: 7248388 };
    const met = { maandbedrag: 0, einde: '2061', kwijt: 9034079 };
    const r = verleggingEffect(basis, met, true);
    expect(r.soort).toBe('winst');
    expect(r.zin).toContain('even lang bezig');
    // B1: "kwijtgescholden" is vervangen door wat het voor je betekent.
    expect(r.zin).toContain('niet te betalen');
  });

  it('geeft niets terug zonder uitkomsten', () => {
    expect(verleggingEffect(null, null, true).soort).toBe('geen');
    expect(verleggingEffect(null, null, true).zin).toBe('');
  });
});

/**
 * De draagkrachtmeting is alleen een keuze onder hoofdstuk 10a (SF15-oud,
 * artikel 10a.7); onder hoofdstuk 6 stelt DUO hem ambtshalve vast. Het
 * verschil zit in draagkrachtmeting_van_toepassing zoals de engine hem
 * meldt bij een berekening zónder aanvraag:
 *
 *   false -> alleen op aanvraag, dus een keuze voor de burger
 *   true  -> ambtshalve, dus geen schakelaar tonen
 *
 * Gemeten over de zeven persona's met het inkomen gehalveerd:
 *   Priya, Wouter, Kwame, Mariska (SF15-oud): false
 *   Aisha, Yusuf, Els (hoofdstuk 6):      true
 *
 * Deze test legt de richting van die vlag vast, want hem omdraaien geeft een
 * schakelaar bij iedereen (of bij niemand) zonder dat een bouwtest faalt.
 */
describe('draagkrachtmeting als keuze', () => {
  // De doorrekening zonder aanvraag, zoals InkomenWatAls hem maakt.
  const isKeuze = (vanToepassing) =>
    draagkrachtmetingIsKeuze({ timeline: [{ draagkrachtmeting_van_toepassing: vanToepassing }] });

  it('is een keuze als de meting zonder aanvraag niet van toepassing is', () => {
    expect(isKeuze(false)).toBe(true);
  });

  it('is geen keuze als DUO hem ambtshalve vaststelt', () => {
    expect(isKeuze(true)).toBe(false);
  });

  it('toont niets bij een onbekende uitkomst', () => {
    expect(isKeuze(null)).toBe(false);
    expect(isKeuze(undefined)).toBe(false);
    expect(draagkrachtmetingIsKeuze(null)).toBe(false);
    expect(draagkrachtmetingIsKeuze({ timeline: [] })).toBe(false);
  });
});

/**
 * Bij een debiteur die niets betaalt (inkomen onder de drempel) klopt "in
 * 2041 ben je klaar" niet: je bent niet klaar, je termijn loopt af terwijl
 * de schuld met rente doorgroeit. Gemeten bij Aisha: 22.000 schuld, 0
 * betaald, 9.007 rente, 31.007 kwijtgescholden.
 */
describe('heroOnderregel', () => {
  const basis = { startSchuld: 2200000, eindejaar: 2041, levenslang: false };

  it('zegt dat de termijn stopt als je niets betaalt', () => {
    const zin = heroOnderregel({ ...basis, totaalBetaald: 0, kwijtgescholden: 3100702 }, {});
    expect(zin).toContain('te laag om te betalen');
    expect(zin).toContain('stopt je termijn');
    expect(zin).not.toContain('ben je klaar');
  });

  it('zegt wel "klaar" als je zelf afbetaalt', () => {
    const zin = heroOnderregel({ ...basis, totaalBetaald: 1500000, kwijtgescholden: 800000 }, {});
    expect(zin).toContain('ben je klaar');
  });

  it('noemt geen kwijtschelding bij afrondingsruis', () => {
    const zin = heroOnderregel({ ...basis, totaalBetaald: 2500000, kwijtgescholden: 5 }, {});
    expect(zin).toContain('ben je klaar');
    expect(zin).not.toContain('hoef je');
  });
});

/**
 * De waarschuwing over het budget maakt onderscheid tussen twee gevallen die
 * de simulatie allebei als betalingsprobleem markeert:
 *
 *   capaciteit 0  -> het inkomen dekt de vaste lasten volgens het
 *                    Nibud-referentiebudget niet; élk bedrag knelt, ook een
 *                    paar euro. "Kijk of je minder kunt betalen" helpt dan
 *                    niet en klinkt bij 8,73 euro ongeloofwaardig.
 *   capaciteit > 0 -> er is wel ruimte, maar minder dan het maandbedrag.
 *
 * Gemeten: Kwame heeft capaciteit 0 en betaalt mét draagkrachtmeting 8,73;
 * daar stond eerder de generieke tekst.
 */
describe('waarschuwing over het budget', () => {
  const kwame = { betalingsprobleem: true, maandbedrag: 873 };

  it('meldt bij capaciteit nul dat het inkomen tekortschiet', () => {
    for (const afloscapaciteit of [0, null]) {
      const s = budgetSignaal({ ...kwame, afloscapaciteit });
      expect(s.variant).toBe('warning');
      expect(s.tekst).toContain('Je inkomen is te laag');
      expect(s.tekst).not.toContain('minder kunt betalen');
    }
  });

  it('meldt bij ruimte dat het bedrag erboven ligt', () => {
    const s = budgetSignaal({ ...kwame, afloscapaciteit: 29912 });
    expect(s.tekst).toContain('Dit maandbedrag ligt boven');
    expect(s.tekst).toContain('minder kunt betalen');
  });

  it('zegt niets zonder betalingsprobleem', () => {
    expect(budgetSignaal({ betalingsprobleem: false, afloscapaciteit: 0, maandbedrag: 873 })).toBeNull();
  });
});
