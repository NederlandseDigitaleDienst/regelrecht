Feature: May I build this here? - Oude Nijkerkerweg 33 in Putten
  A real plan. Afdeling 21.5 of the Omgevingsplan gemeente Putten holds only
  at one location. Bijlage I of the plan lists four areas for it, each with
  its own published geometry. Inside the location one area has the function
  Wonen, and there a norm for bijgebouwen holds.

  The question names no rulework: that the location lies in Putten follows
  from the municipal boundary.

  Coordinates are in the Dutch RD system (EPSG:28992), in metres.

  Background:
    Given the calculation date is "2026-11-17"

  Scenario: Bijgebouwen within the norm, in the area where the norm holds
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I ask whether I may "bijgebouw_bouwen" at the location
    Then the answer is "toegestaan"
    Then because of:
      | rulework                      | article | qualification | through |
      | omgevingsplan_gemeente_putten | 21.23   | toegestaan    |         |

  Scenario: More bijgebouwen than 287 m2
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 300
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I ask whether I may "bijgebouw_bouwen" at the location
    Then the answer is "verbod"
    Then because of:
      | rulework                      | article | qualification | through |
      | omgevingsplan_gemeente_putten | 21.23   | verbod        |         |

  Scenario: Too little demolished in return
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 200
    Given parameter "grootste_bijgebouw_m2" is 180
    When I ask whether I may "bijgebouw_bouwen" at the location
    Then the answer is "verbod"
    Then because of:
      | rulework                      | article | qualification | through |
      | omgevingsplan_gemeente_putten | 21.23   | verbod        |         |

  Scenario: On the same plot but outside the area Wonen - the article is silent
    Given the location is the shape:
      """
      POLYGON((167800 472255, 167810 472255, 167810 472265, 167800 472265, 167800 472255))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I ask whether I may "bijgebouw_bouwen" at the location
    Then the answer is "geen_regel"

  Scenario: Next to the plot - the afdeling does not apply
    Given the location is the shape:
      """
      POLYGON((167900 472300, 167910 472300, 167910 472310, 167900 472310, 167900 472300))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I ask whether I may "bijgebouw_bouwen" at the location
    Then the answer is "geen_regel"

  # A building across the line between the area Wonen and the agrarian part.
  # With OVERLAPS the norm for Wonen holds for the whole building. Whether the
  # plan means that is a legal question this example does not answer.
  Scenario: A building across the line between two areas - the norm holds for all of it
    Given the location is the shape:
      """
      POLYGON((167812 472320, 167822 472320, 167822 472330, 167812 472330, 167812 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I ask whether I may "bijgebouw_bouwen" at the location
    Then the answer is "toegestaan"
    Then because of:
      | rulework                      | article | qualification | through |
      | omgevingsplan_gemeente_putten | 21.23   | toegestaan    |         |

  Scenario: Felling a tree at that location - the article is about something else
    Given the location is the shape:
      """
      POINT(167800 472325)
      """
    When I ask whether I may "boom_kappen" at the location
    Then the answer is "geen_regel"
