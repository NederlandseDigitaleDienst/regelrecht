import { describe, it, expect } from 'vitest';
import { uitvoeringslastVan } from './metrics.js';

// Een klein model met precies de twee soorten partij die ertoe doen: één met
// een tarief (DUO, telt in euro's) en één zonder (de debiteur, telt in uren).
const MODEL = {
  tarieven: { duo_medewerker: 7300 }, // eurocent per uur
  handelingen: [
    { id: 'verwerken', partij: 'duo', aanleiding: 'per_draagkrachtmeting', minuten: 30, tarief: 'duo_medewerker' },
    { id: 'aanvragen', partij: 'debiteur', aanleiding: 'per_draagkrachtmeting', minuten: 90 },
  ],
};

describe('uitvoeringslastVan', () => {
  it('rekent minuten x aantal om naar uren en euro\'s', () => {
    const r = uitvoeringslastVan(MODEL, { per_draagkrachtmeting: 100 });
    // 100 x 30 minuten = 50 uur, x 7300 eurocent = 365.000 eurocent.
    expect(r.perPartij.duo.uren).toBe(50);
    expect(r.perPartij.duo.kosten).toBe(365000);
  });

  it('drukt de tijd van de debiteur niet in geld uit', () => {
    const r = uitvoeringslastVan(MODEL, { per_draagkrachtmeting: 100 });
    const aanvragen = r.perHandeling.find((h) => h.id === 'aanvragen');
    expect(aanvragen.kosten).toBeNull();
    expect(r.perPartij.debiteur.inGeld).toBe(false);
  });

  it('telt de uren van de debiteur niet op bij het eurototaal', () => {
    const r = uitvoeringslastVan(MODEL, { per_draagkrachtmeting: 100 });
    // Alleen DUO zit in kostenTotaal; de 150 uur van de debiteur staat ernaast.
    expect(r.kostenTotaal).toBe(365000);
    expect(r.urenBurger).toBe(150);
  });

  it('telt een aanleiding die niet voorkomt als nul, niet als ontbrekend', () => {
    const r = uitvoeringslastVan(MODEL, {});
    expect(r.kostenTotaal).toBe(0);
    expect(r.perHandeling.every((h) => h.aantal === 0)).toBe(true);
  });

  it('geeft null zonder model: onbekend is niet hetzelfde als nul', () => {
    expect(uitvoeringslastVan(null, { per_draagkrachtmeting: 100 })).toBeNull();
    expect(uitvoeringslastVan({ handelingen: [] }, {})).toBeNull();
  });
});
