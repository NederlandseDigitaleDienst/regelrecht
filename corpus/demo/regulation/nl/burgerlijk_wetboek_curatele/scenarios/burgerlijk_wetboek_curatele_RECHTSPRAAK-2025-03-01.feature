# Converted from burgerlijk_wetboek/burgerlijk_wetboek_curatele_RECHTSPRAAK-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Curatele (BW 1:378-391)
  Als Rechtspraak
  Wil ik curatele-registraties beheren
  Zodat curators hun curandussen kunnen vertegenwoordigen

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Curator heeft actieve curatele voor curandus
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                            |
      | "999400001" | [{"bsn_curandus":"999500001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500001" | []                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500001 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Sophie van Dam (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains CURATOR (array membership not expressible in the canonical grammar)
    # POC: output "permissions" contains ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'] (array membership not expressible in the canonical grammar)
    # POC: output "valid_from_dates" contains 2022-01-15 (array membership not expressible in the canonical grammar)

  Scenario: Persoon zonder curatele-registraties heeft geen delegaties
    Given parameter "bsn" is "999993653"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                            |
      | "999993653" | []                                                                                                                               |
      | "999400001" | [{"bsn_curandus":"999500001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500001" | []                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Beeindigde curatele met status BEEINDIGD geeft geen delegaties
    Given parameter "bsn" is "400000007"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                                    |
      | "400000007" | [{"bsn_curandus":"999500009","naam_curandus":"Jan de Boer","datum_ingang":"2020-01-01","datum_einde":"2023-12-31","status":"BEEINDIGD"}] |
      | "999500009" | []                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Curatele met einddatum in het verleden geeft geen delegaties
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                                    |
      | "999400001" | [{"bsn_curandus":"999500001","naam_curandus":"Sophie van Dam","datum_ingang":"2020-01-01","datum_einde":"2024-12-31","status":"ACTIEF"}] |
      | "999500001" | []                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Curator met meerdere curandussen heeft meerdere delegaties
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                                                                                                                                                        |
      | "999400001" | [{"bsn_curandus":"999500001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":null,"status":"ACTIEF"},{"bsn_curandus":"500000010","naam_curandus":"Pieter Zwak","datum_ingang":"2023-06-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500001" | []                                                                                                                                                                                                                                                           |
      | "500000010" | []                                                                                                                                                                                                                                                           |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500001 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 500000010 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Sophie van Dam (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Pieter Zwak (array membership not expressible in the canonical grammar)

  Scenario: Mix van actieve en beeindigde curatele
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                                                                                                                                                                   |
      | "999400001" | [{"bsn_curandus":"999500001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":null,"status":"ACTIEF"},{"bsn_curandus":"999500009","naam_curandus":"Jan de Boer","datum_ingang":"2020-01-01","datum_einde":"2023-12-31","status":"BEEINDIGD"}] |
      | "999500001" | []                                                                                                                                                                                                                                                                      |
      | "999500009" | []                                                                                                                                                                                                                                                                      |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500001 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 999500009 (negative membership not expressible in the canonical grammar)

  Scenario: Curatele met toekomstige einddatum is nog actief
    Given parameter "bsn" is "999400001"
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_curatele":
      | bsn         | curatele_registraties                                                                                                                    |
      | "999400001" | [{"bsn_curandus":"999500001","naam_curandus":"Sophie van Dam","datum_ingang":"2022-01-15","datum_einde":"2026-12-31","status":"ACTIEF"}] |
      | "999500001" | []                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_curatele"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500001 (array membership not expressible in the canonical grammar)
    # POC: output "valid_until_dates" contains 2026-12-31 (array membership not expressible in the canonical grammar)
