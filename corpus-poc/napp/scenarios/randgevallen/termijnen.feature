Feature: Termijnverlenging volgens de Algemene termijnenwet
  Een wettelijke termijn die op een zaterdag of zondag eindigt, wordt
  verlengd tot en met de eerstvolgende werkdag (Algemene termijnenwet,
  artikel 1). De feestdagentoets is in deze versie niet uitvoerbaar en
  is gemarkeerd als untranslatable.

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Een termijn die op zaterdag eindigt schuift naar maandag
    Given an application with the following data:
      | einddatum | 2026-06-13 |
    When the termijnverlenging is calculated
    Then the verlengde einddatum is "2026-06-15"

  Scenario: Een termijn die op zondag eindigt schuift naar maandag
    Given an application with the following data:
      | einddatum | 2026-06-14 |
    When the termijnverlenging is calculated
    Then the verlengde einddatum is "2026-06-15"

  Scenario: Een termijn die op een werkdag eindigt verandert niet
    Given an application with the following data:
      | einddatum | 2026-06-12 |
    When the termijnverlenging is calculated
    Then the verlengde einddatum is "2026-06-12"

  Scenario: Bezwaartermijn na bekendmaking eindigt nooit in het weekend
    Given an application with the following data:
      | bekendmaking_datum | 2026-05-02 |
    When the bezwaartermijn is calculated including the termijnenwet
    Then the verlengde einddatum is "2026-06-15"

  Scenario: Bezwaartermijn die op een werkdag eindigt blijft gelijk
    Given an application with the following data:
      | bekendmaking_datum | 2026-06-09 |
    When the bezwaartermijn is calculated including the termijnenwet
    Then the verlengde einddatum is "2026-07-21"

  Scenario: Zonder wettelijke termijn en zonder mededeling is de redelijke termijn uiterlijk acht weken na ontvangst verstreken
    Given an application with the following data:
      | aanvraag_datum | 2026-06-09 |
    When the beslistermijn is calculated including the termijnenwet
    Then the verlengde einddatum is "2026-08-04"

  Scenario: De buitengrens van de redelijke termijn die in het weekend eindigt schuift naar maandag
    Given an application with the following data:
      | aanvraag_datum | 2026-06-13 |
    When the beslistermijn is calculated including the termijnenwet
    Then the verlengde einddatum is "2026-08-10"

  Scenario: Wpp-termijnen voor subsidiejaar 2027 (lex specialis t.o.v. AWB 4:13)
    Given an application with the following data:
      | subsidiejaar_startdatum | 2027-01-01 |
    When the verleningstermijnen are calculated
    Then the aanvraagtermijn ends on "2026-11-01"
    And the beslistermijn ends on "2026-12-31"
    And the voorschotpercentage is "80"
