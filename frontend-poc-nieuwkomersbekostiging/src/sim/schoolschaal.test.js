import { describe, it, expect } from 'vitest';
import { schaalNaarEchteSchool } from './simulate.js';

/**
 * De simulatie werkt op drie schalen tegelijk: records, échte leerlingen en
 * échte scholen. Deze test legt vast hoe die zich verhouden, want het door
 * elkaar halen ervan zette eerder de drempel van vier (art. 34 lid 2) volledig
 * uit of juist volledig aan.
 */
const school = {
  school_id: 'P0001',
  sector: 'po',
  gewicht: 15.2, // echte scholen per gesimuleerde school
  leerlinggewicht: 121.3, // echte leerlingen per record
  omvang: 2, // nieuwkomers per jaar op de échte school (mediaan uit de YAML)
};

function record(asiel, overig = 0, tweedejaars = 0) {
  return {
    aantal_asielzoekers_peildatum: asiel,
    aantal_overige_vreemdelingen_peildatum: overig,
    aantal_asielzoekers_telling_1_februari: 0,
    aantal_tweedejaars_asielzoekers_peildatum: tweedejaars,
    aantal_nieuwkomers_peildatum: asiel + overig,
  };
}

describe('schaal van records naar echte scholen', () => {
  it('begrenst de telling op de jaaromvang van de school', () => {
    // Ruw zou 1 record x (121,3 / 15,2) = 8 geven, maar deze school heeft er
    // 2 per jaar. De drempel van vier gaat over die 2, niet over de 8.
    const rec = record(1);
    schaalNaarEchteSchool(rec, school);
    expect(rec.aantal_asielzoekers_peildatum).toBe(2);
  });

  it('laat een grote school wel boven de drempel uitkomen', () => {
    const groot = { ...school, omvang: 25 };
    const rec = record(2, 1);
    schaalNaarEchteSchool(rec, groot);
    const totaal = rec.aantal_asielzoekers_peildatum + rec.aantal_overige_vreemdelingen_peildatum;
    expect(totaal).toBeGreaterThanOrEqual(4);
  });

  it('laat de tellingen met gelijke gewichten ongemoeid', () => {
    const rec = record(3);
    const factor = schaalNaarEchteSchool(rec, { gewicht: 1, leerlinggewicht: 1, omvang: 10 });
    expect(factor).toBe(1);
    expect(rec.aantal_asielzoekers_peildatum).toBe(3);
  });

  it('geeft de gebruikte factor terug, zodat de bedragen dezelfde schaal krijgen', () => {
    const rec = record(1);
    const factor = schaalNaarEchteSchool(rec, school);
    // 2 leerlingen op deze school uit 1 record: factor 2.
    expect(factor).toBeCloseTo(2, 6);
  });

  it('valt terug op de ruwe factor als de omvang ontbreekt', () => {
    const zonder = { gewicht: 15.2, leerlinggewicht: 121.3 };
    const rec = record(1);
    const factor = schaalNaarEchteSchool(rec, zonder);
    expect(factor).toBeCloseTo(121.3 / 15.2, 6);
  });
});
