import { describe, it, expect, beforeEach } from 'vitest';
import { usePopulatie } from './usePopulatie.js';
import { generatePopulation } from '../sim/population.js';
import { STUB_DISTRIBUTIONS } from '../test-utils/stubEngine.js';

const {
  basis, effectief, versie, hasOverrides, instroomTotaal,
  setInstroom, setCategorie, setScholen, setFractie, resetOverrides,
} = usePopulatie();

describe('populatie-aannames', () => {
  beforeEach(() => {
    basis.value = STUB_DISTRIBUTIONS;
    resetOverrides();
  });

  it('geeft zonder overrides de basis terug', () => {
    expect(hasOverrides.value).toBe(false);
    expect(effectief.value.instroom_per_jaar).toEqual(STUB_DISTRIBUTIONS.instroom_per_jaar);
    expect(effectief.value.categorie_verdeling).toEqual(STUB_DISTRIBUTIONS.categorie_verdeling);
  });

  it('past de instroom aan zonder de basis te raken', () => {
    const jaar = Number(Object.keys(STUB_DISTRIBUTIONS.instroom_per_jaar)[0]);
    const voor = instroomTotaal.value;
    setInstroom(jaar, 'po', 1000);

    expect(effectief.value.instroom_per_jaar[jaar].po).toBe(1000);
    expect(basis.value.instroom_per_jaar[jaar].po).not.toBe(1000);
    expect(instroomTotaal.value).not.toBe(voor);
    expect(hasOverrides.value).toBe(true);
  });

  it('bumpt de versie zodat de steekproef opnieuw getrokken wordt', () => {
    const voor = versie.value;
    setScholen('po', 500);
    expect(versie.value).toBeGreaterThan(voor);
  });

  it('levert een andere populatie op bij dezelfde seed', () => {
    const voor = generatePopulation(effectief.value, 400, 42);
    setCategorie('po', 'asielzoeker', 0.9);
    const na = generatePopulation(effectief.value, 400, 42);
    expect(na).not.toEqual(voor);
    expect(na).toHaveLength(400);
  });

  it('begrenst fracties tot 0 en 1 en herstelt naar de basis', () => {
    setFractie('werkelijk_schoolgaand_fractie', 1.4);
    expect(effectief.value.werkelijk_schoolgaand_fractie).toBe(1);
    setFractie('werkelijk_schoolgaand_fractie', -0.2);
    expect(effectief.value.werkelijk_schoolgaand_fractie).toBe(0);

    resetOverrides();
    expect(hasOverrides.value).toBe(false);
    expect(effectief.value.werkelijk_schoolgaand_fractie).toBe(
      STUB_DISTRIBUTIONS.werkelijk_schoolgaand_fractie,
    );
  });
});
