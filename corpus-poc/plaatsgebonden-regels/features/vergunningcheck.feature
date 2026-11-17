Feature: Is a permit required for this, here?
  The question goes to the Omgevingswet, with a shape on the map and an
  activity. Which decentral ruleworks speak about that place follows from the
  shape and from delegation; the scenario names no municipality, no water
  board and no area.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.

  Background:
    Given the calculation date is "2026-11-17"

  Scenario: A shed in a residential street - the plan lifts its own ban
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    Given parameter "activiteit" is "bijbehorend_bouwwerk_bouwen"
    When I evaluate "vergunningplicht_omgevingsplanactiviteit" of "omgevingswet"
    Then output "vergunningplicht_omgevingsplanactiviteit" is false
    When I evaluate "vergunningplicht_waterschapsverordening" of "omgevingswet"
    Then output "vergunningplicht_waterschapsverordening" is false

  Scenario: A house in that same street - the ban holds in the whole municipality
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    Given parameter "activiteit" is "hoofdgebouw_bouwen"
    When I evaluate "vergunningplicht_omgevingsplanactiviteit" of "omgevingswet"
    Then output "vergunningplicht_omgevingsplanactiviteit" is true
    Then output "vergunningplicht_omgevingsplanactiviteit" is filled by:
      | rulework               | article |
      | omgevingsplan_dijkstad | 5.2     |

  Scenario: Felling a tree in that same street - another article of the same plan
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    Given parameter "activiteit" is "boom_kappen"
    When I evaluate "vergunningplicht_omgevingsplanactiviteit" of "omgevingswet"
    Then output "vergunningplicht_omgevingsplanactiviteit" is true
    Then output "vergunningplicht_omgevingsplanactiviteit" is filled by:
      | rulework               | article |
      | omgevingsplan_dijkstad | 4.1     |

  Scenario: A shed in the old town - the exception does not hold in that area
    Given the location is the shape:
      """
      POLYGON((155450 463450, 155462 463450, 155462 463459, 155450 463459, 155450 463450))
      """
    Given parameter "activiteit" is "bijbehorend_bouwwerk_bouwen"
    When I evaluate "vergunningplicht_omgevingsplanactiviteit" of "omgevingswet"
    Then output "vergunningplicht_omgevingsplanactiviteit" is true
    Then output "vergunningplicht_omgevingsplanactiviteit" is filled by:
      | rulework               | article |
      | omgevingsplan_dijkstad | 5.2     |
    When I evaluate "vergunningplicht_waterschapsverordening" of "omgevingswet"
    Then output "vergunningplicht_waterschapsverordening" is false

  Scenario: A shed on the dike in the old town - two layers stack
    Given the location is the shape:
      """
      POLYGON((155500 463620, 155512 463620, 155512 463629, 155500 463629, 155500 463620))
      """
    Given parameter "activiteit" is "bijbehorend_bouwwerk_bouwen"
    When I evaluate "vergunningplicht_omgevingsplanactiviteit" of "omgevingswet"
    Then output "vergunningplicht_omgevingsplanactiviteit" is true
    Then output "vergunningplicht_omgevingsplanactiviteit" is filled by:
      | rulework               | article |
      | omgevingsplan_dijkstad | 5.2     |
    When I evaluate "vergunningplicht_waterschapsverordening" of "omgevingswet"
    Then output "vergunningplicht_waterschapsverordening" is true
    Then output "vergunningplicht_waterschapsverordening" is filled by:
      | rulework                            | article |
      | waterschapsverordening_rivierdijken | 3.1     |

  Scenario: A shed half on the dike strip - partial overlap counts
    Given the location is the shape:
      """
      POLYGON((155100 463645, 155112 463645, 155112 463655, 155100 463655, 155100 463645))
      """
    Given parameter "activiteit" is "bijbehorend_bouwwerk_bouwen"
    When I evaluate "vergunningplicht_waterschapsverordening" of "omgevingswet"
    Then output "vergunningplicht_waterschapsverordening" is true

  Scenario: A shed against the dike strip - touching the boundary does not count
    Given the location is the shape:
      """
      POLYGON((155100 463650, 155112 463650, 155112 463659, 155100 463659, 155100 463650))
      """
    Given parameter "activiteit" is "bijbehorend_bouwwerk_bouwen"
    When I evaluate "vergunningplicht_waterschapsverordening" of "omgevingswet"
    Then output "vergunningplicht_waterschapsverordening" is false

  Scenario: A house on the dike outside Dijkstad - no plan, so the default of the law
    Given the location is the shape:
      """
      POLYGON((156500 463620, 156512 463620, 156512 463629, 156500 463629, 156500 463620))
      """
    Given parameter "activiteit" is "hoofdgebouw_bouwen"
    When I evaluate "vergunningplicht_omgevingsplanactiviteit" of "omgevingswet"
    Then output "vergunningplicht_omgevingsplanactiviteit" is false
    When I evaluate "vergunningplicht_waterschapsverordening" of "omgevingswet"
    Then output "vergunningplicht_waterschapsverordening" is true
    Then output "vergunningplicht_waterschapsverordening" is filled by:
      | rulework                            | article |
      | waterschapsverordening_rivierdijken | 3.1     |
