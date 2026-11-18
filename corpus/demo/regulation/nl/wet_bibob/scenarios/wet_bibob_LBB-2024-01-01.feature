# Converted from bestuursrecht/wet_bibob_LBB-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bibob-advies Landelijk Bureau Bibob
  Als bestuursorgaan
  Wil ik een Bibob-advies opvragen bij het LBB
  Zodat ik een integriteitsbeoordeling kan meewegen bij mijn besluit over een vergunning

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Geen Bibob-advies aangevraagd - verlening niet belemmerd
    # POC: no Bibob advice on record for this enterprise (no bibob_adviezen row applies)
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | false              | null                   | null         | false                        | false               | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, verlening_geadviseerd, weigering_mogelijk, voorschriften_mogelijk" of "wet_bibob"
    Then output "advies_uitgebracht" is false
    And output "verlening_geadviseerd" is true
    And output "weigering_mogelijk" is false
    And output "voorschriften_mogelijk" is false

  Scenario: Bibob-advies uitgebracht - geen gevaar gebleken
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | true               | geen_gevaar            | 2024-05-15   | false                        | false               | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, verlening_geadviseerd, weigering_mogelijk, voorschriften_mogelijk" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "geen_gevaar"
    And output "verlening_geadviseerd" is true
    And output "weigering_mogelijk" is false
    And output "voorschriften_mogelijk" is false

  Scenario: Bibob-advies uitgebracht - mindere mate van gevaar
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | true               | mindere_mate           | 2024-05-15   | true                         | false               | true                      |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, verlening_geadviseerd, weigering_mogelijk, voorschriften_mogelijk" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "mindere_mate"
    And output "verlening_geadviseerd" is false
    And output "weigering_mogelijk" is false
    And output "voorschriften_mogelijk" is true

  Scenario: Bibob-advies uitgebracht - ernstig gevaar (weigering mogelijk)
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | true               | ernstig_gevaar         | 2024-05-15   | true                         | true                | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, verlening_geadviseerd, weigering_mogelijk, voorschriften_mogelijk" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "ernstig_gevaar"
    And output "verlening_geadviseerd" is false
    And output "weigering_mogelijk" is true
    And output "voorschriften_mogelijk" is false

  Scenario: Bibob-advies uitgebracht - ernstig gevaar maar weigering niet proportioneel
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | true               | ernstig_gevaar         | 2024-05-15   | true                         | false               | true                      |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, weigering_mogelijk, voorschriften_mogelijk" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "ernstig_gevaar"
    And output "weigering_mogelijk" is true
    And output "voorschriften_mogelijk" is true

  Scenario: Bibob-advies voor natuurlijk persoon (exploitant)
    Given parameter "kvk_nummer" is "99999990"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "99999990" | true               | geen_gevaar            | 2024-05-15   | false                        | false               | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, verlening_geadviseerd" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "geen_gevaar"
    And output "verlening_geadviseerd" is true

  Scenario: Bibob-advies met financieringsrisico (witwassen)
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | true               | ernstig_gevaar         | 2024-05-15   | false                        | true                | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, weigering_mogelijk, financieringsrisico" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "ernstig_gevaar"
    And output "weigering_mogelijk" is true
    And output "financieringsrisico" is true

  Scenario: Bibob-advies met relatie tot strafbare feiten
    Given parameter "kvk_nummer" is "12345678"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "12345678" | true               | ernstig_gevaar         | 2024-05-15   | true                         | false               | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, weigering_mogelijk, relatie_strafbare_feiten" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "ernstig_gevaar"
    And output "weigering_mogelijk" is true
    And output "relatie_strafbare_feiten" is true

  Scenario: Bibob-advies voor horecavergunning
    Given parameter "kvk_nummer" is "85234567"
    And parameter "aanvraag_type" is "vergunning"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | true               | geen_gevaar            | 2024-05-20   | false                        | false               | false                     |
    And the following parameters:
      | beschikking_type | vergunning |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, verlening_geadviseerd" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "geen_gevaar"
    And output "verlening_geadviseerd" is true

  Scenario: Bibob-advies voor vastgoedtransactie
    Given parameter "kvk_nummer" is "98765432"
    And parameter "aanvraag_type" is "vastgoedtransactie"
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "98765432" | true               | mindere_mate           | 2024-04-10   | false                        | true                | true                      |
    And the following parameters:
      | beschikking_type | vastgoedtransactie |
    # POC: parameter "bsn" not provided by the scenario (None in the POC)
    And parameter "bsn" is "null"
    When I evaluate outputs "advies_uitgebracht, mate_van_gevaar, voorschriften_mogelijk" of "wet_bibob"
    Then output "advies_uitgebracht" is true
    And output "mate_van_gevaar" equals "mindere_mate"
    And output "voorschriften_mogelijk" is true
