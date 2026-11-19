# Converted from overig/faillissementswet_curator_RECHTSPRAAK-2025-03-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Faillissementswet Curator (Fw Art. 64-71)
  Als Rechtspraak
  Wil ik faillissement-registraties beheren
  Zodat curatoren failliete boedels kunnen beheren

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Curator heeft actief faillissement natuurlijk persoon
    Given parameter "bsn" is "400000004"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                        |
      | "400000004" | [{"gefailleerde_id":999500006,"gefailleerde_naam":"Henk Visser","gefailleerde_type":"NATUURLIJK_PERSOON","insolventie_nummer":"F.10/24/123","datum_uitspraak":"2024-03-15","datum_einde":null,"status":"ACTIEF"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_curator"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500006 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Boedel Henk Visser (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains CURATOR_BOEDEL (array membership not expressible in the canonical grammar)
    # POC: output "valid_from_dates" contains 2024-03-15 (array membership not expressible in the canonical grammar)

  Scenario: Curator heeft actief faillissement rechtspersoon
    Given parameter "bsn" is "400000004"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                   |
      | "400000004" | [{"gefailleerde_id":87654321,"gefailleerde_naam":"Failliete BV","gefailleerde_type":"RECHTSPERSOON","insolventie_nummer":"F.10/24/456","datum_uitspraak":"2024-06-01","datum_einde":null,"status":"ACTIEF"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_curator"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 87654321 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Boedel Failliete BV (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains BUSINESS (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains CURATOR_BOEDEL (array membership not expressible in the canonical grammar)
    # POC: output "valid_from_dates" contains 2024-06-01 (array membership not expressible in the canonical grammar)

  Scenario: Curator beheert meerdere failliete boedels
    Given parameter "bsn" is "400000004"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                                                                                                                                                                                                                                   |
      | "400000004" | [{"gefailleerde_id":999500006,"gefailleerde_naam":"Henk Visser","gefailleerde_type":"NATUURLIJK_PERSOON","insolventie_nummer":"F.10/24/123","datum_uitspraak":"2024-03-15","datum_einde":null,"status":"ACTIEF"},{"gefailleerde_id":87654321,"gefailleerde_naam":"Failliete BV","gefailleerde_type":"RECHTSPERSOON","insolventie_nummer":"F.10/24/456","datum_uitspraak":"2024-06-01","datum_einde":null,"status":"ACTIEF"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_curator"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999500006 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 87654321 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Boedel Henk Visser (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Boedel Failliete BV (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains BUSINESS (array membership not expressible in the canonical grammar)

  Scenario: Curator met opgeheven faillissement heeft geen actieve delegatie
    Given parameter "bsn" is "400000004"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                                   |
      | "400000004" | [{"gefailleerde_id":999500006,"gefailleerde_naam":"Henk Visser","gefailleerde_type":"NATUURLIJK_PERSOON","insolventie_nummer":"F.10/24/123","datum_uitspraak":"2024-03-15","datum_einde":"2025-01-15","status":"OPGEHEVEN"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "faillissementswet_curator"
    # Audit-correctie: voldoet_aan_voorwaarden telde eerder alle registraties, ook opgeheven
    # faillissementen; art. 68 jo. 193 Fw geeft curatorbevoegdheid alleen zolang het faillissement
    # actief is.
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Persoon zonder curator-registraties heeft geen delegaties
    Given parameter "bsn" is "999993653"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                        |
      | "999993653" | []                                                                                                                                                                                                                |
      | "400000004" | [{"gefailleerde_id":999500006,"gefailleerde_naam":"Henk Visser","gefailleerde_type":"NATUURLIJK_PERSOON","insolventie_nummer":"F.10/24/123","datum_uitspraak":"2024-03-15","datum_einde":null,"status":"ACTIEF"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties, subject_ids" of "faillissementswet_curator"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Curator met mix van actieve en opgeheven faillissementen
    Given parameter "bsn" is "400000004"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                                                                                                                                                                                                                                              |
      | "400000004" | [{"gefailleerde_id":999500006,"gefailleerde_naam":"Henk Visser","gefailleerde_type":"NATUURLIJK_PERSOON","insolventie_nummer":"F.10/24/123","datum_uitspraak":"2024-03-15","datum_einde":"2025-01-15","status":"OPGEHEVEN"},{"gefailleerde_id":87654321,"gefailleerde_naam":"Failliete BV","gefailleerde_type":"RECHTSPERSOON","insolventie_nummer":"F.10/24/456","datum_uitspraak":"2024-06-01","datum_einde":null,"status":"ACTIEF"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_delegaties" of "faillissementswet_curator"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 87654321 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 999500006 (negative membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Boedel Failliete BV (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" does not contain Boedel Henk Visser (negative membership not expressible in the canonical grammar)

  Scenario: Curator krijgt juiste rechten voor boedelbeheer
    Given parameter "bsn" is "400000004"
    And the following "RECHTSPRAAK" data with key "bsn" for law "faillissementswet_curator":
      | bsn         | faillissement_registraties                                                                                                                                                                                        |
      | "400000004" | [{"gefailleerde_id":999500006,"gefailleerde_naam":"Henk Visser","gefailleerde_type":"NATUURLIJK_PERSOON","insolventie_nummer":"F.10/24/123","datum_uitspraak":"2024-03-15","datum_einde":null,"status":"ACTIEF"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "faillissementswet_curator"
    Then output "voldoet_aan_voorwaarden" is true
    # POC: output "permissions" contains ["LEZEN", "CLAIMS_INDIENEN", "BESLUITEN_ONTVANGEN"] (array membership not expressible in the canonical grammar)
