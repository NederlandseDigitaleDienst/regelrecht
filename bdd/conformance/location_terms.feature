@tier:core
Feature: An open term filled by the regulations of a place
  As an author of machine-readable law
  I want an open term to be filled by the regulation of the place a case is in
  So that the caller gives a location and not the code of an authority

  # Exercises schema v0.8.23 on an open term: `location`, a reference to a
  # shape that decides which authority's regulation fills the term, and
  # `combine: OR`, which asks every implementation instead of one winner.
  # Driven against the test law corpus/regulation/nl/wet/test_location_terms
  # and three municipal test regulations.
  #
  # Linkerdorp (GM9901) is the square from x 0 to 10, Rechterdorp (GM9902)
  # the square next to it, from x 10 to 20. Its plan forbids building in
  # article 1 and felling in article 2; Rechterdorp's plan forbids
  # demolishing a monument. No scenario gives the territory of Nergenshuizen
  # (GM9909).

  Background:
    Given the calculation date is "2025-01-01"
    And the territory of "GM9901" is "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))"
    And the territory of "GM9902" is "POLYGON((10 0, 20 0, 20 10, 10 10, 10 0))"

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

  Scenario: A regulation whose territory nobody gave is an error that names the code
    # Whether the regulation of Nergenshuizen applies here is not known.
    # That is never read as "it does not".
    Given parameter "location" is "POINT(5 5)"
    When I evaluate "terras_niet_toegestaan" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "no territory is registered for 'GM9909'"
    Then the execution fails with "test_terrasverordening_nergenshuizen"

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

  Scenario: A term without a location does not look at the territories
    Given parameter "gemeente_code" is "GM9903"
    When I evaluate "verschuldigde_leges" of "test_location_terms"
    Then the execution fails with "has no implementation and no default"
