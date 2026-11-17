Feature: May I do this here?
  The question is a shape on the map and an activity. It names no rulework,
  no municipality, no water board and no area: which articles speak follows
  from the shape.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.
  Dijkstad and waterschap Rivierdijken are fictional.

  Background:
    Given the calculation date is "2026-11-17"

  Scenario: A shed in a residential street - no article speaks
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    When I ask whether I may "bijbehorend_bouwwerk_bouwen" at the location
    Then the answer is "geen_regel"

  Scenario: A house in that same street - the plan forbids it without a permit
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    When I ask whether I may "hoofdgebouw_bouwen" at the location
    Then the answer is "vergunningplicht"
    Then because of:
      | rulework     | article | qualification    | through                    |
      | omgevingswet | 5.1     | vergunningplicht | omgevingsplan_dijkstad 5.2 |

  Scenario: A shed in the old town - the exception does not hold in that area
    Given the location is the shape:
      """
      POLYGON((155450 463450, 155462 463450, 155462 463459, 155450 463459, 155450 463450))
      """
    When I ask whether I may "bijbehorend_bouwwerk_bouwen" at the location
    Then the answer is "vergunningplicht"
    Then because of:
      | rulework     | article | qualification    | through                    |
      | omgevingswet | 5.1     | vergunningplicht | omgevingsplan_dijkstad 5.2 |

  Scenario: A shed on the dike in the old town - two layers stack
    Given the location is the shape:
      """
      POLYGON((155500 463620, 155512 463620, 155512 463629, 155500 463629, 155500 463620))
      """
    When I ask whether I may "bijbehorend_bouwwerk_bouwen" at the location
    Then the answer is "vergunningplicht"
    Then because of:
      | rulework     | article | qualification    | through                                 |
      | omgevingswet | 5.1     | vergunningplicht | omgevingsplan_dijkstad 5.2              |
      | omgevingswet | 5.3     | vergunningplicht | waterschapsverordening_rivierdijken 3.1 |

  Scenario: A shed half on the dike strip - partial overlap counts
    Given the location is the shape:
      """
      POLYGON((155100 463645, 155112 463645, 155112 463655, 155100 463655, 155100 463645))
      """
    When I ask whether I may "bijbehorend_bouwwerk_bouwen" at the location
    Then the answer is "vergunningplicht"
    Then because of:
      | rulework     | article | qualification    | through                                 |
      | omgevingswet | 5.3     | vergunningplicht | waterschapsverordening_rivierdijken 3.1 |

  Scenario: A shed against the dike strip - touching the boundary does not count
    Given the location is the shape:
      """
      POLYGON((155100 463650, 155112 463650, 155112 463659, 155100 463659, 155100 463650))
      """
    When I ask whether I may "bijbehorend_bouwwerk_bouwen" at the location
    Then the answer is "geen_regel"

  Scenario: A house on the dike outside Dijkstad - only the water board reaches there
    Given the location is the shape:
      """
      POLYGON((156500 463620, 156512 463620, 156512 463629, 156500 463629, 156500 463620))
      """
    When I ask whether I may "hoofdgebouw_bouwen" at the location
    Then the answer is "vergunningplicht"
    Then because of:
      | rulework     | article | qualification    | through                                 |
      | omgevingswet | 5.3     | vergunningplicht | waterschapsverordening_rivierdijken 3.1 |

  # A tree is a point. The plan reads an area that the water board designated.

  Scenario: Felling a tree in a residential street - a point as location
    Given the location is the shape:
      """
      POINT(155105 463105)
      """
    When I ask whether I may "boom_kappen" at the location
    Then the answer is "vergunningplicht"
    Then because of:
      | rulework     | article | qualification    | through                    |
      | omgevingswet | 5.1     | vergunningplicht | omgevingsplan_dijkstad 4.1 |

  Scenario: Felling a tree on the dike - the plan steps back for the water board's area
    Given the location is the shape:
      """
      POINT(155105 463625)
      """
    When I ask whether I may "boom_kappen" at the location
    Then the answer is "geen_regel"

  # The same case asked of the law itself. The ban sits in the Omgevingswet;
  # the plan and the water board only fill what the law leaves open.

  Scenario: A shed on the dike in the old town - asked of the Omgevingswet
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
