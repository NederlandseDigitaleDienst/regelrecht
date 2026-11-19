@tier:core
Feature: Data sources bound to one law
  A `source: {}` input is resolved outside the YAML. When the caller supplies
  that data for one law, the engine must consult it only while resolving that
  law's inputs. Otherwise a raw register column can shadow a same-named
  cross-law input of another law, and the afnemer silently gets the register
  value instead of the computed one.

  Driven against test_scoped_source_bron (doubles a register value) and
  test_scoped_source_afnemer (takes the doubled value cross-law, under the
  same input name).

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"

  Scenario: The bound law reads its register data
    Given the following "register" data with key "bsn" for law "test_scoped_source_bron":
      | bsn         | inkomen |
      | "999993653" | 500     |
    When I evaluate "inkomen" of "test_scoped_source_bron"
    Then the execution succeeds
    Then output "inkomen" equals 1000

  Scenario: A same-named input of another law is not shadowed
    Given the following "register" data with key "bsn" for law "test_scoped_source_bron":
      | bsn         | inkomen |
      | "999993653" | 500     |
    When I evaluate "uitkomst" of "test_scoped_source_afnemer"
    Then the execution succeeds
    # The afnemer gets the computed 1000, not the raw 500.
    Then output "uitkomst" equals 1000

  Scenario: An unscoped source still answers for every law
    Given the following "register" data with key "bsn":
      | bsn         | inkomen |
      | "999993653" | 500     |
    When I evaluate "uitkomst" of "test_scoped_source_afnemer"
    Then the execution succeeds
    # Unscoped, the register value shadows the cross-law input: the
    # afnemer's own `inkomen` input resolves from the source and stays 500.
    Then output "uitkomst" equals 500
