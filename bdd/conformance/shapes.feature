@tier:core
Feature: Shapes and OVERLAPS
  As an author of machine-readable law
  I want to test a location against an area a regulation designates
  So that a rule that holds in one place is decided where the activity is

  # Exercises schema v0.8.23: the value type shape (a string of 2D WKT) and
  # the operation OVERLAPS, which is true when the interiors of two shapes
  # meet (DE-9IM T********). Driven against the test laws
  # corpus/regulation/nl/wet/test_shapes and test_shapes_afnemer.
  #
  # Article 1 of test_shapes designates the area: a strip from x 154000 to
  # 157000 and from y 463600 to 463650. The engine attaches no coordinate
  # system to a shape; the location and the area are in the same one.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: A location inside the area overlaps it
    Given parameter "location" is "POLYGON((155100 463610, 155110 463610, 155110 463620, 155100 463620, 155100 463610))"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then output "ligt_in_beperkingengebied" is true

  Scenario: A location partly on the area overlaps it
    # The lower half of the location lies on the strip, the upper half above it.
    Given parameter "location" is "POLYGON((155100 463640, 155110 463640, 155110 463660, 155100 463660, 155100 463640))"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then output "ligt_in_beperkingengebied" is true

  Scenario: A location that shares only an edge with the area does not overlap it
    # The location stands on the upper edge of the strip, y 463650. The
    # boundaries meet; the interiors do not.
    Given parameter "location" is "POLYGON((155100 463650, 155110 463650, 155110 463660, 155100 463660, 155100 463650))"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then output "ligt_in_beperkingengebied" is false

  Scenario: A location outside the area does not overlap it
    Given parameter "location" is "POLYGON((155100 463700, 155110 463700, 155110 463710, 155100 463710, 155100 463700))"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then output "ligt_in_beperkingengebied" is false

  Scenario: A point inside the area overlaps it
    Given parameter "location" is "POINT(155105 463625)"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then output "ligt_in_beperkingengebied" is true

  Scenario: A point on the boundary of the area does not overlap it
    Given parameter "location" is "POINT(155105 463650)"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then output "ligt_in_beperkingengebied" is false

  Scenario: A location that is not a shape fails, and the error names the operand
    # An area that cannot be read has not been shown to lie outside anything,
    # so the answer is an error and never false.
    Given parameter "location" is "ergens langs de dijk"
    When I evaluate "ligt_in_beperkingengebied" of "test_shapes"
    Then the execution fails
    Then the execution fails with "subject of OVERLAPS"
    Then the execution fails with "does not parse as WKT"

  Scenario: An absent shape fails, as ordering an absence does
    # A row is untyped, so no declaration refuses the null at the boundary;
    # the operation does. Whether nothing overlaps an area is not a question
    # the law asked.
    Given parameter "percelen" is the collection:
      | naam    | vorm |
      | perceel | null |
    When I evaluate "raakt_beperkingengebied" of "test_shapes"
    Then the execution fails
    Then the execution fails with "operand is null"
    Then the execution fails with "OVERLAPS"

  Scenario: One parcel on the area is enough, whatever the other is
    Given parameter "percelen" is the collection:
      | naam   | vorm                                                                                 |
      | buiten | POLYGON((155100 463700, 155110 463700, 155110 463710, 155100 463710, 155100 463700)) |
      | binnen | POINT(155105 463625)                                                                 |
    When I evaluate "raakt_beperkingengebied" of "test_shapes"
    Then output "raakt_beperkingengebied" is true

  Scenario: A law tests its location against the area another law designates
    Given parameter "location" is "POINT(155105 463625)"
    When I evaluate "vergunningplichtig" of "test_shapes_afnemer"
    Then output "vergunningplichtig" is true

  Scenario: Outside the area another law designates, the rule does not apply
    Given parameter "location" is "POINT(153000 463625)"
    When I evaluate "vergunningplichtig" of "test_shapes_afnemer"
    Then output "vergunningplichtig" is false
