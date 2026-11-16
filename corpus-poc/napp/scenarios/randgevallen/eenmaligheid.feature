Feature: Eenmalige verstrekking per subsidiejaar (Wpp artikel 13)
  De subsidie wordt per onderdeel ten hoogste eenmaal per subsidiejaar
  verstrekt. Een lopende aanvraag of een eerdere toekenning blokkeert een
  nieuwe aanvraag; een eerdere afwijzing bewust niet.

  Scenario: Een vrij onderdeel is beschikbaar
    Given the calculation date is "2026-06-01"
    And the following facts:
      | onderdeel_in_behandeling   | false |
      | onderdeel_eerder_toegekend | false |
      | onderdeel_eerder_afgewezen | false |
    When the beschikbaarheid of artikel 13 is evaluated
    Then the output "onderdeel_beschikbaar" is true

  Scenario: Een lopende aanvraag blokkeert een nieuwe aanvraag
    Given the calculation date is "2026-06-01"
    And the following facts:
      | onderdeel_in_behandeling   | true  |
      | onderdeel_eerder_toegekend | false |
      | onderdeel_eerder_afgewezen | false |
    When the beschikbaarheid of artikel 13 is evaluated
    Then the output "onderdeel_beschikbaar" is false

  Scenario: Een eerdere toekenning blokkeert een nieuwe aanvraag
    Given the calculation date is "2026-06-01"
    And the following facts:
      | onderdeel_in_behandeling   | false |
      | onderdeel_eerder_toegekend | true  |
      | onderdeel_eerder_afgewezen | false |
    When the beschikbaarheid of artikel 13 is evaluated
    Then the output "onderdeel_beschikbaar" is false

  Scenario: Een eerdere afwijzing staat een nieuwe aanvraag niet in de weg
    Given the calculation date is "2026-06-01"
    And the following facts:
      | onderdeel_in_behandeling   | false |
      | onderdeel_eerder_toegekend | false |
      | onderdeel_eerder_afgewezen | true  |
    When the beschikbaarheid of artikel 13 is evaluated
    Then the output "onderdeel_beschikbaar" is true
