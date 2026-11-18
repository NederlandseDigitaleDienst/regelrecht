/**
 * De oordelen die het beslissende artikel zelf aan het bestuursorgaan vraagt,
 * zoals bij Awb 7:11 of de indiener belanghebbende is. Welke dat zijn, zegt
 * het formulier van dat besluit (`vormen`), dus de wet; dit bestand noemt er
 * geen bij naam.
 */

/**
 * Wat het bestuursorgaan over `velden` oordeelt, met de waarden van het
 * formulier. Een schakelaar die uit staat, zegt nee, zoals bij een besluit:
 * een te laat bezwaar wordt dan niet stil verschoonbaar.
 */
export const eigenOordelen = (velden, waarden) =>
  Object.fromEntries(velden.map((v) => [v.naam, waarden[v.naam] ?? false]));

/**
 * Wat het besluit `besluit` over `velden` vastlegde, om een nieuw besluit op
 * dezelfde indiening mee te beginnen. Het nieuwe besluit legt zijn eigen
 * oordelen vast: de bezoeker ziet ze en kan ze wijzigen, de cel vult niets
 * zelf in.
 */
export const eerdereOordelen = (besluit, velden) =>
  Object.fromEntries(
    velden
      .filter((v) => besluit?.fields && v.naam in besluit.fields)
      .map((v) => [v.naam, besluit.fields[v.naam]]),
  );
