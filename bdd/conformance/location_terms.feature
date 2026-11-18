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
  # (GM9909). Around the two lies nothing: no plan is known there.

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

  Scenario: Two territories that share a boundary cover a location across it
    # Both plans are asked and neither forbids walking. No part of the
    # location lies outside the two, so that is the answer: false.
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    And parameter "activiteit" is "wandelen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is false

  Scenario: A location that reaches past the territories is not answered by the plans alone
    # Half of the location lies in Rechterdorp and half beyond it, where no
    # plan is known. Rechterdorp's plan says nothing about building, and
    # that is no answer for the half beyond.
    Given parameter "location" is "POLYGON((12 4, 22 4, 22 6, 12 6, 12 4))"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "verboden_in_omgevingsplan"
    Then the execution fails with "has no implementation and no default for a part of its location"
    Then the execution fails with "lies in no territory for which a regulation is known"

  Scenario: A location around a corner of a boundary reaches outside it
    # The location holds the corner of Linkerdorp at 0 0, so part of it lies
    # in no municipality the register knows.
    Given parameter "location" is "POLYGON((-1 -1, 4 -1, 4 4, -1 4, -1 -1))"
    And parameter "activiteit" is "wandelen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then the execution fails with "has no implementation and no default for a part of its location"

  Scenario: A plan in scope that says so decides, also where the location reaches outside
    Given parameter "location" is "POLYGON((-1 -1, 4 -1, 4 4, -1 4, -1 -1))"
    And parameter "activiteit" is "bouwen"
    When I evaluate "vergunningplichtig" of "test_location_terms"
    Then output "vergunningplichtig" is true

  Scenario: Where a plan covers the location the default is not asked
    # Linkerdorp's plan allows a jetty. The default of the term, a ban, is
    # for where no plan is known.
    Given parameter "location" is "POINT(5 5)"
    When I evaluate "steiger_niet_toegestaan" of "test_location_terms"
    Then output "steiger_niet_toegestaan" is false

  Scenario: Where no plan is known the default is the term
    Given parameter "location" is "POINT(50 50)"
    When I evaluate "steiger_niet_toegestaan" of "test_location_terms"
    Then output "steiger_niet_toegestaan" is true

  Scenario: For the part of a location outside the plan the default is one more answer
    # The plan allows a jetty in Linkerdorp. Part of the location lies
    # outside Linkerdorp, where the default forbids it: the term is true.
    Given parameter "location" is "POLYGON((-1 -1, 4 -1, 4 4, -1 4, -1 -1))"
    When I evaluate "steiger_niet_toegestaan" of "test_location_terms"
    Then output "steiger_niet_toegestaan" is true

  Scenario: The territory of a municipality whose plan does not fill the term is not covered
    # Rechterdorp has no rule on jetties, so for this term no regulation is
    # known there. The location lies half in Rechterdorp: the default
    # answers for that half.
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    When I evaluate "steiger_niet_toegestaan" of "test_location_terms"
    Then output "steiger_niet_toegestaan" is true

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

  Scenario: Without combine a location across two territories is refused
    # One regulation fills such a term for its whole location. The two plans
    # are of one layer and date, but that is not why: see the next two.
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    When I evaluate "toegestane_bouwhoogte" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "Ambiguous"
    Then the execution fails with "its location lies in more than one territory: GM9901 (test_omgevingsplan_linkerdorp), GM9902 (test_omgevingsplan_rechterdorp)"

  Scenario: Without combine the newer plan of another municipality does not win
    # The plan of Linkerdorp is of 2025-01-01 and the verordening of
    # Rechterdorp of 2024-07-01. Lex posterior ranks the regulations of one
    # territory. Two municipalities are not each other's later law.
    Given parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    When I evaluate "toegestane_goothoogte" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "its location lies in more than one territory: GM9901 (test_omgevingsplan_linkerdorp), GM9902 (test_goothoogteverordening_rechterdorp)"

  Scenario: Without combine the newer plan does not win the other way round either
    # From 2025-03-01 Rechterdorp has a new verordening, newer than the plan
    # of Linkerdorp.
    Given the calculation date is "2025-06-01"
    And parameter "location" is "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))"
    When I evaluate "toegestane_goothoogte" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "its location lies in more than one territory: GM9901 (test_omgevingsplan_linkerdorp), GM9902 (test_goothoogteverordening_rechterdorp)"

  Scenario: Without combine each regulation fills the term in its own territory
    Given parameter "location" is "POINT(5 5)"
    When I evaluate "toegestane_goothoogte" of "test_location_terms"
    Then output "toegestane_goothoogte" equals 6

  Scenario: Without combine the version in force of a regulation fills the term
    Given the calculation date is "2025-06-01"
    And parameter "location" is "POINT(15 5)"
    When I evaluate "toegestane_goothoogte" of "test_location_terms"
    Then output "toegestane_goothoogte" equals 5

  Scenario: Without combine a location that reaches past its plan is refused
    # Part of the location lies outside Linkerdorp, where no plan is known:
    # the plan of Linkerdorp does not hold for the whole location.
    Given parameter "location" is "POLYGON((-1 -1, 4 -1, 4 4, -1 4, -1 -1))"
    When I evaluate "toegestane_bouwhoogte" of "test_location_terms"
    Then the execution fails
    Then the execution fails with "Ambiguous"
    Then the execution fails with "part of its location lies in no territory for which a regulation is known"

  Scenario: A term without a location is still matched by the code among the parameters
    Given parameter "gemeente_code" is "GM9902"
    When I evaluate "verschuldigde_leges" of "test_location_terms"
    Then output "verschuldigde_leges" equals 250

  Scenario: A term without a location does not ask for a territory
    Given parameter "gemeente_code" is "GM9903"
    When I evaluate "verschuldigde_leges" of "test_location_terms"
    Then the execution fails with "has no implementation and no default"
