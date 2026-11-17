@tier:core
Feature: An open term filled by the regulations of a place
  As an author of machine-readable law
  I want an open term to be filled by the regulation of the place a case is in
  So that the caller gives a location and not the code of an authority

  # Exercises schema v0.8.23 on an open term: `location`, a reference to a
  # shape that decides which authority's regulation fills the term;
  # `territory`, the output of a law that gives the territory of an
  # authority; and `combine: OR`, which asks every implementation instead of
  # one winner. Driven against the test law
  # corpus/regulation/nl/wet/test_location_terms and three municipal test
  # regulations.
  #
  # The territories come from test_grenzenregister, a law that plays the
  # register of boundaries: its article reads the boundary of a municipality
  # as a `source: {}` input and gives it as its output. Each scenario gets
  # the register as a data source bound to that law, as any register is
  # given. The engine asks the law with the code a regulation carries.
  #
  # Linkerdorp (GM9901) is the square from x 0 to 10, Rechterdorp (GM9902)
  # the square next to it, from x 10 to 20. Linkerdorp's plan forbids
  # building in article 1 and felling in article 2; Rechterdorp's plan
  # forbids demolishing a monument. The register has no row for Nergenshuizen
  # (GM9909).

  Background:
    Given the calculation date is "2025-01-01"
    And the following "grenzenregister" data with key "gemeente_code" for law "test_grenzenregister":
      | gemeente_code | grens_in_register                          |
      | GM9901        | POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))     |
      | GM9902        | POLYGON((10 0, 20 0, 20 10, 10 10, 10 0))  |

  Scenario: A location in one territory asks the plan of that municipality
    Given parameter "location" is "POINT(5 5)"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is true

  Scenario: The plan of the municipality next door is not asked
    # Building is forbidden in Linkerdorp only. In Rechterdorp its plan is
    # asked, says nothing about building, and the term is false.
    Given parameter "location" is "POINT(15 5)"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is false

  Scenario: A code among the parameters plays no part for a term with a location
    Given parameter "location" is "POINT(5 5)"
    And parameter "gemeente_code" is "GM9902"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is true

  Scenario: A location across two territories asks the plan of the one
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is true

  Scenario: A location across two territories asks the plan of the other too
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    And parameter "activiteit" is "slopen"
    And parameter "monument" is "true"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is true

  Scenario: A location in no territory leaves the required term without an implementation
    Given parameter "location" is "POINT(50 50)"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "verboden_in_omgevingsplan"
    Then the execution fails with "has no implementation and no default"

  Scenario: A location on the boundary of a territory is not in it
    # OVERLAPS asks for interiors that meet. A point on the edge that
    # Linkerdorp and Rechterdorp share lies in neither.
    Given parameter "location" is "POINT(10 5)"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then the execution fails with "has no implementation and no default"

  Scenario: A regulation whose territory the register does not hold leaves the term unknown
    # The register has no row for Nergenshuizen, so whether its regulation
    # applies here is not known. That is never read as "it does not": in
    # Rechterdorp, where no plan speaks about terraces, the term is unknown
    # for lack of the boundary in the register.
    Given parameter "location" is "POINT(15 5)"
    When I evaluate "terras_niet_toegestaan" of "test_location_terms"
    Then the execution succeeds
    Then output "terras_niet_toegestaan" is unknown for lack of "grens_in_register"

  Scenario: A plan in scope that says so wins over a territory the register does not hold
    # In Linkerdorp the plan forbids a terrace. Whatever Nergenshuizen's
    # regulation would add, the term is true.
    Given parameter "location" is "POINT(5 5)"
    When I evaluate "terras_niet_toegestaan" of "test_location_terms"
    Then output "terras_niet_toegestaan" is true

  Scenario: With the boundary in the register the regulation is held against the location
    Given the following "grenzenregister" data with key "gemeente_code" for law "test_grenzenregister":
      | gemeente_code | grens_in_register                      |
      | GM9909        | POLYGON((10 0, 16 0, 16 6, 10 6, 10 0)) |
    And parameter "location" is "POINT(15 5)"
    When I evaluate "terras_niet_toegestaan" of "test_location_terms"
    Then output "terras_niet_toegestaan" is true

  Scenario: A territory asked of a law that is not loaded is an error that names the term and the source
    # A defect of the term, whatever a regulation would say: Linkerdorp's
    # verordening forbids the event, and the term still fails.
    Given parameter "location" is "POINT(5 5)"
    When I evaluate "evenement_niet_toegestaan" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "Open term 'evenement_verboden' on test_location_terms#5"
    Then the execution fails with "reads its territory from test_register_dat_niet_bestaat#grens_van_gemeente"
    Then the execution fails with "no law 'test_register_dat_niet_bestaat' is loaded"

  Scenario: With combine, a territory that cannot be read does not fail a case the own plan decides
    # The register holds a boundary for Nergenshuizen that crosses itself.
    # The location lies in Linkerdorp, whose plan forbids a terrace: the term
    # is true, and the failure of the other territory is recorded, not raised.
    Given the following "grenzenregister" data with key "gemeente_code" for law "test_grenzenregister":
      | gemeente_code | grens_in_register                      |
      | GM9909        | POLYGON((30 0, 40 10, 40 0, 30 10, 30 0)) |
    And parameter "location" is "POINT(5 5)"
    When I evaluate "terras_niet_toegestaan" of "test_location_terms"
    Then output "terras_niet_toegestaan" is true

  Scenario: With combine, a territory that cannot be read fails the case when no plan says so
    # In Rechterdorp no plan speaks about terraces, so nothing outweighs the
    # territory that could not be read.
    Given the following "grenzenregister" data with key "gemeente_code" for law "test_grenzenregister":
      | gemeente_code | grens_in_register                      |
      | GM9909        | POLYGON((30 0, 40 10, 40 0, 30 10, 30 0)) |
    And parameter "location" is "POINT(15 5)"
    When I evaluate "terras_niet_toegestaan" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "Open term 'terras_verboden' on test_location_terms#2"
    Then the execution fails with "the territory of 'GM9909'"
    Then the execution fails with "from test_grenzenregister#grens_van_gemeente is no shape"
    Then the execution fails with "is not a valid geometry"

  Scenario: Of two articles of one plan, one that says so is enough
    # Article 1 of Linkerdorp's plan is silent on felling; article 2 forbids it.
    Given parameter "location" is "POINT(5 5)"
    And parameter "activiteit" is "kappen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is true

  Scenario: When every implementation is silent the term is false
    # The plan is there and no article of it forbids walking: that is a no,
    # not a missing plan.
    Given parameter "location" is "POINT(5 5)"
    And parameter "activiteit" is "wandelen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is false

  Scenario: An implementation that says no leaves the term false
    Given parameter "location" is "POINT(15 5)"
    And parameter "activiteit" is "slopen"
    And parameter "monument" is "false"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is false

  Scenario: One implementation that cannot say, and none that says so, leaves the term unknown
    # Rechterdorp forbids demolishing a monument, and nobody said whether
    # this is one.
    Given parameter "location" is "POINT(15 5)"
    And parameter "activiteit" is "slopen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is unknown for lack of "monument"

  Scenario: Silent implementations beside one that cannot say leave the term unknown
    # Across both territories the two articles of Linkerdorp's plan are
    # silent on demolishing, and Rechterdorp still cannot say.
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    And parameter "activiteit" is "slopen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is unknown for lack of "monument"

  Scenario: Without combine a location in one territory takes the value of its plan
    Given parameter "location" is "POINT(15 5)"
    When I evaluate "toegestane_bouwhoogte" of "test_location_terms"
    Then output "toegestane_bouwhoogte" equals 12

  Scenario: Without combine two plans of one layer and date leave no winner
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    When I evaluate "toegestane_bouwhoogte" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "Ambiguous priority"

  Scenario: A term without a location is still matched by the code among the parameters
    Given parameter "gemeente_code" is "GM9902"
    When I evaluate "verschuldigde_leges" of "test_location_terms"
    Then output "verschuldigde_leges" equals 250

  Scenario: A term without a location does not ask for a territory
    Given parameter "gemeente_code" is "GM9903"
    When I evaluate "verschuldigde_leges" of "test_location_terms"
    Then the execution fails with "has no implementation and no default"
