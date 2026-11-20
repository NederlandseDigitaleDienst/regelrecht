@tier:core
Feature: Law end dates (valid_to) — RFC-019
  A law version can carry a valid_to: the last calendar day on which it is in
  force (inclusive). Version selection drops the law after that day and does
  not fall through to an older version; a reference to an ended law fails with
  the data fact (when and until when the law was in force) instead of silently
  computing on no-longer-valid law.

  Scenario: A law resolves while it is in force
    Given the calculation date is "2024-06-01"
    When I evaluate "normbedrag" of "test_einddatum"
    Then the execution succeeds
    Then output "normbedrag" equals 500

  Scenario: A law still resolves on its last day in force (inclusive bound)
    Given the calculation date is "2024-12-31"
    When I evaluate "normbedrag" of "test_einddatum"
    Then the execution succeeds
    Then output "normbedrag" equals 500

  Scenario: A law no longer resolves after its end date
    Given the calculation date is "2025-06-01"
    When I evaluate "normbedrag" of "test_einddatum"
    Then the execution fails with "No version of law 'test_einddatum' in force on 2025-06-01; last in force until 2024-12-31"

  Scenario: A cross-law reference to an ended law states the data fact
    Given the calculation date is "2025-06-01"
    When I evaluate "afgeleid_bedrag" of "test_einddatum_afnemer"
    Then the execution fails with "No version of law 'test_einddatum' in force on 2025-06-01"

  Scenario: The dependent law works while the referenced law is in force
    Given the calculation date is "2024-06-01"
    When I evaluate "afgeleid_bedrag" of "test_einddatum_afnemer"
    Then the execution succeeds
    Then output "afgeleid_bedrag" equals 500
