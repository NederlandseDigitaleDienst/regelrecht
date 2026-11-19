Feature: Awb termijnen
  The decision term of article 4:13 and the objection term of articles 6:7
  and 6:8, computed by the Awb itself. How a law of its own fills or
  overrides these terms is an engine construct, tested on a fictional
  general act in packages/engine/tests/procedure_lifecycle.rs.

  Background:
    Given the calculation date is "2026-03-12"

  Scenario: The objection term ends six weeks after the decision is made known
    Given parameter "bekendmaking_datum" is "2026-03-12"
    When I evaluate outputs "termijn_startdatum, termijn_einddatum" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "termijn_startdatum" equals "2026-03-13"
    Then output "termijn_einddatum" equals "2026-04-23"

  Scenario: Without a term set by statutory provision the decision is due eight weeks after receipt
    Given parameter "datum_ontvangst" is "2026-01-05"
    When I evaluate outputs "uiterste_beslisdatum, redelijke_beslistermijn_einddatum" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "uiterste_beslisdatum" equals "2026-03-02"
    Then output "redelijke_beslistermijn_einddatum" equals "2026-03-02"
