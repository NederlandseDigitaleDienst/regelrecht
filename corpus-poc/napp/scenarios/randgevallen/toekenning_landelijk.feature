Feature: Subsidie voor landelijke politieke partijen
  Als landelijke politieke partij met ten minste een kamerzetel en duizend
  betalende leden wil ik subsidie ontvangen van de Nederlandse autoriteit
  politieke partijen. De subsidie bestaat uit een basisbedrag, een bedrag
  per kamerzetel en een aandeel in het ledenbudget naar rato van de
  opgegeven ledentallen van alle ontvangende partijen (artikel 14,
  onderdeel a). Bij verlening hoort een voorschot van 80% (artikel 17).

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Kleine partij precies op de drempel van 1 zetel en 1000 leden
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 1         |
      | aantal_betalende_leden           | 1000      |
      | totaal_aantal_betalende_leden    | 100000    |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "32870917" eurocent

  Scenario: Grote partij met 30 zetels en 50000 van 500000 leden
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 30        |
      | aantal_betalende_leden           | 50000     |
      | totaal_aantal_betalende_leden    | 500000    |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "259037270" eurocent
    And a betaalopdracht of "207229816" eurocent is required

  Scenario: De enige ontvangende partij krijgt het volledige ledenbudget
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | totaal_aantal_betalende_leden    | 5000      |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "352622800" eurocent

  Scenario: Specificatie in vier delen met aangewezen neveninstellingen
    Given an application with the following data:
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | totaal_aantal_betalende_leden    | 100000    |
      | heeft_wetenschappelijk_instituut | true      |
      | heeft_jongerenorganisatie        | true      |
      | aantal_leden_jongerenorganisatie | 2000      |
      | heeft_instelling_buitenland      | true      |
    When the subsidiebedragen of artikel 14 are calculated
    Then the output "subsidie_partij" is "106191185" eurocent
    And the output "subsidie_wetenschappelijk_instituut" is "25803300" eurocent
    And the output "subsidie_jongerenorganisatie" is "4391100" eurocent
    And the output "subsidie_buitenland" is "3797000" eurocent
    And the output "subsidie_landelijk" is "140182585" eurocent

  Scenario: Besluit inclusief neveninstellingen met voorschot van 80%
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | totaal_aantal_betalende_leden    | 100000    |
      | heeft_wetenschappelijk_instituut | true      |
      | heeft_jongerenorganisatie        | true      |
      | aantal_leden_jongerenorganisatie | 2000      |
      | heeft_instelling_buitenland      | true      |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "140182585" eurocent
    And a betaalopdracht of "112146068" eurocent is required
