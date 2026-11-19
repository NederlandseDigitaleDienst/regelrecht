import { describe, it, expect } from 'vitest';
import { uitvoeringslastVan } from './metrics.js';

/**
 * Bijstellen gebeurt door het model dat aan `uitvoeringslastVan` wordt
 * meegegeven te wijzigen; de aantallen blijven staan. Deze test legt vast dat
 * die rekensom doet wat de knoppen beloven: minuten en tarieven werken door,
 * en de scheiding tussen euro's en uren blijft overeind.
 */
const MODEL = {
  tarieven: { duo_medewerker: 7300 },
  handelingen: [
    { id: 'verwerken', partij: 'duo', aanleiding: 'per_draagkrachtmeting', minuten: 30, tarief: 'duo_medewerker' },
    { id: 'aanvragen', partij: 'debiteur', aanleiding: 'per_draagkrachtmeting', minuten: 90 },
  ],
};

/** Zoals de composable het doet: overrides over de basis heen. */
function metBijstelling({ minuten = {}, tarieven = {} } = {}) {
  return {
    ...MODEL,
    tarieven: { ...MODEL.tarieven, ...tarieven },
    handelingen: MODEL.handelingen.map((h) => ({ ...h, minuten: minuten[h.id] ?? h.minuten })),
  };
}

const VOLUMES = { per_draagkrachtmeting: 100 };

describe('het uitvoeringslastmodel bijstellen', () => {
  it('halveert de kosten als de minuten halveren', () => {
    const voor = uitvoeringslastVan(MODEL, VOLUMES);
    const na = uitvoeringslastVan(metBijstelling({ minuten: { verwerken: 15 } }), VOLUMES);

    expect(na.kostenTotaal).toBe(voor.kostenTotaal / 2);
  });

  it('rekent een hoger tarief door in de euro\'s', () => {
    const voor = uitvoeringslastVan(MODEL, VOLUMES);
    const na = uitvoeringslastVan(metBijstelling({ tarieven: { duo_medewerker: 14600 } }), VOLUMES);

    expect(na.kostenTotaal).toBe(voor.kostenTotaal * 2);
  });

  it('laat de uren van de debiteur ongemoeid bij een tariefwijziging', () => {
    const voor = uitvoeringslastVan(MODEL, VOLUMES);
    const na = uitvoeringslastVan(metBijstelling({ tarieven: { duo_medewerker: 99999 } }), VOLUMES);

    // De debiteur heeft geen tarief; daar kan geen tariefknop iets aan doen.
    expect(na.urenBurger).toBe(voor.urenBurger);
  });

  it('stelt de uren van de debiteur bij via zijn eigen minuten', () => {
    const na = uitvoeringslastVan(metBijstelling({ minuten: { aanvragen: 45 } }), VOLUMES);

    // 100 × 45 minuten = 75 uur, en nog steeds geen euro's.
    expect(na.urenBurger).toBe(75);
    expect(na.perPartij.debiteur.inGeld).toBe(false);
  });

  it('raakt de aantallen niet: bijstellen is geen hersimulatie', () => {
    const na = uitvoeringslastVan(metBijstelling({ minuten: { verwerken: 1 } }), VOLUMES);

    expect(na.perHandeling.every((h) => h.aantal === 100)).toBe(true);
  });
});
