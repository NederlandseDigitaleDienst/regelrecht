# Converted from burgerlijk_wetboek/burgerlijk_wetboek_beschermingsbewind_RECHTSPRAAK-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Beschermingsbewind (BW 1:431-449)
  Als Rechtspraak
  Wil ik bewind-registraties beheren
  Zodat bewindvoerders hun rechthebbenden kunnen vertegenwoordigen in financiele zaken

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Bewindvoerder heeft actief volledig bewind voor rechthebbende
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                      |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500002 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Bart Willems (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BEWINDVOERDER (array membership not expressible in the canonical grammar)
    # POC: output "valid_from_dates" contains 2023-03-01 (array membership not expressible in the canonical grammar)

  Scenario: Bewindvoerder met meerdere rechthebbenden
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                                                                                                                                                                                                        |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":null,"status":"ACTIEF"},{"bsn_rechthebbende":"999500003","naam_rechthebbende":"Maria Hulpbehoevend","bewind_type":"GEDEELTELIJK_BEWIND","datum_ingang":"2024-01-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                                                                                                                                                                                                         |
      | "999500003" | []                                                                                                                                                                                                                                                                                                                                                         |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500002 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 999500003 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Bart Willems (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Maria Hulpbehoevend (array membership not expressible in the canonical grammar)

  Scenario: Persoon is geen bewindvoerder - geen registraties
    Given parameter "bsn" is "999600001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                      |
      | "999600001" | []                                                                                                                                                                       |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Bewind is beeindigd door datum_einde in het verleden
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                              |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":"2024-12-31","status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Bewind is beeindigd door status BEEINDIGD
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                         |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":null,"status":"BEEINDIGD"}] |
      | "999500002" | []                                                                                                                                                                          |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Bewind met toekomstige einddatum is nog actief
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                              |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":"2026-12-31","status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500002 (array membership not expressible in the canonical grammar)

  Scenario: Gedeeltelijk bewind - beperkt tot specifieke goederen
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                         |
      | "999400001" | [{"bsn_rechthebbende":"999500004","naam_rechthebbende":"Jan Beperkt","bewind_type":"GEDEELTELIJK_BEWIND","datum_ingang":"2024-06-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500004" | []                                                                                                                                                                          |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500004 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BEWINDVOERDER (array membership not expressible in the canonical grammar)

  Scenario: Combinatie van actief en beeindigd bewind
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                                                                                                                                                                                                           |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2020-01-01","datum_einde":"2022-12-31","status":"BEEINDIGD"},{"bsn_rechthebbende":"999500005","naam_rechthebbende":"Anna Actief","bewind_type":"GEDEELTELIJK_BEWIND","datum_ingang":"2024-01-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                                                                                                                                                                                                            |
      | "999500005" | []                                                                                                                                                                                                                                                                                                                                                            |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500005 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 999500002 (negative membership not expressible in the canonical grammar)

  Scenario: Bewindvoerder krijgt volledige rechten voor financiele zaken (Art. 1:441 BW)
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                      |
      | "999400001" | [{"bsn_rechthebbende":"999500002","naam_rechthebbende":"Bart Willems","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-03-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500002" | []                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true

  Scenario: Bewind ingegaan voor peildatum is actief
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                      |
      | "999400001" | [{"bsn_rechthebbende":"999500006","naam_rechthebbende":"Kees Vandaag","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2024-06-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500006" | []                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500006 (array membership not expressible in the canonical grammar)

  Scenario: Bewind met einddatum na peildatum is nog actief
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_beschermingsbewind":
      | bsn         | bewind_registraties                                                                                                                                                              |
      | "999400001" | [{"bsn_rechthebbende":"999500007","naam_rechthebbende":"Lisa Vandaag","bewind_type":"VOLLEDIG_BEWIND","datum_ingang":"2023-01-01","datum_einde":"2026-06-01","status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_beschermingsbewind"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
