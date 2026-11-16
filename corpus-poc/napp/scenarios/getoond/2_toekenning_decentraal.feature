Feature: Subsidie voor decentrale politieke partijen
  Als decentrale politieke partij met ten minste een zetel in de
  gemeenteraad, provinciale staten, de eilandsraad of het algemeen bestuur
  van een waterschap wil ik subsidie ontvangen, berekend als aantal zetels
  maal een bedrag per zetel uit het Besluit subsidiëring decentrale
  politieke partijen (AMvB op grond van artikel 26; voor gemeenteraadszetels
  afhankelijk van het inwoneraantal). Bij verlening hoort een voorschot
  van 80% (artikel 17).

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Lokale partij met 5 zetels in een middelgrote gemeente
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 5          |
      | inwoneraantal_gemeente           | 50000      |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "450000" eurocent
    And a betaalopdracht of "360000" eurocent is required
