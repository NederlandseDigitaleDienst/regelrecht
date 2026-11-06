Feature: An answer is about one day and given as known on another
  As a cell that answers questions about the past and the future
  I want to say on which day an answer must hold and up to which day I read my chronicle
  So that a question about last month can be answered with what I knew then or with what I know now

  # The answer is about the calculation date (`$referencedate` in the
  # reduction). "As known on" names the day up to which the cell's records are
  # played (schema v0.8.15). The reduction test_reductie_verleningen reads
  # the grants for a period in force (fictional article 9 of
  # test_chronicle_effects), each with the day it starts and the day it ends,
  # and answers whether one of them holds on the calculation date. Article 4 is a
  # fictional withdrawal that undoes the grant from the start.

  Background:
    Given parameter "aanvrager" is "burger_x"

  Scenario: A decision today grants a card from next week
    Given the calculation date is "2025-03-01"
    When the cell decides "verlening" under article "9" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | ingangsdatum | 2025-03-08 |
      | einddatum    | 2026-03-08 |
    Given the calculation date is "2025-03-03"
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle as known on "2025-03-01"
    Then output "heeft_geldige_kaart_op_dag" is false
    Given the calculation date is "2025-03-10"
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle as known on "2025-03-01"
    Then output "heeft_geldige_kaart_op_dag" is true
    And the records in force are "verlening"

  Scenario: A question about last month, as known then and as known now
    # The grant is withdrawn on 1 March, from the start. Asked about 15
    # February, the cell knew of no withdrawal on that day; it knows of one
    # now.
    Given the calculation date is "2025-02-01"
    When the cell decides "verlening" under article "9" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | ingangsdatum | 2025-02-01 |
      | einddatum    | 2026-02-01 |
    Given the calculation date is "2025-03-01"
    When the cell decides "intrekking" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    Given the calculation date is "2025-02-15"
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle as known on "2025-02-15"
    Then output "heeft_geldige_kaart_op_dag" is true
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle as known on "2025-03-05"
    Then output "heeft_geldige_kaart_op_dag" is false
    And the projection trace is:
      | record     | event   | detail                                                                                                         |
      | verlening  | applied | - |
      | intrekking | rewound | undoes verlening                                                                                               |
      | verlening  | skipped | undone by intrekking                                                                                           |
      | intrekking | applied | -                                                                                                              |
    When the cell answers "heeft_geldige_kaart_op_dag" as known on "2025-02-15"
    Then the answer of the cell is true
    When the cell answers "heeft_geldige_kaart_op_dag" as known on "2025-03-05"
    Then the answer of the cell is false
