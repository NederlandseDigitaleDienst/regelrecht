# Converted from overig/omgevingswet_werkgebonden_personenmobiliteit_RVO-2024-07-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: WPM Rapportageverplichting
  Als werkgever
  Wil ik weten of ik verplicht ben om te rapporteren over werkgebonden personenmobiliteit
  Zodat ik voldoe aan de wettelijke verplichtingen volgens de Omgevingswet

  Background:
    Given the calculation date is "2024-07-01"

  Scenario: Organisatie met 100 werknemers is verplicht te rapporteren
    Given parameter "kvk_nummer" is "12345678"
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit":
      | kvk_nummer | aantal_werknemers | verstrekt_mobiliteitsvergoeding |
      | "12345678" | 100               | true                            |
    When I evaluate outputs "voldoet_aan_voorwaarden, rapportageverplichting, aantal_werknemers" of "omgevingswet/werkgebonden_personenmobiliteit"
    Then output "voldoet_aan_voorwaarden" is true
    And output "rapportageverplichting" is true
    And output "aantal_werknemers" equals 100

  Scenario: Organisatie met 99 werknemers is niet verplicht te rapporteren
    Given parameter "kvk_nummer" is "87654321"
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit":
      | kvk_nummer | aantal_werknemers | verstrekt_mobiliteitsvergoeding |
      | "87654321" | 99                | true                            |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "omgevingswet/werkgebonden_personenmobiliteit"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Organisatie zonder mobiliteitsvergoeding hoeft niet te rapporteren
    Given parameter "kvk_nummer" is "44444444"
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit":
      | kvk_nummer | aantal_werknemers | verstrekt_mobiliteitsvergoeding |
      | "44444444" | 150               | false                           |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "omgevingswet/werkgebonden_personenmobiliteit"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Organisatie rapporteert reisgegevens en CO2-uitstoot
    Given parameter "kvk_nummer" is "55555555"
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit":
      | kvk_nummer | aantal_werknemers | verstrekt_mobiliteitsvergoeding |
      | "55555555" | 120               | true                            |
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit/gegevens":
      | kvk_nummer | woon_werk_auto_benzine | woon_werk_auto_diesel | zakelijk_auto_benzine | zakelijk_auto_diesel | woon_werk_openbaar_vervoer |
      | "55555555" | 10000                  | 5000                  | 3000                  | 2000                 | 8000                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, rapportageverplichting" of "omgevingswet/werkgebonden_personenmobiliteit"
    Then output "voldoet_aan_voorwaarden" is true
    And output "rapportageverplichting" is true
    When I evaluate outputs "woon_werk_auto_benzine, woon_werk_auto_diesel, zakelijk_auto_benzine, zakelijk_auto_diesel, woon_werk_openbaar_vervoer, co2_uitstoot_totaal" of "omgevingswet/werkgebonden_personenmobiliteit/gegevens"
    Then output "woon_werk_auto_benzine" equals 10000
    And output "woon_werk_auto_diesel" equals 5000
    And output "zakelijk_auto_benzine" equals 3000
    And output "zakelijk_auto_diesel" equals 2000
    And output "woon_werk_openbaar_vervoer" equals 8000
    And output "co2_uitstoot_totaal" equals 3500000

  Scenario: Emissie per reizigerskilometer wordt berekend per mobiliteitssoort
    Given parameter "kvk_nummer" is "66666666"
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit/gegevens":
      | kvk_nummer | woon_werk_auto_benzine | woon_werk_auto_diesel | zakelijk_auto_benzine | zakelijk_auto_diesel | woon_werk_openbaar_vervoer |
      | "66666666" | 1000                   | 0                     | 0                     | 1000                 | 0                          |
    When I evaluate outputs "emissie_per_reizigerskilometer_woon_werk, emissie_per_reizigerskilometer_zakelijk" of "omgevingswet/werkgebonden_personenmobiliteit/gegevens"
    Then output "emissie_per_reizigerskilometer_woon_werk" equals 170
    And output "emissie_per_reizigerskilometer_zakelijk" equals 150

  Scenario: Emissie per reizigerskilometer is nul zonder gereisde kilometers
    Given parameter "kvk_nummer" is "77777777"
    And the following "RVO" data with key "kvk_nummer" for law "omgevingswet/werkgebonden_personenmobiliteit/gegevens":
      | kvk_nummer | woon_werk_auto_benzine | woon_werk_auto_diesel | zakelijk_auto_benzine | zakelijk_auto_diesel | woon_werk_openbaar_vervoer |
      | "77777777" | 0                      | 0                     | 0                     | 0                    | 0                          |
    When I evaluate outputs "emissie_per_reizigerskilometer_woon_werk, emissie_per_reizigerskilometer_zakelijk" of "omgevingswet/werkgebonden_personenmobiliteit/gegevens"
    Then output "emissie_per_reizigerskilometer_woon_werk" equals 0
    And output "emissie_per_reizigerskilometer_zakelijk" equals 0
