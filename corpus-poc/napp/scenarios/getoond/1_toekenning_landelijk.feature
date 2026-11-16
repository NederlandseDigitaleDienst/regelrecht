Feature: Subsidie voor landelijke politieke partijen
  Als landelijke politieke partij met ten minste een kamerzetel en duizend
  betalende leden wil ik subsidie ontvangen van de Nederlandse autoriteit
  politieke partijen. De subsidie bestaat uit een basisbedrag, een bedrag
  per kamerzetel en een aandeel in het ledenbudget naar rato van de
  opgegeven ledentallen van alle ontvangende partijen (artikel 14,
  onderdeel a). Bij verlening hoort een voorschot van 80% (artikel 17).

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Middelgrote landelijke partij met 10 zetels en 5000 van 100000 leden
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | totaal_aantal_betalende_leden    | 100000    |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "106191185" eurocent
    And a betaalopdracht of "84952948" eurocent is required
