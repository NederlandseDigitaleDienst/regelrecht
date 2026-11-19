Feature: A regelwerk of an outside corpus reads one from corpus/regulation
  The example caller of the reusable BDD workflow runs this as bucket A, with
  corpus/regulation loaded beside it. test_extern_toeslag is fictional; it
  doubles the normbedrag of test_einddatum, a synthetic law of the central
  corpus that is in force throughout 2024.

  Scenario: The toeslag is twice the normbedrag of the central corpus
    Given the calculation date is "2024-06-01"
    When I evaluate "toeslag" of "test_extern_toeslag"
    Then the execution succeeds
    Then output "toeslag" equals 1000
