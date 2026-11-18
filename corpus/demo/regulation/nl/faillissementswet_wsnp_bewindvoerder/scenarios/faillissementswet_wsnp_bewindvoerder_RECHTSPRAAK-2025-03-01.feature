# Converted from overig/faillissementswet_wsnp_bewindvoerder_RECHTSPRAAK-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Faillissementswet WSNP Bewindvoerder (Fw Titel III)
  Als Rechtspraak
  Wil ik WSNP-registraties beheren
  Zodat bewindvoerders schuldsaneringen kunnen begeleiden

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: WSNP bewindvoerder heeft actieve schuldsanering
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                               |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500007 (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains WSNP_BEWINDVOERDER_BOEDEL (array membership not expressible in the canonical grammar)

  Scenario: Persoon is geen WSNP bewindvoerder
    Given parameter "bsn" is "123456789"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                               |
      | "123456789" | []                                                                                                                                                              |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Voltooide WSNP met schone lei - bewindvoerder heeft geen actieve delegaties meer
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                                           |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":"2025-02-01","status":"SCHONE_LEI"}] |
      | "999500007" | []                                                                                                                                                                          |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "faillissementswet_wsnp_bewindvoerder"
    # Audit-correctie: voldoet_aan_voorwaarden telde eerder alle registraties, ook beëindigde;
    # art. 316 jo. 356 Fw geeft alleen bevoegdheid zolang de schuldsanering actief is.
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: WSNP beeindigd zonder schone lei
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                                          |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":"2024-06-15","status":"BEEINDIGD"}] |
      | "999500007" | []                                                                                                                                                                         |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "faillissementswet_wsnp_bewindvoerder"
    # Audit-correctie: voldoet_aan_voorwaarden telde eerder alle registraties, ook beëindigde;
    # art. 316 jo. 356 Fw geeft alleen bevoegdheid zolang de schuldsanering actief is.
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Bewindvoerder heeft meerdere actieve WSNP-zaken
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":null,"status":"ACTIEF"},{"bsn_saniet":999500008,"naam_saniet":"Pieter Probleem","insolventie_nummer":"R.18/24/123","datum_uitspraak":"2024-03-15","datum_einde":null,"status":"ACTIEF"},{"bsn_saniet":999500009,"naam_saniet":"Maria Moeilijk","insolventie_nummer":"R.18/24/456","datum_uitspraak":"2024-06-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
      | "999500008" | []                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
      | "999500009" | []                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500007 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 999500008 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 999500009 (array membership not expressible in the canonical grammar)

  Scenario: Mix van actieve en voltooide WSNP-zaken - alleen actieve tellen mee
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                                                                                                                                                                                                           |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":"2025-02-01","status":"SCHONE_LEI"},{"bsn_saniet":999500008,"naam_saniet":"Pieter Probleem","insolventie_nummer":"R.18/24/123","datum_uitspraak":"2024-03-15","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                                                                                                                                                                                                          |
      | "999500008" | []                                                                                                                                                                                                                                                                                                                                          |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500008 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 999500007 (negative membership not expressible in the canonical grammar)

  Scenario: WSNP met einddatum in de toekomst is nog actief
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                                       |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":"2025-09-01","status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                                      |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500007 (array membership not expressible in the canonical grammar)

  Scenario: WSNP bewindvoerder heeft correcte rechten voor boedelbeheer
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                               |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true

  Scenario: WSNP standaardduur van 18 maanden
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                               |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "valid_from_dates" contains 2023-09-01 (array membership not expressible in the canonical grammar)

  Scenario: Sanietnamen bevatten WSNP Boedel prefix
    Given parameter "bsn" is "400000005"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_wsnp_bewindvoerder":
      | bsn         | wsnp_registraties                                                                                                                                               |
      | "400000005" | [{"bsn_saniet":999500007,"naam_saniet":"Sandra Meijer","insolventie_nummer":"R.18/23/789","datum_uitspraak":"2023-09-01","datum_einde":null,"status":"ACTIEF"}] |
      | "999500007" | []                                                                                                                                                              |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "faillissementswet_wsnp_bewindvoerder"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "subject_names" contains WSNP Boedel Sandra Meijer (array membership not expressible in the canonical grammar)
