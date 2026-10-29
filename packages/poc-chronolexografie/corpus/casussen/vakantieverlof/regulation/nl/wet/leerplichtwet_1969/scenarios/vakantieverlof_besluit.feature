Feature: Het besluit op de aanvraag om vakantieverlof
  Het hoofd verleent verlof op aanvraag (artikel 13a, eerste lid), voor de
  vakantie bedoeld in artikel 11, onder f, voor ten hoogste tien dagen en
  niet in de eerste twee lesweken van het schooljaar, en slechts eenmaal per
  schooljaar (tweede lid). De eerste lesdag komt uit het (fictieve) beleid van
  de school; de twee gegevens van artikel 11, onder f (het beroep van een ouder
  en verlof buiten de schoolvakanties), oordeelt het hoofd. Of het hoofd de
  jongere dit schooljaar al verlof verleende, stelt het hoofd ook vast; in de
  cel van de school koppelt zijn beleid dat open begrip aan het antwoord van
  de cel, uit haar besluiten (RFC-048). Het besluit is een
  beschikking: de Awb vraagt een motivering (3:46) en opent een bezwaartermijn
  van zes weken (6:7).

  Background:
    Given the calculation date is "2026-10-20"
    Given law "algemene_wet_bestuursrecht" is loaded

  Scenario: Vijf schooldagen buiten de eerste twee lesweken worden verleend
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-01 |
      | laatste_verlofdag | 2027-02-05 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
      | verlof_al_verleend_in_schooljaar_volgens_hoofd   | false |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlof_verleend" is true
    Then output "verlofdagen" equals 5
    Then output "verlof_raakt_eerste_twee_lesweken" is false
    Then output "motivering_vereist" is true
    Then output "motivering_vereist" has reactive provenance
    Then output "termijn_weken" equals 6
    Then output "termijn_weken" has reactive provenance

  Scenario: Twee weken met het weekend ertussen zijn tien schooldagen
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-01 |
      | laatste_verlofdag | 2027-02-12 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
      | verlof_al_verleend_in_schooljaar_volgens_hoofd   | false |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlofdagen" equals 10
    Then output "verlof_verleend" is true

  Scenario: Elf schooldagen worden geweigerd
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-01 |
      | laatste_verlofdag | 2027-02-15 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlofdagen" equals 11
    Then output "verlof_verleend" is false

  Scenario: Verlof in de tweede lesweek wordt geweigerd
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2026-09-10 |
      | laatste_verlofdag | 2026-09-11 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlof_raakt_eerste_twee_lesweken" is true
    Then output "verlof_verleend" is false

  Scenario: Zonder een beroep van specifieke aard wordt verlof geweigerd
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-01 |
      | laatste_verlofdag | 2027-02-05 |
      | specifieke_aard_beroep_ouder       | false |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlof_verleend" is false

  Scenario: Verlof binnen de schoolvakanties wordt geweigerd
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-01 |
      | laatste_verlofdag | 2027-02-05 |
      | specifieke_aard_beroep_ouder       | true  |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | false |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlof_verleend" is false

  Scenario: Verlof voor alleen een weekend wordt geweigerd
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-06 |
      | laatste_verlofdag | 2027-02-07 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlofdagen" equals 0
    Then output "verlof_verleend" is false

  Scenario: Verlof met de laatste verlofdag vóór de eerste wordt geweigerd
    Given the following parameters:
      | bsn_jongere       | 999990019  |
      | eerste_verlofdag  | 2027-02-05 |
      | laatste_verlofdag | 2027-02-01 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlof_verleend" is false

  Scenario: Een tweede verlof in hetzelfde schooljaar wordt geweigerd
    # Het hoofd verleende deze jongere dit schooljaar al verlof. In de cel
    # van de school geeft de cel dat antwoord (RFC-048); hier geeft het
    # scenario de vaststelling van het hoofd, op de id van het begrip.
    Given the following parameters:
      | bsn_jongere       | 999990020  |
      | eerste_verlofdag  | 2027-02-01 |
      | laatste_verlofdag | 2027-02-05 |
      | specifieke_aard_beroep_ouder       | true |
    Given the following findings:
      | verlof_valt_buiten_schoolvakanties_volgens_hoofd | true |
      | verlof_al_verleend_in_schooljaar_volgens_hoofd   | true |
    When I evaluate "verlof_verleend" of "leerplichtwet_1969"
    Then the execution succeeds
    Then output "verlofdagen" equals 5
    Then output "verlof_raakt_eerste_twee_lesweken" is false
    Then output "verlof_verleend" is false
