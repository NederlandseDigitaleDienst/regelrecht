Feature: Bezwaartermijn chain
  As a citizen receiving a government decision
  I want to know when the objection deadline expires
  So that I can file an objection in time

  # This feature tests RFC-007 (hooks, overrides) working together:
  # - Hooks: AWB articles fire automatically on BESCHIKKING
  # - Overrides: Vreemdelingenwet overrides AWB 6:7 (lex specialis)

  Background:
    Given the calculation date is "2026-01-01"
    Given law "algemene_wet_bestuursrecht" is loaded

  Scenario: Vreemdelingenwet beschikking triggers AWB hooks with override
    When I evaluate "minister_is_bevoegd" of "vreemdelingenwet_2000"
    Then the execution succeeds
    Then output "minister_is_bevoegd" is true
    # AWB 3:46 hook fires pre_actions on BESCHIKKING
    Then output "motivering_vereist" is true
    # AWB 6:7 hook fires post_actions, but Vw art 69 overrides to 4 weeks
    Then output "termijn_weken" equals 4

  # NB: Art 14 merely states that the Minister is competent to grant/deny.
  # The article has no conditions — it is a declaratory statement of
  # authority. The engine must always return true, never false.
  Scenario: Vreemdelingenwet Art 14 — minister_is_bevoegd is always true
    When I evaluate "minister_is_bevoegd" of "vreemdelingenwet_2000"
    Then the execution succeeds
    Then output "minister_is_bevoegd" is true
