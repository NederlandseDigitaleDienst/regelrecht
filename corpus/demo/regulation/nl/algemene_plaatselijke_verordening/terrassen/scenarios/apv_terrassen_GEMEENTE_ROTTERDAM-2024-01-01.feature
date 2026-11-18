# Converted from overig/apv_terrassen_GEMEENTE_ROTTERDAM-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen recht op Terrasvergunning horeca Rotterdam
  Als horecaondernemer in Rotterdam
  Wil ik weten of ik een terrasvergunning kan krijgen
  Zodat ik een terras mag exploiteren bij mijn horecabedrijf

  Background:
    Given the calculation date is "2024-06-01"
    And parameter "kvk_nummer" is "85234567"

  Scenario: Succesvolle aanvraag - standaard terras voor het pand
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 24                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 2.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning, vergunde_oppervlakte, vergunde_sluitingstijd_doordeweeks, vergunde_sluitingstijd_weekend, precariobelasting_verschuldigd, precariobelasting_per_jaar" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_recht_op_terrasvergunning" is true
    And output "vergunde_oppervlakte" equals 10
    And output "vergunde_sluitingstijd_doordeweeks" equals 23
    And output "vergunde_sluitingstijd_weekend" equals 24
    And output "precariobelasting_verschuldigd" is true
    And output "precariobelasting_per_jaar" equals 0

  Scenario: Succesvolle aanvraag - grenswaarde obstakelvrije ruimte (precies 1.8m)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 24                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 1.8      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_recht_op_terrasvergunning" is true

  Scenario: Afwijzing - geen geldige exploitatievergunning (artikel 2:28)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | false                       | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 24                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 2.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_terrasvergunning" is false

  Scenario: Afwijzing - onvoldoende obstakelvrije ruimte (1.5m < 1.8m minimum)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 24                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 1.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_terrasvergunning" is false

  Scenario: Afwijzing - gevraagde oppervlakte groter dan beschikbare ruimte
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 24                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 20       |
      | obstakelvrije_ruimte               | 2.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_terrasvergunning" is false

  Scenario: Afwijzing - verkeerde BGT-functie oppervlak (rijbaan i.p.v. voetpad)
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 24                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | rijbaan           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 2.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_terrasvergunning" is false

  Scenario: Afwijzing - gewenste sluitingstijd weekend later dan toegestaan
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 24                            | 23                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 2.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 23       |
      | gewenste_sluitingstijd_weekend     | 24       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_terrasvergunning" is false

  Scenario: Afwijzing - gewenste sluitingstijd 01:00 doordeweeks later dan toegestaan (23:00)
    # Sluitingstijden staan op één schaal (24 is middernacht, 25 is 01:00), zodat 01:00 later is dan 23:00.
    Given the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/exploitatievergunning":
      | kvk_nummer | bsn_eigenaar | heeft_geldige_vog | schenkt_alcohol | horecagebiedsplan                                                                           | categorie_toegestaan | vergunning_geschiedenis | ingetrokken_slecht_levensgedrag | beheerders                                                                                        | alle_hebben_vog | alle_voldoen_leeftijd | geen_onder_curatele | heeft_svh_diploma | heeft_exploitatievergunning | categorie   | horeca_toegestaan |
      | "85234567" | "999999990"  | true              | true            | {"gebied":null,"toegestane_categorieen":null,"maximaal_aantal":null,"ontwikkelruimte":null} | true                 | null                    | false                           | [{"bsn":null,"heeft_vog":null,"leeftijd":null,"is_onder_curatele":null,"heeft_svh_diploma":true}] | true            | true                  | true                | true              | true                        | middelzwaar | true              |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "algemene_plaatselijke_verordening/terrassen":
      | kvk_nummer | heeft_alcoholvergunning | terrassenbeleid_gebied                                                                    | tarief_per_m2 |
      | "85234567" | true                    | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "seizoen" for law "algemene_plaatselijke_verordening/terrassen":
      | seizoen  | terrassenbeleid_gebied                                                                                           | max_sluitingstijd_doordeweeks | max_sluitingstijd_weekend | tarief_per_m2 |
      | jaarrond | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 23                            | 23                        | 25            |
    And the following "GEMEENTE_ROTTERDAM" data with key "terras_locatie" for law "algemene_plaatselijke_verordening/terrassen":
      | terras_locatie | beschikbare_oppervlakte | functie_oppervlak | is_openbare_weg | terrassenbeleid_gebied                                                                                           | tarief_per_m2 |
      | voor           | 15                      | voetpad           | true            | {"gebied":null,"max_sluitingstijd":null,"seizoensregels":null,"toegestane_locaties":null} | 25            |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn         | curatele_als_curandus                                                                                                                              |
      | "999999990" | [{"bsn_curator":null,"bsn_curandus":"999999990","naam_curandus":null,"datum_ingang":"2020-01-01","datum_einde":"2021-01-01","status":"BEËINDIGD"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                     |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Witte de Withstraat 1"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                 | verblijfsobject                                                                     |
      | Witte de Withstraat 1 | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | "85234567" | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999999990" | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following parameters:
      | terras_locatie                     | voor     |
      | terras_oppervlakte                 | 10       |
      | obstakelvrije_ruimte               | 2.5      |
      | seizoen                            | jaarrond |
      | gewenste_openingstijd              | 8        |
      | gewenste_sluitingstijd_doordeweeks | 25       |
      | gewenste_sluitingstijd_weekend     | 23       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_terrasvergunning, weigeringsgrond" of "algemene_plaatselijke_verordening/terrassen"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_terrasvergunning" is false
    And output "weigeringsgrond" equals "Gevraagde sluitingstijd doordeweeks (zo-do) later dan toegestaan in dit gebied"
