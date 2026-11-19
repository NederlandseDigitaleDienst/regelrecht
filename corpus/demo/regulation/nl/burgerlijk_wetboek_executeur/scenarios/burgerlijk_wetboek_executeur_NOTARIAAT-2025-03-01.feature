# Converted from burgerlijk_wetboek/burgerlijk_wetboek_executeur_NOTARIAAT-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Executeur (BW 4:144-150)
  Als Notariaat
  Wil ik executeur-registraties beheren
  Zodat executeurs nalatenschappen kunnen afwikkelen

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "400000003"

  Scenario: Executeur heeft actieve nalatenschap
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                    |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500005" | []                                                                                                                                                                                                        |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains NAL-2024-00123 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Nalatenschap Karel Posthumus (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains EXECUTEUR_NALATENSCHAP (array membership not expressible in the canonical grammar)

  Scenario: Persoon is geen executeur
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                    |
      | "400000003" | []                                                                                                                                                                                                        |
      | "999999999" | [{"bsn_erflater":"999888888","naam_erflater":"Andere Erflater","nalatenschap_id":"NAL-2024-99999","datum_overlijden":"2024-01-01","datum_aanvaarding":"2024-01-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999888888" | []                                                                                                                                                                                                        |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Nalatenschap is afgewikkeld (status AFGEWIKKELD)
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                                 |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":"2025-01-15","status":"AFGEWIKKELD"}] |
      | "999500005" | []                                                                                                                                                                                                                     |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Executeur heeft benoeming afgewezen
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                             |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"","datum_einde":null,"status":"AFGEWEZEN"}] |
      | "999500005" | []                                                                                                                                                                                                 |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Executeur met meerdere actieve nalatenschappen
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                                                                                                                                                                                                                           |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":null,"status":"ACTIEF"},{"bsn_erflater":"999500006","naam_erflater":"Marie de Vries","nalatenschap_id":"NAL-2024-00456","datum_overlijden":"2024-08-01","datum_aanvaarding":"2024-08-10","datum_einde":null,"status":"ACTIEF"}] |
      | "999500005" | []                                                                                                                                                                                                                                                                                                                                                                                                               |
      | "999500006" | []                                                                                                                                                                                                                                                                                                                                                                                                               |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains NAL-2024-00123 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains NAL-2024-00456 (array membership not expressible in the canonical grammar)

  Scenario: Executeur met mix van actieve en afgewikkelde nalatenschappen
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                                                                                                                                                                                                                                        |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":null,"status":"ACTIEF"},{"bsn_erflater":"999500006","naam_erflater":"Marie de Vries","nalatenschap_id":"NAL-2023-00789","datum_overlijden":"2023-02-01","datum_aanvaarding":"2023-02-15","datum_einde":"2024-03-01","status":"AFGEWIKKELD"}] |
      | "999500005" | []                                                                                                                                                                                                                                                                                                                                                                                                                            |
      | "999500006" | []                                                                                                                                                                                                                                                                                                                                                                                                                            |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains NAL-2024-00123 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain NAL-2023-00789 (negative membership not expressible in the canonical grammar)

  Scenario: Executeurschap met einddatum in de toekomst blijft actief
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                            |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":"2026-06-15","status":"ACTIEF"}] |
      | "999500005" | []                                                                                                                                                                                                                |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains NAL-2024-00123 (array membership not expressible in the canonical grammar)

  Scenario: Executeurschap met einddatum in het verleden is niet meer actief
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                            |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":"2025-01-01","status":"ACTIEF"}] |
      | "999500005" | []                                                                                                                                                                                                                |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Executeur heeft correct rechten voor nalatenschap beheer
    Given the following "NOTARIAAT" data with key "bsn" for law "burgerlijk_wetboek_executeur":
      | bsn         | executeur_registraties                                                                                                                                                                                    |
      | "400000003" | [{"bsn_erflater":"999500005","naam_erflater":"Karel Posthumus","nalatenschap_id":"NAL-2024-00123","datum_overlijden":"2024-06-01","datum_aanvaarding":"2024-06-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500005" | []                                                                                                                                                                                                        |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "burgerlijk_wetboek_executeur"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "permissions" contains ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'] (array membership not expressible in the canonical grammar)
