Feature: A real omgevingsplan - Oude Nijkerkerweg 33 in Putten
  Afdeling 21.5 of the Omgevingsplan gemeente Putten holds only at one
  location. Bijlage I of the plan lists four areas for it, each with its own
  published geometry. Inside the location one area has the function Wonen,
  and there a norm for bijgebouwen holds.

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
    When I evaluate "afdeling_21_5_van_toepassing" of "omgevingsplan_gemeente_putten"
    Then output "afdeling_21_5_van_toepassing" is true
    When I evaluate "bestemming_wonen_van_toepassing" of "omgevingsplan_gemeente_putten"
    Then output "bestemming_wonen_van_toepassing" is true
    When I evaluate "oppervlakte_bijgebouwen_toegestaan" of "omgevingsplan_gemeente_putten"
    Then output "oppervlakte_bijgebouwen_toegestaan" is true

  Scenario: More bijgebouwen than 287 m2
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 300
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I evaluate "oppervlakte_bijgebouwen_toegestaan" of "omgevingsplan_gemeente_putten"
    Then output "oppervlakte_bijgebouwen_toegestaan" is false

  Scenario: Too little demolished in return
    Given the location is the shape:
      """
      POLYGON((167795 472320, 167805 472320, 167805 472330, 167795 472330, 167795 472320))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 200
    Given parameter "grootste_bijgebouw_m2" is 180
    When I evaluate "oppervlakte_bijgebouwen_toegestaan" of "omgevingsplan_gemeente_putten"
    Then output "oppervlakte_bijgebouwen_toegestaan" is false

  Scenario: On the same plot but outside the area Wonen - the articles are silent
    Given the location is the shape:
      """
      POLYGON((167800 472255, 167810 472255, 167810 472265, 167800 472265, 167800 472255))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I evaluate "afdeling_21_5_van_toepassing" of "omgevingsplan_gemeente_putten"
    Then output "afdeling_21_5_van_toepassing" is true
    When I evaluate "bestemming_wonen_van_toepassing" of "omgevingsplan_gemeente_putten"
    Then output "bestemming_wonen_van_toepassing" is absent
    When I evaluate "oppervlakte_bijgebouwen_toegestaan" of "omgevingsplan_gemeente_putten"
    Then output "oppervlakte_bijgebouwen_toegestaan" is absent

  Scenario: Next to the plot - the afdeling does not apply
    Given the location is the shape:
      """
      POLYGON((167900 472300, 167910 472300, 167910 472310, 167900 472310, 167900 472300))
      """
    Given parameter "oppervlakte_bijgebouwen_m2" is 250
    Given parameter "sloopmeters_eigen_terrein_m2" is 310
    Given parameter "grootste_bijgebouw_m2" is 180
    When I evaluate "afdeling_21_5_van_toepassing" of "omgevingsplan_gemeente_putten"
    Then output "afdeling_21_5_van_toepassing" is false
    When I evaluate "oppervlakte_bijgebouwen_toegestaan" of "omgevingsplan_gemeente_putten"
    Then output "oppervlakte_bijgebouwen_toegestaan" is absent

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
    When I evaluate "bestemming_wonen_van_toepassing" of "omgevingsplan_gemeente_putten"
    Then output "bestemming_wonen_van_toepassing" is true
    When I evaluate "oppervlakte_bijgebouwen_toegestaan" of "omgevingsplan_gemeente_putten"
    Then output "oppervlakte_bijgebouwen_toegestaan" is true
