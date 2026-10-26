import { describe, expect, it } from 'vitest';
import { declaredClaim } from './declarations';

describe('declaredClaim', () => {
  const profile = { declarations: { 'wet_op_de_accijns/accijnsplicht_alcohol': { type_product: 'bier', hoeveelheid_hectoliter: 24, is_kleine_brouwerij: false } } };

  it('geeft wat het profiel al opgaf als goedgekeurde eigen opgave', () => {
    expect(declaredClaim(profile, 'wet_op_de_accijns/accijnsplicht_alcohol', 'type_product')).toMatchObject({
      lawId: 'wet_op_de_accijns/accijnsplicht_alcohol',
      input: 'type_product',
      newValue: 'bier',
      status: 'APPROVED',
      selfDeclared: true,
    });
  });

  it('houdt false en 0 als antwoord, en geeft niets voor wat niet is opgegeven', () => {
    expect(declaredClaim(profile, 'wet_op_de_accijns/accijnsplicht_alcohol', 'is_kleine_brouwerij')?.newValue).toBe(false);
    expect(declaredClaim(profile, 'wet_op_de_accijns/accijnsplicht_alcohol', 'alcoholpercentage')).toBeNull();
    expect(declaredClaim(profile, 'zorgtoeslagwet', 'type_product')).toBeNull();
    expect(declaredClaim(null, 'zorgtoeslagwet', 'type_product')).toBeNull();
  });
});
