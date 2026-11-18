# Converted from overig/precariobelasting_GEMEENTE_ROTTERDAM-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen precariobelasting voor terras op openbare grond Rotterdam
  Als horecaondernemer in Rotterdam
  Wil ik weten of en hoeveel precariobelasting ik verschuldigd ben
  Zodat ik mijn kosten voor een terras op gemeentegrond kan bepalen

  Background:
    Given the calculation date is "2024-06-01"
    And parameter "kvk_nummer" is "85234567"

  Scenario: Belastingplichtig - terras 80 m² (boven vrijstellingsdrempel)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie | horeca_toegestaan |
      | "85234567" | null         | null              | null            | null              | null                 | null                    | null                            | []         | null            | null                  | null                | null              | null                        | null      | null              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied | tarief_per_m2 |
      | "85234567" | null                    |                        | 25            |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "verordening_precariobelasting":
      | kvk_nummer | heeft_terras | is_openbare_weg |
      | "85234567" | true         | true            |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject |
      | Witte de Withstraat 1 | null            |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following parameters:
      | heeft_actieve_vergunning | true |
      | vergunde_oppervlakte     | 80   |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_belastingplichtig, heffingsmaatstaf_m2, tarief_per_m2, belasting_per_jaar" of "verordening_precariobelasting"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_belastingplichtig" is true
    And output "heffingsmaatstaf_m2" equals 30
    And output "tarief_per_m2" equals 3610
    And output "belasting_per_jaar" equals 108300

  Scenario: Belastingplichtig - terras precies 50 m² (op vrijstellingsdrempel)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie | horeca_toegestaan |
      | "85234567" | null         | null              | null            | null              | null                 | null                    | null                            | []         | null            | null                  | null                | null              | null                        | null      | null              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied | tarief_per_m2 |
      | "85234567" | null                    |                        | 25            |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "verordening_precariobelasting":
      | kvk_nummer | heeft_terras | is_openbare_weg |
      | "85234567" | true         | true            |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject |
      | Witte de Withstraat 1 | null            |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following parameters:
      | heeft_actieve_vergunning | true |
      | vergunde_oppervlakte     | 50   |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_belastingplichtig, heffingsmaatstaf_m2, belasting_per_jaar" of "verordening_precariobelasting"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_belastingplichtig" is true
    And output "heffingsmaatstaf_m2" equals 0
    And output "belasting_per_jaar" equals 0

  Scenario: Belastingplichtig - klein terras 20 m² (onder vrijstellingsdrempel)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie | horeca_toegestaan |
      | "85234567" | null         | null              | null            | null              | null                 | null                    | null                            | []         | null            | null                  | null                | null              | null                        | null      | null              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied | tarief_per_m2 |
      | "85234567" | null                    |                        | 25            |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "verordening_precariobelasting":
      | kvk_nummer | heeft_terras | is_openbare_weg |
      | "85234567" | true         | true            |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject |
      | Witte de Withstraat 1 | null            |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following parameters:
      | heeft_actieve_vergunning | true |
      | vergunde_oppervlakte     | 20   |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_belastingplichtig, heffingsmaatstaf_m2, belasting_per_jaar" of "verordening_precariobelasting"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_belastingplichtig" is true
    And output "heffingsmaatstaf_m2" equals 0
    And output "belasting_per_jaar" equals 0

  Scenario: Niet belastingplichtig - geen terras
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie | horeca_toegestaan |
      | "85234567" | null         | null              | null            | null              | null                 | null                    | null                            | []         | null            | null                  | null                | null              | null                        | null      | null              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied | tarief_per_m2 |
      | "85234567" | null                    |                        | 25            |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "verordening_precariobelasting":
      | kvk_nummer | heeft_terras | is_openbare_weg |
      | "85234567" | false        | true            |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject |
      | Witte de Withstraat 1 | null            |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following parameters:
      | heeft_actieve_vergunning | true |
      | vergunde_oppervlakte     | 10   |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "verordening_precariobelasting"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Niet belastingplichtig - geen actieve terrasvergunning
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie | horeca_toegestaan |
      | "85234567" | null         | null              | null            | null              | null                 | null                    | null                            | []         | null            | null                  | null                | null              | null                        | null      | null              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied | tarief_per_m2 |
      | "85234567" | null                    |                        | 25            |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "verordening_precariobelasting":
      | kvk_nummer | heeft_terras | is_openbare_weg |
      | "85234567" | true         | true            |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject |
      | Witte de Withstraat 1 | null            |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following parameters:
      | heeft_actieve_vergunning | false |
      | vergunde_oppervlakte     | 10    |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "verordening_precariobelasting"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Niet belastingplichtig - locatie niet op openbare weg
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie | horeca_toegestaan |
      | "85234567" | null         | null              | null            | null              | null                 | null                    | null                            | []         | null            | null                  | null                | null              | null                        | null      | null              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied | tarief_per_m2 |
      | "85234567" | null                    |                        | 25            |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "verordening_precariobelasting":
      | kvk_nummer | heeft_terras | is_openbare_weg |
      | "85234567" | true         | false           |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject |
      | Witte de Withstraat 1 | null            |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following parameters:
      | heeft_actieve_vergunning | true |
      | vergunde_oppervlakte     | 10   |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "verordening_precariobelasting"
    Then output "voldoet_aan_voorwaarden" is false
