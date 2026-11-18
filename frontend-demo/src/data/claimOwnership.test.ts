/**
 * De regel uit `data/claimOwnership.ts`, die `demoStore.resubmitCase` draait.
 *
 * Dezelfde functie en geen nagebouwde kopie: een kopie bewijst alleen dat de
 * regel klopt zoals hij hier staat, niet dat de winkel hem nog gebruikt. Juist
 * de fout die dit moet tegenhouden — het `tileLawId`-filter dat wegvalt — zou
 * een kopie niet zien.
 */
import { describe, expect, it } from 'vitest';
import { assignClaimOwnership as assignOwnership } from './claimOwnership';

interface TestClaim {
  id: string;
  caseId: string | null;
  tileLawId: string;
}

const claim = (over: Partial<TestClaim>): TestClaim => ({ id: 'c1', caseId: null, tileLawId: 'huurtoeslag', ...over });

describe('eigenaarschap van een correctie', () => {
  it('hangt een correctie aan de zaak van dezelfde regeling', () => {
    const claims = [claim({ id: 'c1', tileLawId: 'huurtoeslag' })];
    assignOwnership(claims, { id: 'zaak-1', lawId: 'huurtoeslag' });
    expect(claims[0]?.caseId).toBe('zaak-1');
  });

  it('laat een correctie van een andere regeling met rust', () => {
    // Deze correctie telt wél mee voor de vraag of er beoordeeld moet worden,
    // maar hoort in het dossier van de zorgtoeslag en niet hier.
    const claims = [claim({ id: 'c2', tileLawId: 'zorgtoeslagwet' })];
    assignOwnership(claims, { id: 'zaak-1', lawId: 'huurtoeslag' });
    expect(claims[0]?.caseId).toBeNull();
  });

  it('laat een correctie die al bij een zaak hoort staan waar hij staat', () => {
    const claims = [claim({ id: 'c3', caseId: 'zaak-eerder' })];
    assignOwnership(claims, { id: 'zaak-1', lawId: 'huurtoeslag' });
    expect(claims[0]?.caseId).toBe('zaak-eerder');
  });

  it('houdt twee zaken uit elkaar', () => {
    // Het geval waar het misging: één persoon, twee lopende regelingen. Wie het
    // eerst opnieuw indient, mocht niet de correcties van de ander opeisen.
    const claims = [
      claim({ id: 'huur', tileLawId: 'huurtoeslag' }),
      claim({ id: 'zorg', tileLawId: 'zorgtoeslagwet' }),
    ];
    assignOwnership(claims, { id: 'zaak-huur', lawId: 'huurtoeslag' });
    assignOwnership(claims, { id: 'zaak-zorg', lawId: 'zorgtoeslagwet' });
    expect(claims.map((c) => c.caseId)).toEqual(['zaak-huur', 'zaak-zorg']);
  });
});
