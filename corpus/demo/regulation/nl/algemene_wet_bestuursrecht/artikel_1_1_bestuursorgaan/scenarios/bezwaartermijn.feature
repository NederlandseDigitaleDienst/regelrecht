Feature: AWB artikel 6:7 en 6:8 - Bezwaartermijn
  Als belanghebbende bij een besluit
  Wil ik weten tot wanneer ik bezwaar kan maken
  Zodat ik op tijd ben

  # Artikel 6:8 telt vanaf de dag na de bekendmaking, en de termijn van 6:7 is
  # zes weken. De einddatum is zes weken na de bekendmaking: de startdag telt
  # mee. Een eerdere versie gaf hier de bekendmakingsdatum zelf terug.
  Scenario: De bezwaartermijn eindigt zes weken na de bekendmaking
    Given the calculation date is "2026-03-12"
    Given parameter "bekendmaking_datum" is "2026-03-12"
    When I evaluate outputs "termijn_startdatum, termijn_einddatum" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "termijn_startdatum" equals "2026-03-13"
    Then output "termijn_einddatum" equals "2026-04-23"
