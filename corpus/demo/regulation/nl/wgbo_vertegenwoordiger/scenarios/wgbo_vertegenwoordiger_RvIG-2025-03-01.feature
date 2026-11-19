# Converted from overig/wgbo_vertegenwoordiger_RvIG-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: WGBO Vertegenwoordiger (BW 7:465)
  Als RvIG
  Wil ik medische vertegenwoordigingsrelaties bepalen
  Zodat wilsonbekwame patienten vertegenwoordigd kunnen worden bij medische beslissingen

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Echtgenoot is WGBO vertegenwoordiger voor wilsonbekwame partner
    Given parameter "bsn" is "400000006"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                                     |
      | "400000006" | [{"bsn_patient":"999500008","naam_patient":"Gerda Groen-van Dijk","relatie_type":"ECHTGENOOT","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-01-10","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "999500008" | []                                                                                                                                                                                                                   |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500008 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Gerda Groen-van Dijk (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_PARTNER (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)

  Scenario: Geregistreerd partner is WGBO vertegenwoordiger voor wilsonbekwame partner
    Given parameter "bsn" is "400000010"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                                             |
      | "400000010" | [{"bsn_patient":"500000010","naam_patient":"Jan Geregistreerd","relatie_type":"GEREGISTREERD_PARTNER","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-02-15","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000010" | []                                                                                                                                                                                                                           |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 500000010 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_PARTNER (array membership not expressible in the canonical grammar)

  Scenario: Patient is wilsbekwaam - geen WGBO vertegenwoordiging
    Given parameter "bsn" is "400000011"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                        |
      | "400000011" | [{"bsn_patient":"500000011","naam_patient":"Henk Wilsbekwaam","relatie_type":"ECHTGENOOT","is_wilsonbekwaam":false,"datum_wilsonbekwaamheid":"","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000011" | []                                                                                                                                                                                                      |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Ouder is WGBO vertegenwoordiger voor wilsonbekwaam meerderjarig kind
    Given parameter "bsn" is "400000012"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                         |
      | "400000012" | [{"bsn_patient":"500000012","naam_patient":"Marie Dochter","relatie_type":"OUDER","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2023-06-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000012" | []                                                                                                                                                                                                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_OUDER (array membership not expressible in the canonical grammar)

  Scenario: Kind is WGBO vertegenwoordiger voor wilsonbekwame ouder
    Given parameter "bsn" is "400000013"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                     |
      | "400000013" | [{"bsn_patient":"500000013","naam_patient":"Piet Vader","relatie_type":"KIND","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-03-20","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000013" | []                                                                                                                                                                                                   |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_KIND (array membership not expressible in the canonical grammar)

  Scenario: Broer is WGBO vertegenwoordiger voor wilsonbekwame zus
    Given parameter "bsn" is "400000014"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                    |
      | "400000014" | [{"bsn_patient":"500000014","naam_patient":"Anna Zus","relatie_type":"BROER","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-05-10","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000014" | []                                                                                                                                                                                                  |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_SIBLING (array membership not expressible in the canonical grammar)

  Scenario: Zus is WGBO vertegenwoordiger voor wilsonbekwame broer
    Given parameter "bsn" is "400000015"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                     |
      | "400000015" | [{"bsn_patient":"500000015","naam_patient":"Klaas Broer","relatie_type":"ZUS","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-04-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000015" | []                                                                                                                                                                                                   |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_SIBLING (array membership not expressible in the canonical grammar)

  Scenario: Levensgezel is WGBO vertegenwoordiger (gelijk aan partner in hierarchie)
    Given parameter "bsn" is "400000016"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                                  |
      | "400000016" | [{"bsn_patient":"500000016","naam_patient":"Sara Levensgezel","relatie_type":"LEVENSGEZEL","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-06-15","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000016" | []                                                                                                                                                                                                                |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_PARTNER (array membership not expressible in the canonical grammar)

  Scenario: Persoon zonder familierelatie met wilsonbekwame heeft geen WGBO vertegenwoordiging
    Given parameter "bsn" is "400000017"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                               |
      | "400000017" | []                                                                                                                                                                                                             |
      | "999999999" | [{"bsn_patient":"500000099","naam_patient":"Andere Familie","relatie_type":"ECHTGENOOT","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-01-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000099" | []                                                                                                                                                                                                             |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false

  Scenario: Persoon is vertegenwoordiger voor meerdere wilsonbekwame familieleden
    Given parameter "bsn" is "400000018"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                                                                                                                                                                                                                             |
      | "400000018" | [{"bsn_patient":"500000018","naam_patient":"Moeder Martha","relatie_type":"KIND","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2023-01-15","is_gemachtigd":false,"wenst_niet_op_te_treden":false},{"bsn_patient":"500000019","naam_patient":"Vader Victor","relatie_type":"KIND","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-02-20","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000018" | []                                                                                                                                                                                                                                                                                                                                                                                                           |
      | "500000019" | []                                                                                                                                                                                                                                                                                                                                                                                                           |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 500000018 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 500000019 (array membership not expressible in the canonical grammar)

  Scenario: Neef/nicht is geen geldige WGBO vertegenwoordiger
    Given parameter "bsn" is "400000019"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                  |
      | "400000019" | [{"bsn_patient":"500000020","naam_patient":"Oom Jan","relatie_type":"NEEF","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-01-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000020" | []                                                                                                                                                                                                |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: WGBO vertegenwoordiger krijgt volledige medische beslissingsrechten
    Given parameter "bsn" is "400000006"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                                     |
      | "400000006" | [{"bsn_patient":"999500008","naam_patient":"Gerda Groen-van Dijk","relatie_type":"ECHTGENOOT","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-01-10","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "999500008" | []                                                                                                                                                                                                                   |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "valid_from_dates" contains 2024-01-10 (array membership not expressible in the canonical grammar)

  Scenario: Persoon met zowel wilsbekwame als wilsonbekwame familieleden
    Given parameter "bsn" is "400000020"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                                                                                                                                                                                                                                  |
      | "400000020" | [{"bsn_patient":"500000021","naam_patient":"Emma Wilsbekwaam","relatie_type":"ECHTGENOOT","is_wilsonbekwaam":false,"datum_wilsonbekwaamheid":"","is_gemachtigd":false,"wenst_niet_op_te_treden":false},{"bsn_patient":"500000022","naam_patient":"Opa Wilsonbekwaam","relatie_type":"KIND","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-07-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000021" | []                                                                                                                                                                                                                                                                                                                                                                                                                |
      | "500000022" | []                                                                                                                                                                                                                                                                                                                                                                                                                |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 500000022 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 500000021 (negative membership not expressible in the canonical grammar)

  Scenario: Schriftelijk gemachtigde is WGBO vertegenwoordiger (art. 7:465 lid 3 BW)
    Given parameter "bsn" is "400000021"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                        |
      | "400000021" | [{"bsn_patient":"500000023","naam_patient":"Willem Patiënt","relatie_type":"NEEF","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-08-01","is_gemachtigd":true,"wenst_niet_op_te_treden":false}] |
      | "500000023" | []                                                                                                                                                                                                      |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_GEMACHTIGDE (array membership not expressible in the canonical grammar)

  Scenario: Echtgenoot die niet wenst op te treden is geen WGBO vertegenwoordiger (art. 7:465 lid 3 BW)
    Given parameter "bsn" is "400000022"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                             |
      | "400000022" | [{"bsn_patient":"500000024","naam_patient":"Sofie Patiënt","relatie_type":"ECHTGENOOT","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-09-01","wenst_niet_op_te_treden":true,"is_gemachtigd":false}] |
      | "500000024" | []                                                                                                                                                                                                           |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Grootouder is WGBO vertegenwoordiger voor wilsonbekwaam kleinkind (art. 7:465 lid 3 BW)
    Given parameter "bsn" is "400000023"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                              |
      | "400000023" | [{"bsn_patient":"500000025","naam_patient":"Tim Kleinkind","relatie_type":"GROOTOUDER","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-10-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000025" | []                                                                                                                                                                                                            |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_GROOTOUDER (array membership not expressible in the canonical grammar)

  Scenario: Kleinkind is WGBO vertegenwoordiger voor wilsonbekwame grootouder (art. 7:465 lid 3 BW)
    Given parameter "bsn" is "400000024"
    And the following "RvIG" data with key "bsn" for law "wgbo_vertegenwoordiger":
      | bsn         | familie_relaties                                                                                                                                                                                              |
      | "400000024" | [{"bsn_patient":"500000026","naam_patient":"Ans Grootouder","relatie_type":"KLEINKIND","is_wilsonbekwaam":true,"datum_wilsonbekwaamheid":"2024-11-01","is_gemachtigd":false,"wenst_niet_op_te_treden":false}] |
      | "500000026" | []                                                                                                                                                                                                            |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "wgbo_vertegenwoordiger"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "delegation_types" contains WGBO_VERTEGENWOORDIGER_KLEINKIND (array membership not expressible in the canonical grammar)
