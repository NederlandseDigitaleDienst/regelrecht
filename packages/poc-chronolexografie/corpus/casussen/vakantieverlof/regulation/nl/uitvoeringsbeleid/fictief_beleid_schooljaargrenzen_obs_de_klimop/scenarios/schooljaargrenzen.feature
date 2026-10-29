Feature: Begin en einde van het schooljaar bij OBS De Klimop
  Het fictieve beleid van de school zegt in welk schooljaar een dag valt: een
  schooljaar loopt van 1 augustus tot en met 31 juli van het jaar erna. Het
  beleid van het hoofd vraagt het voor de eerste verlofdag, om te weten of
  het hoofd de jongere in dat schooljaar al verlof verleende (artikel 13a,
  tweede lid, van de Leerplichtwet 1969: slechts eenmaal per schooljaar).

  Background:
    Given the calculation date is "2027-03-01"

  Scenario: Een dag in het voorjaar hoort bij het schooljaar dat in augustus ervoor begon
    Given the following parameters:
      | datum | 2027-05-10 |
    When I evaluate "einde_schooljaar" of "fictief_beleid_schooljaargrenzen_obs_de_klimop"
    Then output "begin_schooljaar" equals "2026-08-01"
    Then output "einde_schooljaar" equals "2027-07-31"

  Scenario: 31 juli is de laatste dag van het schooljaar
    Given the following parameters:
      | datum | 2027-07-31 |
    When I evaluate "einde_schooljaar" of "fictief_beleid_schooljaargrenzen_obs_de_klimop"
    Then output "begin_schooljaar" equals "2026-08-01"
    Then output "einde_schooljaar" equals "2027-07-31"

  Scenario: 1 augustus is de eerste dag van het nieuwe schooljaar
    Given the following parameters:
      | datum | 2027-08-01 |
    When I evaluate "einde_schooljaar" of "fictief_beleid_schooljaargrenzen_obs_de_klimop"
    Then output "begin_schooljaar" equals "2027-08-01"
    Then output "einde_schooljaar" equals "2028-07-31"

  Scenario: Het schooljaar 2026–2027 begint in 2026 en eindigt in 2027
    Given the following parameters:
      | datum | 2027-02-01 |
    When I evaluate "eerste_jaar_schooljaar" of "fictief_beleid_schooljaargrenzen_obs_de_klimop"
    Then output "eerste_jaar_schooljaar" equals 2026
    When I evaluate "laatste_jaar_schooljaar" of "fictief_beleid_schooljaargrenzen_obs_de_klimop"
    Then output "laatste_jaar_schooljaar" equals 2027
