/**
 * Welke correcties bij welke zaak horen.
 *
 * Twee vragen die op elkaar lijken maar niet hetzelfde zijn:
 *
 * - *Moet er een mens naar kijken?* Daarvoor telt élke openstaande correctie
 *   van deze persoon mee, ook een die bij een andere regeling is opgegeven. Een
 *   inkomenswijziging bij de huurtoeslag verandert immers ook de zorgtoeslag.
 * - *Bij welke zaak hoort deze correctie?* Daarvoor telt alleen de regeling
 *   waarop de correctie is ingediend.
 *
 * Die tweede vraag met het antwoord van de eerste beantwoorden, hangt een
 * correctie voorgoed aan de verkeerde zaak: het veld wordt maar één keer gezet,
 * dus de zaak waar hij wél bij hoort krijgt hem daarna niet meer.
 *
 * Staat hier en niet in de store, zodat de regel die getest wordt dezelfde
 * regel is die draait.
 */

/**
 * Hang de correcties die bij deze regeling horen aan deze zaak.
 *
 * Muteert de correcties die eraan toe zijn, en laat de rest staan: een
 * correctie die al bij een zaak hoort blijft daar, en een correctie van een
 * andere regeling wordt niet opgeëist.
 */
/** A claim (correction), as far as this module reads and sets it. */
export interface OwnableClaim {
  caseId?: string | null;
  tileLawId?: string | null;
}

/** A case (zaak), as far as this module reads it. */
interface OwningCase {
  id: string;
  lawId: string;
}

export function assignClaimOwnership<C extends OwnableClaim>(claims: C[], caseRecord: OwningCase): C[] {
  for (const claim of claims) {
    if (!claim.caseId && claim.tileLawId === caseRecord.lawId) claim.caseId = caseRecord.id;
  }
  return claims;
}
