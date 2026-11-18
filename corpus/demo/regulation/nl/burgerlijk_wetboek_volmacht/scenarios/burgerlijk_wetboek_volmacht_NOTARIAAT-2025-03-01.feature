# Converted from burgerlijk_wetboek/burgerlijk_wetboek_volmacht_NOTARIAAT-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Volmacht (BW 3:60-79)
  Als Notariaat
  Wil ik volmacht-registraties beheren
  Zodat gevolmachtigden namens volmachtgevers kunnen handelen

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Gevolmachtigde heeft actieve algemene volmacht
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                            |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500004 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Elisabeth van den Berg-Smit (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains GEVOLMACHTIGDE_ALGEMEEN (array membership not expressible in the canonical grammar)

  Scenario: Algemene volmacht geeft volledige rechten (LEZEN, CLAIMS_INDIENEN, BESLUITEN_ONTVANGEN)
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                            |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true

  Scenario: Gevolmachtigde heeft actieve bijzondere volmacht
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                              |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"BIJZONDERE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                                 |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains GEVOLMACHTIGDE_BIJZONDER (array membership not expressible in the canonical grammar)

  Scenario: Bijzondere volmacht geeft beperkte rechten (alleen LEZEN)
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                              |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"BIJZONDERE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                                 |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "delegation_types" contains GEVOLMACHTIGDE_BIJZONDER (array membership not expressible in the canonical grammar)

  Scenario: Persoon zonder volmacht-registratie is geen gevolmachtigde
    Given parameter "bsn" is "123456789"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                            |
      | "123456789" | []                                                                                                                                                                                                               |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Herroepen volmacht geeft geen bevoegdheid
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                           |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":true}] |
      | "999500004" | []                                                                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Inactieve volmacht geeft geen bevoegdheid
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                              |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"INACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                                 |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false

  Scenario: Verlopen volmacht geeft geen bevoegdheid
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                                    |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":"2024-12-31","status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false

  Scenario: Volmacht met toekomstige einddatum is nog actief
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                                    |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":"2026-12-31","status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true

  Scenario: Gevolmachtigde met meerdere volmachten van verschillende volmachtgevers
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                                                                                                                                                                                                                              |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false},{"bsn_volmachtgever":"999600006","naam_volmachtgever":"Jan de Vries","volmacht_type":"BIJZONDERE_VOLMACHT","datum_ingang":"2022-01-15","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                                                                                                                                                                                                                                 |
      | "999600006" | []                                                                                                                                                                                                                                                                                                                                                                                                                 |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500004 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 999600006 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains GEVOLMACHTIGDE_ALGEMEEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains GEVOLMACHTIGDE_BIJZONDER (array membership not expressible in the canonical grammar)

  Scenario: Gevolmachtigde met een actieve en een herroepen volmacht
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                                                                                                                                                                                                                           |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false},{"bsn_volmachtgever":"999600006","naam_volmachtgever":"Jan de Vries","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2020-01-15","datum_einde":null,"status":"ACTIEF","is_herroepen":true}] |
      | "999500004" | []                                                                                                                                                                                                                                                                                                                                                                                                              |
      | "999600006" | []                                                                                                                                                                                                                                                                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500004 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 999600006 (negative membership not expressible in the canonical grammar)

  Scenario: Volmacht geldig vanaf ingangsdatum wordt correct geregistreerd
    Given parameter "bsn" is "999400002"
    And the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_volmacht":
      | bsn         | volmacht_registraties                                                                                                                                                                                            |
      | "999400002" | [{"bsn_volmachtgever":"999500004","naam_volmachtgever":"Elisabeth van den Berg-Smit","volmacht_type":"ALGEMENE_VOLMACHT","datum_ingang":"2021-05-01","datum_einde":null,"status":"ACTIEF","is_herroepen":false}] |
      | "999500004" | []                                                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "burgerlijk_wetboek_volmacht"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "valid_from_dates" contains 2021-05-01 (array membership not expressible in the canonical grammar)
