Feature: A decision on a submission recorded in the chronicle
  The construct "decision_on_submission": the cell records an application,
  then decides on it, and the decision gives what the act's article does.
  The example caller of the reusable BDD workflow runs this set through the
  cell's runner (BDD_CONSTRUCT_SETS) as a construct set of an outside corpus.
  It loads ../regulation and nothing else: test_extern_vergunningwet, which
  is fictional.

  Scenario: A decision on a recorded application grants the permit
    Given the calculation date is "2026-03-03"
    When the cell records submission "aanvraag" under article "1" of "test_extern_vergunningwet" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-20"
    When the cell decides "besluit" on submission "aanvraag"
    Then the execution succeeds
    And output "permit_granted" is true

  Scenario: A decision on an application that does not meet the conditions refuses it
    Given the calculation date is "2026-03-03"
    When the cell records submission "aanvraag" under article "1" of "test_extern_vergunningwet" with:
      | meets_conditions | false |
    Given the calculation date is "2026-03-20"
    When the cell decides "besluit" on submission "aanvraag"
    Then the execution succeeds
    And output "permit_granted" is false
