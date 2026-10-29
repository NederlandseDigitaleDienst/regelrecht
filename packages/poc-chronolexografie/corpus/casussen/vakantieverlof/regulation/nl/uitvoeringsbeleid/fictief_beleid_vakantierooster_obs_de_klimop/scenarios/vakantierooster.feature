Feature: Het vakantierooster van OBS De Klimop
  Het bevoegd gezag stelt per schooljaar een herfst- en een
  voorjaarsvakantie vast, als regel van de school (fictief beleid). De
  versie van 1 augustus 2026 kent alleen het schooljaar 2026–2027.

  Background:
    Given the calculation date is "2026-08-01"

  Scenario: Het schooljaar 2026–2027 heeft herfst- en voorjaarsvakantie
    Given the following parameters:
      | eerste_jaar_schooljaar | 2026 |
    When I evaluate "herfstvakantie_eerste_dag" of "fictief_beleid_vakantierooster_obs_de_klimop"
    Then the execution succeeds
    Then output "herfstvakantie_eerste_dag" equals "2026-10-17"
    When I evaluate "herfstvakantie_laatste_dag" of "fictief_beleid_vakantierooster_obs_de_klimop"
    Then output "herfstvakantie_laatste_dag" equals "2026-10-25"
    When I evaluate "voorjaarsvakantie_eerste_dag" of "fictief_beleid_vakantierooster_obs_de_klimop"
    Then output "voorjaarsvakantie_eerste_dag" equals "2027-02-20"
    When I evaluate "voorjaarsvakantie_laatste_dag" of "fictief_beleid_vakantierooster_obs_de_klimop"
    Then output "voorjaarsvakantie_laatste_dag" equals "2027-02-28"

  Scenario: Voor een ander schooljaar zegt deze versie niets
    Given the following parameters:
      | eerste_jaar_schooljaar | 2027 |
    When I evaluate "herfstvakantie_eerste_dag" of "fictief_beleid_vakantierooster_obs_de_klimop"
    Then the execution succeeds
    Then output "herfstvakantie_eerste_dag" is absent
    When I evaluate "voorjaarsvakantie_laatste_dag" of "fictief_beleid_vakantierooster_obs_de_klimop"
    Then output "voorjaarsvakantie_laatste_dag" is absent
