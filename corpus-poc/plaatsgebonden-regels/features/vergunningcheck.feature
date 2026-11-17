Feature: Can I do this here?
  The question is a shape on the map and an activity. Which ruleworks and
  which articles speak about that place is found from the shape; the scenario
  names no municipality, no water board and no area.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.

  Background:
    Given the calculation date is "2026-11-17"

  Scenario: Building in a residential street - nothing applies
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    When I ask what "bouwen" needs at the location
    Then there are no permit duties

  Scenario: Felling a tree in that same street - the plan holds in the whole municipality
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    When I ask what "boom_kappen" needs at the location
    Then the permit duties are:
      | rulework               | article |
      | omgevingsplan_dijkstad | 4.1     |

  Scenario: Building in the old town - one article of the plan has a smaller area
    Given the location is the shape:
      """
      POLYGON((155450 463450, 155462 463450, 155462 463459, 155450 463459, 155450 463450))
      """
    When I ask what "bouwen" needs at the location
    Then the permit duties are:
      | rulework               | article |
      | omgevingsplan_dijkstad | 5.2     |

  Scenario: Building on the dike in the old town - two layers stack
    Given the location is the shape:
      """
      POLYGON((155500 463620, 155512 463620, 155512 463629, 155500 463629, 155500 463620))
      """
    When I ask what "bouwen" needs at the location
    Then the permit duties are:
      | rulework                            | article |
      | omgevingsplan_dijkstad              | 5.2     |
      | waterschapsverordening_rivierdijken | 3.1     |

  Scenario: Building half on the dike strip - partial overlap counts
    Given the location is the shape:
      """
      POLYGON((155100 463645, 155112 463645, 155112 463655, 155100 463655, 155100 463645))
      """
    When I ask what "bouwen" needs at the location
    Then the permit duties are:
      | rulework                            | article |
      | waterschapsverordening_rivierdijken | 3.1     |

  Scenario: Building against the dike strip - touching the boundary does not count
    Given the location is the shape:
      """
      POLYGON((155100 463650, 155112 463650, 155112 463659, 155100 463659, 155100 463650))
      """
    When I ask what "bouwen" needs at the location
    Then there are no permit duties

  Scenario: Building on the dike outside Dijkstad - the water board reaches further than the municipality
    Given the location is the shape:
      """
      POLYGON((156500 463620, 156512 463620, 156512 463629, 156500 463629, 156500 463620))
      """
    When I ask what "bouwen" needs at the location
    Then the permit duties are:
      | rulework                            | article |
      | waterschapsverordening_rivierdijken | 3.1     |

  Scenario: Felling a tree outside Dijkstad - the plan does not reach there
    Given the location is the shape:
      """
      POLYGON((156500 463620, 156512 463620, 156512 463629, 156500 463629, 156500 463620))
      """
    When I ask what "boom_kappen" needs at the location
    Then there are no permit duties
