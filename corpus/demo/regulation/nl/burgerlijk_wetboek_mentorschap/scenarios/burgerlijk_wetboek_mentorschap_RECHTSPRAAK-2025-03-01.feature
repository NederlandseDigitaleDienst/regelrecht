# Converted from burgerlijk_wetboek/burgerlijk_wetboek_mentorschap_RECHTSPRAAK-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Mentorschap (BW 1:450-462)
  Als Rechtspraak
  Wil ik mentorschap-registraties beheren
  Zodat mentors beslissingen kunnen nemen over verzorging, verpleging en behandeling

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Mentor heeft actief mentorschap
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_mentorschap":
      | bsn         | mentorschap_registraties                                                                                                            |
      | "999400001" | [{"bsn_betrokkene":"999500003","naam_betrokkene":"Willem Jansen","datum_ingang":"2023-06-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500003" | []                                                                                                                                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_mentorschap"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500003 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Willem Jansen (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains MENTOR (array membership not expressible in the canonical grammar)
    # POC: output "valid_from_dates" contains 2023-06-15 (array membership not expressible in the canonical grammar)

  Scenario: Persoon is geen mentor
    Given parameter "bsn" is "999999999"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_mentorschap":
      | bsn         | mentorschap_registraties                                                                                                            |
      | "999999999" | []                                                                                                                                  |
      | "999400001" | [{"bsn_betrokkene":"999500003","naam_betrokkene":"Willem Jansen","datum_ingang":"2023-06-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500003" | []                                                                                                                                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids, subject_names" of "burgerlijk_wetboek_mentorschap"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"
    # POC: output is an empty list
    And output "subject_names" equals "[]"

  Scenario: Beeindigd mentorschap geeft geen actieve delegaties
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_mentorschap":
      | bsn         | mentorschap_registraties                                                                                                                       |
      | "999400001" | [{"bsn_betrokkene":"999500003","naam_betrokkene":"Willem Jansen","datum_ingang":"2023-06-15","datum_einde":"2024-12-31","status":"BEEINDIGD"}] |
      | "999500003" | []                                                                                                                                             |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_mentorschap"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Mentor met meerdere betrokkenen
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_mentorschap":
      | bsn         | mentorschap_registraties                                                                                                                                                                                                                                                   |
      | "999400001" | [{"bsn_betrokkene":"999500003","naam_betrokkene":"Willem Jansen","datum_ingang":"2023-06-15","datum_einde":null,"status":"ACTIEF"},{"bsn_betrokkene":"999500004","naam_betrokkene":"Anna Hulpbehoevend","datum_ingang":"2024-01-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500003" | []                                                                                                                                                                                                                                                                         |
      | "999500004" | []                                                                                                                                                                                                                                                                         |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_mentorschap"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500003 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 999500004 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Willem Jansen (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Anna Hulpbehoevend (array membership not expressible in the canonical grammar)

  Scenario: Mentor met mix van actief en beeindigd mentorschap
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_mentorschap":
      | bsn         | mentorschap_registraties                                                                                                                                                                                                                                                              |
      | "999400001" | [{"bsn_betrokkene":"999500003","naam_betrokkene":"Willem Jansen","datum_ingang":"2023-06-15","datum_einde":null,"status":"ACTIEF"},{"bsn_betrokkene":"999500004","naam_betrokkene":"Anna Hulpbehoevend","datum_ingang":"2022-01-01","datum_einde":"2024-06-30","status":"BEEINDIGD"}] |
      | "999500003" | []                                                                                                                                                                                                                                                                                    |
      | "999500004" | []                                                                                                                                                                                                                                                                                    |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_mentorschap"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500003 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 999500004 (negative membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Willem Jansen (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" does not contain Anna Hulpbehoevend (negative membership not expressible in the canonical grammar)

  Scenario: Mentorschap met einddatum in de toekomst is nog actief
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_mentorschap":
      | bsn         | mentorschap_registraties                                                                                                                    |
      | "999400001" | [{"bsn_betrokkene":"999500003","naam_betrokkene":"Willem Jansen","datum_ingang":"2023-06-15","datum_einde":"2026-12-31","status":"ACTIEF"}] |
      | "999500003" | []                                                                                                                                          |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_mentorschap"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500003 (array membership not expressible in the canonical grammar)
