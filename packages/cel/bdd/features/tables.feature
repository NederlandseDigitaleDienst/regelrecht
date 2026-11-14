Feature: An array with columns in the state of an aggregate
  As a cell that records facts holding a table
  I want an aggregate that keeps that table in its state
  So that a read model can iterate over the rows of each aggregate

  # Exercises an array with columns in `projection.state` (schema v0.8.20)
  # against the synthetic law test_uitslagen. Article 1 records the result of
  # a polling station with an array of lists and their votes; article 2
  # records a correction that refers to a result, with a new array. The
  # reduction test_reductie_uitslagen (fixtures/cells/test_cel) plays one
  # aggregate per result, whose state holds the array, and its read model sums
  # the votes of one list over the arrays of every aggregate with a nested
  # FOREACH. A second read model builds a row with two columns per aggregate
  # (schema v0.8.22) and reads that table back.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanduiding" is "L1"
    When the cell records fact "uitslag" as "uitslag_noord" under article "1" of "test_uitslagen" with:
      | stembureau | noord                                                                                       |
      | lijsten    | [{"aanduiding": "L1", "aantal_stemmen": 120}, {"aanduiding": "L2", "aantal_stemmen": 80}] |
    And the cell records fact "uitslag" as "uitslag_zuid" under article "1" of "test_uitslagen" with:
      | stembureau | zuid                                          |
      | lijsten    | [{"aanduiding": "L1", "aantal_stemmen": 30}] |

  Scenario: A read model sums a column over the array of each aggregate
    When I evaluate "stemmen_op_lijst" of "test_reductie_uitslagen" over the chronicle
    Then output "stemmen_op_lijst" equals 150

  Scenario: A correction replaces the array of the result it refers to
    When the cell records fact "correctie" as "correctie_noord" about "uitslag_noord" under article "2" of "test_uitslagen" with:
      | lijsten | [{"aanduiding": "L1", "aantal_stemmen": 102}, {"aanduiding": "L2", "aantal_stemmen": 98}] |
    And I evaluate "stemmen_op_lijst" of "test_reductie_uitslagen" over the chronicle
    Then output "stemmen_op_lijst" equals 132

  Scenario: A read model builds a row per aggregate and reads it as a table
    Given parameter "stembureau" is "noord"
    When I evaluate "aantal_rijen" of "test_reductie_uitslagen" over the chronicle
    Then output "aantal_rijen" equals 2
    When I evaluate "stemmen_op_stembureau" of "test_reductie_uitslagen" over the chronicle
    Then output "stemmen_op_stembureau" equals 120

  Scenario: A correction changes the row of its aggregate in the table
    Given parameter "stembureau" is "noord"
    When the cell records fact "correctie" as "correctie_noord" about "uitslag_noord" under article "2" of "test_uitslagen" with:
      | lijsten | [{"aanduiding": "L1", "aantal_stemmen": 102}, {"aanduiding": "L2", "aantal_stemmen": 98}] |
    And I evaluate "stemmen_op_stembureau" of "test_reductie_uitslagen" over the chronicle
    Then output "stemmen_op_stembureau" equals 102
