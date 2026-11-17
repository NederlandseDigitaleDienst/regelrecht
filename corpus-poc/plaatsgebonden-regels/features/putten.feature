Feature: May I build this here? - Oude Nijkerkerweg 33 in Putten
  Real law, from four ruleworks:
  - the Omgevingswet forbids an omgevingsplanactiviteit without a permit;
  - the Besluit bouwwerken leefomgeving exempts some structures from that;
  - the Omgevingsplan gemeente Putten forbids building without a permit, with
    an exception for a bijbehorend bouwwerk, and has an afdeling for this one
    location;
  - the bestemmingsplan of 2018, which is part of that plan, limits
    bijgebouwen to 80 m2.

  Under the Omgevingswet an activity needs a permit when the plan requires
  one for it, or when it conflicts with the plan. The second kind is a
  buitenplanse omgevingsplanactiviteit.

  The question names no rulework: that the location lies in Putten follows
  from the municipal boundary.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.

  Background:
    Given the calculation date is "2026-11-17"
    # What is built: a free-standing bijgebouw behind the house.
    Given the following parameters:
      | is_bijbehorend_bouwwerk                                                | true  |
      | is_bijgebouw                                                           | true  |
      | bouwwerk_genoemd_in_artikel_2_29_onder_a_tot_en_met_q                  | false |
      | op_de_grond_staand                                                     | true  |
      | gelegen_in_achtererfgebied                                             | true  |
      | gelegen_in_voorerfgebied                                               | false |
      | afstand_tot_openbaar_toegankelijk_gebied_m                             | 12    |
      | afstand_tot_oorspronkelijk_hoofdgebouw_m                               | 10    |
      | hoogte_m                                                               | 4     |
      | goothoogte_m                                                           | 3     |
      | heeft_schuin_dak                                                       | true  |
      | hoogte_dakvoet_m                                                       | 2.8   |
      | daknok_gevormd_door_twee_of_meer_schuine_dakvlakken                    | true  |
      | hellingshoek_graden                                                    | 40    |
      | afstand_daknok_tot_perceelsgrens_m                                     | 8     |
      | verblijfsgebied_op_tweede_bouwlaag_of_hoger                            | false |
      | heeft_dakterras_balkon_of_buitenruimte_boven_de_grond                  | false |
      | functioneel_ondergeschikt_aan_hoofdgebouw                              | true  |
      | huisvesting_in_verband_met_mantelzorg                                  | false |
      | oppervlakte_bouwwerk_m2                                                | 60    |
      | grootste_bijgebouw_m2                                                  | 180   |
      | sloopmeters_eigen_terrein_m2                                           | 310   |
      | bebouwingsgebied_m2                                                    | 2000  |
      | uitbreiding_van_of_gelegen_aan_of_bij_hoofdgebouw                      | true  |
      | hoofdgebouw_is_woonwagen_tijdelijk_vergund_of_recreatief_nachtverblijf | false |
      | woning_is_wooncomplex_kleine_woning_of_recreatiewoning                 | false |
      | vrijgesteld_volgens_artikel_2_29_onder_a_tot_en_met_q                  | false |
      | vrijgesteld_volgens_artikel_22_27_onder_b_tot_en_met_i                 | false |
      | in_overeenstemming_volgens_artikel_22_36_onder_b_of_c                  | false |
      | in_aan_of_op_monument                                                  | false |
      | bij_monument                                                           | false |
      | locatie_heeft_functieaanduiding_rijksbeschermd_stads_of_dorpsgezicht   | false |
      | archeologische_onderzoeksregels_gelden_voor_locatie                    | false |

  Scenario: 250 m2 of bijgebouwen, with the demolition the plan asks - no permit
    # Above the 80 m2 of the bestemmingsplan and above the 150 m2 that
    # accords with the plan by law, but within what article 21.23 adds.
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 250 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 250 |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "without a permit"

  Scenario: 300 m2 of bijgebouwen - it conflicts with the plan
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 300 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 300 |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                            |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.3 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.3 |

  Scenario: 250 m2 of bijgebouwen, with too little demolished - it conflicts with the plan
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 250 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 250 |
      | sloopmeters_eigen_terrein_m2                               | 200 |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                            |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.3 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.3 |

  Scenario: 120 m2 of bijgebouwen, nothing demolished - it accords with the plan by law
    # Above the 80 m2 of the bestemmingsplan and outside what article 21.23
    # adds, but article 22.36 allows up to 150 m2 on this plot in any case.
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 120 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 120 |
      | sloopmeters_eigen_terrein_m2                               | 0   |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "without a permit"

  Scenario: 250 m2 of bijgebouwen with a gutter at 4 m - the bestemmingsplan still holds
    # Article 21.23 widens the surface only. The gutter height of article
    # 19.2.8 onder c of the bestemmingsplan stays at 3,5 m.
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 250 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 250 |
      | goothoogte_m                                               | 4   |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                            |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.3 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.3 |

  Scenario: 250 m2 of bijgebouwen, 5,5 m high - the plan's own permit duty
    # Article 22.27 onder a, 4: not higher than 5 m. The exception fails, so
    # the ban of article 22.26 holds. It does not conflict with the plan: the
    # bestemmingsplan allows 6 m.
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 250 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 250 |
      | hoogte_m                                                   | 5.5 |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                             |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 22.26 |

  Scenario: 250 m2 of bijgebouwen at a monument - the exception is taken away
    # Article 22.28 lid 1.
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 250  |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 250  |
      | in_aan_of_op_monument                                      | true |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article | output                                   | through                             |
      | 5.1     | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 22.26 |

  # A building across the line between the area Wonen and the agrarian part.
  # With OVERLAPS the rules for Wonen hold for the whole building. Whether the
  # plan means that is a legal question this example does not answer.
  Scenario: 300 m2 of bijgebouwen across the line between two areas - the rules hold for all of it
    Given the location is the shape:
      """
      POLYGON((167812 472320, 167822 472320, 167822 472330, 167812 472330, 167812 472320))
      """
    Given the following parameters:
      | oppervlakte_bijgebouwen_m2                                 | 300 |
      | oppervlakte_bijbehorende_bouwwerken_in_bebouwingsgebied_m2 | 300 |
    When I ask whether I may "bouwactiviteit" at the location
    Then the answer is "only with a permit"
    Then because of:
      | article         | output                                   | through                            |
      | 5.1             | vergunningplicht_omgevingsplanactiviteit | omgevingsplan_gemeente_putten 21.3 |
      | Bijlage onder A | buitenplanse_omgevingsplanactiviteit     | omgevingsplan_gemeente_putten 21.3 |
