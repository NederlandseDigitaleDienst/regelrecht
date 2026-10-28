# Converted from overig/wet_brp_laa_RvIG-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Landelijke Aanpak Adreskwaliteit (LAA)
  Als RvIG
  Wil ik signalen genereren over mogelijk onjuiste adresregistraties
  Zodat gemeenten de adreskwaliteit in de BRP kunnen verbeteren

  Background:
    Given the calculation date is "2025-03-01"

  # Alleen de genoemde bron twijfelt: Toeslagen kent het BRP-adres en CJIB meldt
  # geen onbestelbare post, dus het signaal kan alleen van de Belastingdienst komen.
  Scenario: Belastingdienst meldt twijfel over adres - genereert signaal type MELDING
    Given parameter "bsn" is "999993653"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                   | verblijfsobject |
      | {"postcode":"1234AB","huisnummer":"10"} | null            |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                   | aantal_bewoners |
      | {"postcode":"1234AB","huisnummer":"10"} | 0               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres                        | langdurig_afwezig |
      | 999993653 | {"postcode":"9999ZZ","huisnummer":"1"} | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993653 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres                           | toeslagen_onderzoek |
      | 999993653 | {"postcode":"1234AB","huisnummer":"10"} | null                |
    And the following parameters:
      | adres | {"postcode":"1234AB","huisnummer":"10"} |
    When I evaluate outputs "genereer_signaal, signaal_type, reactietermijn_weken, onderzoekstermijn_maanden" of "wet_brp/laa"
    Then output "genereer_signaal" is true
    And output "signaal_type" equals "MELDING"
    And output "reactietermijn_weken" equals 4
    And output "onderzoekstermijn_maanden" equals 6

  Scenario: Toeslagen meldt twijfel over adres - genereert signaal type MELDING
    Given parameter "bsn" is "999993654"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                   | verblijfsobject |
      | {"postcode":"5678CD","huisnummer":"25"} | null            |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                   | aantal_bewoners |
      | {"postcode":"5678CD","huisnummer":"25"} | 0               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres | langdurig_afwezig |
      | 999993654 | null            | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993654 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres | toeslagen_onderzoek |
      | 999993654 | null          | null                |
    And the following parameters:
      | adres | {"postcode":"5678CD","huisnummer":"25"} |
    When I evaluate outputs "genereer_signaal, signaal_type" of "wet_brp/laa"
    Then output "genereer_signaal" is true
    And output "signaal_type" equals "MELDING"

  Scenario: CJIB meldt twijfel over adres - genereert signaal type MELDING
    Given parameter "bsn" is "999993655"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                   | verblijfsobject |
      | {"postcode":"9012EF","huisnummer":"42"} | null            |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                   | aantal_bewoners |
      | {"postcode":"9012EF","huisnummer":"42"} | 0               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres | langdurig_afwezig |
      | 999993655 | null            | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993655 | true                | 1                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres                           | toeslagen_onderzoek |
      | 999993655 | {"postcode":"9012EF","huisnummer":"42"} | null                |
    And the following parameters:
      | adres | {"postcode":"9012EF","huisnummer":"42"} |
    When I evaluate outputs "genereer_signaal, signaal_type" of "wet_brp/laa"
    Then output "genereer_signaal" is true
    And output "signaal_type" equals "MELDING"

  Scenario: Profiel "Overbewoning" - hoog aantal bewoners op adres zonder woonfunctie
    Given parameter "bsn" is "999993657"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                    | verblijfsobject                                                                 |
      | {"postcode":"3456GH","huisnummer":"100"} | {"gebruiksdoel":"kantoorfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                    | aantal_bewoners |
      | {"postcode":"3456GH","huisnummer":"100"} | 8               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres                                                          | langdurig_afwezig |
      | 999993657 | {"postcode":"3456GH","huisnummer":"100","straat":null,"woonplaats":null} | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993657 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres                                                            | toeslagen_onderzoek |
      | 999993657 | {"postcode":"3456GH","huisnummer":"100","straat":null,"woonplaats":null} | null                |
    And the following parameters:
      | adres | {"postcode":"3456GH","huisnummer":"100"} |
    When I evaluate outputs "genereer_signaal, signaal_type, reactietermijn_weken, onderzoekstermijn_maanden" of "wet_brp/laa"
    Then output "genereer_signaal" is true
    And output "signaal_type" equals "PROFIEL"
    And output "reactietermijn_weken" equals 4
    And output "onderzoekstermijn_maanden" equals 6

  Scenario: Normaal adres zonder meldingen of profielen - geen signaal
    Given parameter "bsn" is "999993658"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                   | verblijfsobject |
      | {"postcode":"7890IJ","huisnummer":"15"} | null            |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                   | aantal_bewoners |
      | {"postcode":"7890IJ","huisnummer":"15"} | 3               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres                                                         | langdurig_afwezig |
      | 999993658 | {"postcode":"7890IJ","huisnummer":"15","straat":null,"woonplaats":null} | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993658 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres                                                           | toeslagen_onderzoek |
      | 999993658 | {"postcode":"7890IJ","huisnummer":"15","straat":null,"woonplaats":null} | null                |
    And the following parameters:
      | adres | {"postcode":"7890IJ","huisnummer":"15"} |
    When I evaluate outputs "genereer_signaal" of "wet_brp/laa"
    Then output "genereer_signaal" is false

  Scenario: Hoog aantal bewoners maar met woonfunctie - geen signaal
    Given parameter "bsn" is "999993659"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                    | verblijfsobject                                                              |
      | {"postcode":"2345KL","huisnummer":"200"} | {"gebruiksdoel":"woonfunctie","oppervlakte":0,"status":null,"bouwjaar":null} |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                    | aantal_bewoners |
      | {"postcode":"2345KL","huisnummer":"200"} | 8               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres                                                          | langdurig_afwezig |
      | 999993659 | {"postcode":"2345KL","huisnummer":"200","straat":null,"woonplaats":null} | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993659 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres                                                            | toeslagen_onderzoek |
      | 999993659 | {"postcode":"2345KL","huisnummer":"200","straat":null,"woonplaats":null} | null                |
    And the following parameters:
      | adres | {"postcode":"2345KL","huisnummer":"200"} |
    When I evaluate outputs "genereer_signaal" of "wet_brp/laa"
    Then output "genereer_signaal" is false

  Scenario: Adres zonder woonfunctie maar met laag aantal bewoners - geen signaal
    Given parameter "bsn" is "999993660"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                   | verblijfsobject |
      | {"postcode":"6789MN","huisnummer":"50"} | null            |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                   | aantal_bewoners |
      | {"postcode":"6789MN","huisnummer":"50"} | 2               |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres                                                         | langdurig_afwezig |
      | 999993660 | {"postcode":"6789MN","huisnummer":"50","straat":null,"woonplaats":null} | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993660 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres                                                           | toeslagen_onderzoek |
      | 999993660 | {"postcode":"6789MN","huisnummer":"50","straat":null,"woonplaats":null} | null                |
    And the following parameters:
      | adres | {"postcode":"6789MN","huisnummer":"50"} |
    When I evaluate outputs "genereer_signaal" of "wet_brp/laa"
    Then output "genereer_signaal" is false

  Scenario: Combinatie van melding en profiel - signaal type blijft MELDING
    Given parameter "bsn" is "999993656"
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                   | verblijfsobject |
      | {"postcode":"4567OP","huisnummer":"75"} | null            |
    And the following "RvIG" data with key "adres" for law "wet_brp/laa":
      | adres                                   | aantal_bewoners |
      | {"postcode":"4567OP","huisnummer":"75"} | 10              |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_brp/terugmelding/belastingdienst":
      | bsn       | belasting_adres | langdurig_afwezig |
      | 999993656 | null            | null              |
    And the following "CJIB" data with key "bsn" for law "wet_brp/terugmelding/cjib":
      | bsn       | onbestelbaar_retour | aantal_onbestelbaar |
      | 999993656 | null                | 0                   |
    And the following "TOESLAGEN" data with key "bsn" for law "wet_brp/terugmelding/toeslagen":
      | bsn       | toeslag_adres | toeslagen_onderzoek |
      | 999993656 | null          | null                |
    And the following parameters:
      | adres | {"postcode":"4567OP","huisnummer":"75"} |
    When I evaluate outputs "genereer_signaal, signaal_type" of "wet_brp/laa"
    Then output "genereer_signaal" is true
    And output "signaal_type" equals "MELDING"
