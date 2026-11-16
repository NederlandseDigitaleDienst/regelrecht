Feature: AWB-procedure rondom het subsidiebesluit
  Het subsidiebesluit is een beschikking in de zin van de Algemene wet
  bestuursrecht. De AWB-regels haken automatisch aan op het besluit:
  motiveringsplicht (3:46) en bezwaartermijn (6:7) gelden zowel bij
  toekenning als bij afwijzing.

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Toekenning is een beschikking met motiveringsplicht en bezwaartermijn
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And motivering is vereist
    And the bezwaartermijn is "6" weken
