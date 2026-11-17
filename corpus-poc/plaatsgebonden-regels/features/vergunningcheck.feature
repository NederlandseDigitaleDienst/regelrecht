Feature: May I do this here?
  The question is a shape on the map and an activity. It names no rulework,
  no municipality, no water board and no area. The bans are in the
  Omgevingswet; which plan and which water board fill them in follows from
  the shape.

  "without a permit" means that none of the bans named in questions.yaml
  holds. That file also names the bans it does not cover.

  The Omgevingswet and the Besluit bouwwerken leefomgeving are real here.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.
  Dijkstad and waterschap Rivierdijken are fictional.

  Background:
    Given the calculation date is "2026-11-17"
    # What is built, as far as the Besluit bouwwerken leefomgeving asks.
    Given the following parameters:
      | bouwwerk_genoemd_in_artikel_2_29_onder_a_tot_en_met_q                | false |
      | gelegen_in_achtererfgebied                                           | true  |
      | gelegen_in_voorerfgebied                                             | false |
      | hoogte_m                                                             | 4     |
      | oppervlakte_bouwwerk_m2                                              | 12    |
      | vrijgesteld_volgens_artikel_2_29_onder_a_tot_en_met_q                | false |
      | in_aan_of_op_monument                                                | false |
      | locatie_heeft_functieaanduiding_rijksbeschermd_stads_of_dorpsgezicht | false |

  Scenario: A shed in a residential street - the plan lifts its own ban
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "true"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "without a permit"

  Scenario: A house in that same street - the plan forbids it without a permit
    Given the location is the shape:
      """
      POLYGON((155100 463100, 155112 463100, 155112 463109, 155100 463109, 155100 463100))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "false"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                    |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_dijkstad 5.2 |

  Scenario: A shed in the old town - the exception does not hold in that area
    Given the location is the shape:
      """
      POLYGON((155450 463450, 155462 463450, 155462 463459, 155450 463459, 155450 463450))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "true"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                    |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_dijkstad 5.2 |

  Scenario: A shed on the dike in the old town - two layers stack
    Given the location is the shape:
      """
      POLYGON((155500 463620, 155512 463620, 155512 463629, 155500 463629, 155500 463620))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "true"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                                 |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_dijkstad 5.2              |
      | 5.3     | vergunningplicht_waterschapsverordening  | waterschapsverordening_rivierdijken 3.1 |

  Scenario: A shed half on the dike strip - partial overlap counts
    Given the location is the shape:
      """
      POLYGON((155100 463645, 155112 463645, 155112 463655, 155100 463655, 155100 463645))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "true"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                  | through                                 |
      | 5.3     | vergunningplicht_waterschapsverordening | waterschapsverordening_rivierdijken 3.1 |

  Scenario: A shed against the dike strip - touching the boundary does not count
    Given the location is the shape:
      """
      POLYGON((155100 463650, 155112 463650, 155112 463659, 155100 463659, 155100 463650))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "true"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "without a permit"

  Scenario: A house on the dike outside Dijkstad - only the water board reaches there
    Given the location is the shape:
      """
      POLYGON((156500 463620, 156512 463620, 156512 463629, 156500 463629, 156500 463620))
      """
    Given parameter "is_bijbehorend_bouwwerk" is "false"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                  | through                                 |
      | 5.3     | vergunningplicht_waterschapsverordening | waterschapsverordening_rivierdijken 3.1 |

  Scenario: A low fence post base of 1,5 m2 on the dike in the old town - exempt from one ban only
    # Article 2.29 onder r of the Besluit bouwwerken leefomgeving lifts the
    # ban of article 5.1 lid 1 of the Omgevingswet for a structure of at most
    # 1 m high and 2 m2. Article 5.3 has no such clause, so the water board's
    # ban stays.
    Given the location is the shape:
      """
      POLYGON((155500 463620, 155501 463620, 155501 463621.5, 155500 463621.5, 155500 463620))
      """
    Given the following parameters:
      | is_bijbehorend_bouwwerk | false |
      | hoogte_m                | 0.9   |
      | oppervlakte_bouwwerk_m2 | 1.5   |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                  | through                                 |
      | 5.3     | vergunningplicht_waterschapsverordening | waterschapsverordening_rivierdijken 3.1 |

  # A tree is a point. The plan reads an area that the water board designated.

  Scenario: Felling a tree in a residential street - a point as location
    Given the location is the shape:
      """
      POINT(155105 463105)
      """
    When I ask whether I may "boom_kappen" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                    |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_dijkstad 4.1 |

  Scenario: Felling a tree on the dike - the plan steps back for the water board's area
    Given the location is the shape:
      """
      POINT(155105 463625)
      """
    When I ask whether I may "boom_kappen" at the location
    Then the answer is "without a permit"
