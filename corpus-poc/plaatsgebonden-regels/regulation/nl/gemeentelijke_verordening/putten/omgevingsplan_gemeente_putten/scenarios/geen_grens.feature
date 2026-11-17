Feature: A location, and no boundary from the register
  Which plan speaks about a place follows from the boundary of the
  municipality, and that boundary is a fact from the Basisregistratie Kadaster
  (Kadasterwet article 48 lid 3 onder c). This feature gives the register
  nothing.

  Scenario: Without the boundary of Putten it cannot be said whether its plan speaks
    # The plan of Putten is loaded, but whether the location lies in Putten is
    # not known. The law does not read that as a place without a plan.
    Given the calculation date is "2026-11-17"
    Given parameter "location" is "POLYGON((167795 472320, 167801 472320, 167801 472330, 167795 472330, 167795 472320))"
    Given parameter "activiteit" is "bouwactiviteit"
    When I evaluate "omgevingsplanactiviteit" of "omgevingswet"
    Then output "omgevingsplanactiviteit" is unknown for lack of "grens_van_gemeente"
