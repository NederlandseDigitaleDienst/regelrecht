/**
 * Het formulier uit ScenarioFormulier als populatierecord, precies het schema
 * uit personas.yaml: euro's naar eurocent, jaartal naar een datum, procent
 * naar een ratio. Een veld dat hier ontbreekt struikelt de simulatie, of
 * ontbreekt stilletjes in de doorrekening.
 *
 * @param {object} vorm - de waarden van het formulier, in hele euro's en procenten
 * @param {string} bsn - het bsn van het scenario (bestaand of nieuw)
 */
export function scenarioRecord(vorm, bsn) {
  const heeftPartner = vorm.huishoudtype.startsWith('paar');
  return {
    bsn,
    naam: vorm.naam.trim() || 'Naamloos scenario',
    geboortejaar: vorm.geboortejaar,
    omschrijving: 'Zelf toegevoegd scenario.',
    huishoudtype: vorm.huishoudtype,
    heeft_partner: heeftPartner,
    partnerinkomen: heeftPartner ? Math.round(vorm.partnerinkomen * 100) : 0,
    inkomen: Math.round(vorm.inkomen * 100),
    inkomensgroei: Math.round(vorm.groei * 10) / 1000,
    schuld: Math.round(vorm.schuld * 100),
    eerste_studiefinanciering: `${vorm.startjaar}-09-01`,
    onderwijssoort: vorm.onderwijssoort,
    is_levenlanglerenkrediet: vorm.lllk,
    // Welke keuzes zinvol zijn volgt uit de wet; de burgerview vult die lijst
    // zelf aan op basis van wat er op dat moment mag.
    keuzemomenten: [],
    eigen: true,
  };
}
