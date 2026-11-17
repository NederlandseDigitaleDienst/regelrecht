Feature: May I build this here? - Oude Nijkerkerweg 33 in Putten
  A real plan. The Omgevingsplan gemeente Putten forbids building without a
  permit (article 22.26) and lifts that ban for a bijbehorend bouwwerk that
  meets six requirements (article 22.27, onder a). At Oude Nijkerkerweg 33 a
  separate afdeling holds, with a norm for the surface of bijgebouwen
  (article 21.23).

  Under the Omgevingswet an activity needs a permit when the plan requires
  one for it, or when it conflicts with the plan. The second kind is a
  buitenplanse omgevingsplanactiviteit.

  The question names no rulework: that the location lies in Putten follows
  from the municipal boundary.

  The facts in the Background are what articles that are not modelled would
  decide. They are given here, so no answer rests on a rule that is missing.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.

  Background:
    Given the calculation date is "2026-11-17"
    Given the following parameters:
      | bij_amvb_aangewezen_geval                             | false |
      | artikel_22_28_sluit_uitzondering_uit                  | false |
      | van_rechtswege_in_overeenstemming_artikel_22_36       | false |
      | past_binnen_artikel_19_2_bestemmingsplan              | false |
      | is_bijbehorend_bouwwerk                               | true  |
      | is_bijgebouw                                          | true  |
      | op_de_grond_staand                                    | true  |
      | gelegen_in_achtererfgebied                            | true  |
      | afstand_tot_openbaar_toegankelijk_gebied_m            | 12    |
      | hoogte_m                                              | 4     |
      | verblijfsgebied_alleen_op_eerste_bouwlaag             | true  |
      | heeft_dakterras_balkon_of_buitenruimte_boven_de_grond | false |
      | sloopmeters_eigen_terrein_m2                          | 310   |
      | grootste_bijgebouw_m2                                 | 180   |

  Scenario: A bijgebouw within every limit - no permit
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "250"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "without a permit"

  Scenario: More bijgebouwen than 287 m2 - it conflicts with the plan
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "300"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                             |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.23 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.23 |

  Scenario: Too little demolished in return - it conflicts with the plan
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "250"
    Given parameter "sloopmeters_eigen_terrein_m2" is "200"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                             |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.23 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.23 |

  Scenario: Within the surface norm but 6 m high - the plan's own permit duty
    # Article 22.27 onder a, 4: not higher than 5 m. The exception fails, so
    # the ban of article 22.26 holds. This is not a conflict with the plan.
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "250"
    Given parameter "hoogte_m" is "6"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                             |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 22.26 |

  Scenario: Too large and too high - both grounds at once
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "300"
    Given parameter "hoogte_m" is "6"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                                                                  |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.23, omgevingsplan_gemeente_putten 22.26 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.23                                      |

  Scenario: The same large bijgebouw next to the plot - the norm does not hold there
    # Outside the area article 21.23 is silent. What the bestemmingsplan says
    # there is not modelled; the Background takes it not to fit, but that
    # fact is only read inside the area, so here nothing conflicts.
    Given the location is the shape:
      """
      POLYGON((167900 472300, 167910 472300, 167910 472310, 167900 472310, 167900 472300))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "300"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "without a permit"

  # A building across the line between the area Wonen and the agrarian part.
  # With OVERLAPS the norm for Wonen holds for the whole building. Whether the
  # plan means that is a legal question this example does not answer.
  Scenario: A large bijgebouw across the line between two areas - the norm holds for all of it
    Given the location is the shape:
      """
      POLYGON((167812 472320, 167822 472320, 167822 472330, 167812 472330, 167812 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is "300"
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                             |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.23 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.23 |
