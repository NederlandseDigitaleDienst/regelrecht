@tier:provenance
Feature: Multi-output evaluation
  As a caller of the engine API
  I want to request multiple specific outputs from a single evaluation
  So that I avoid redundant evaluations while only receiving the data I asked for

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Request two outputs from different articles in the same law
    Given the untranslatable mode is "error"
    When I evaluate outputs "basistoeslag, toegekende_toeslag" of "test_untranslatables"
    Then the execution succeeds
    Then output "basistoeslag" equals 1000
    Then output "toegekende_toeslag" equals 2000
    Then output "basistoeslag" has direct provenance
    Then output "toegekende_toeslag" has direct provenance

  Scenario: Single output via multi-output API returns only that output
    Given the untranslatable mode is "error"
    When I evaluate outputs "basistoeslag" of "test_untranslatables"
    Then the execution succeeds
    Then the result contains exactly the outputs "basistoeslag"
    Then output "basistoeslag" equals 1000

  Scenario: Non-existent output returns error
    When I evaluate outputs "nonexistent_output" of "test_untranslatables"
    Then the execution fails with "Output 'nonexistent_output' not found"

  Scenario: Hook outputs are included when requesting a BESCHIKKING output
    Given the calculation date is "2026-01-01"
    When I evaluate outputs "minister_is_bevoegd" of "vreemdelingenwet_2000"
    Then the execution succeeds
    Then output "minister_is_bevoegd" is true
    Then output "minister_is_bevoegd" has direct provenance
    # AWB hooks fire on BESCHIKKING — their outputs are causally entailed
    Then output "motivering_vereist" is true
    Then output "motivering_vereist" has reactive provenance
    Then output "termijn_weken" equals 4
    # Vw art. 69 overrides Awb 6:7 inside the hook; the value's ground is the override
    Then output "termijn_weken" has override provenance
