Feature: Uitsluiting van Awir-artikelen door een inkomensafhankelijke regeling
  Awir artikel 4 (kind) en artikel 5 (wijzigingen per maand) haken in op
  elke tegemoetkoming. Een regeling kan zo'n artikel uitsluiten: de Wet op
  het kindgebonden budget sluit artikel 4 helemaal uit (artikel 1, derde
  lid), de Wet kinderopvang sluit artikel 5 uit voor wijzigingen in de kosten
  van kinderopvang per kind (artikel 1.3, tweede lid), en voor andere
  wijzigingen niet.

  Background:
    Given the calculation date is "2026-01-01"
    Given law "algemene_wet_inkomensafhankelijke_regelingen" is loaded

  Scenario: Zonder uitsluiting vuurt Awir 4 voor de kinderopvangtoeslag
    Given the following parameters:
      | kinderopvang_in_geregistreerd_kindercentrum   | true |
      | bloedverwant_of_aanverwant_in_neergaande_lijn | true |
      | in_belangrijke_mate_onderhouden               | true |
      | als_ingezetene_op_woonadres_ingeschreven      | true |
    When I evaluate "heeft_aanspraak_op_kinderopvangtoeslag" of "wet_kinderopvang"
    Then the execution succeeds
    Then output "heeft_aanspraak_op_kinderopvangtoeslag" is true
    Then output "is_kind" is true
    Then output "is_kind" has reactive provenance

  Scenario: De Wet op het kindgebonden budget sluit Awir 4 helemaal uit
    Given the following parameters:
      | kinderbijslag_betaald_of_zou_worden_betaald   | true |
      | bloedverwant_of_aanverwant_in_neergaande_lijn | true |
      | in_belangrijke_mate_onderhouden               | true |
      | als_ingezetene_op_woonadres_ingeschreven      | true |
    When I evaluate "heeft_aanspraak_op_kindgebonden_budget" of "wet_op_het_kindgebonden_budget"
    Then the execution succeeds
    Then output "heeft_aanspraak_op_kindgebonden_budget" is true
    # Awir 4 draait niet: zijn uitkomst ontbreekt, die van Awir 5 niet.
    Then the result contains exactly the outputs "heeft_aanspraak_op_kindgebonden_budget, wijziging_in_aanmerking_vanaf"

  Scenario: Awir 5 vuurt voor de kinderopvangtoeslag bij een andere wijziging
    # Een nieuwe partner op 15 maart telt vanaf 1 april.
    Given the following parameters:
      | kinderopvang_in_geregistreerd_kindercentrum | true       |
      | datum_wijziging                             | 2026-03-15 |
      | gewijzigde_omstandigheid                    | PARTNER    |
    When I evaluate "heeft_aanspraak_op_kinderopvangtoeslag" of "wet_kinderopvang"
    Then the execution succeeds
    Then output "wijziging_in_aanmerking_vanaf" equals "2026-04-01"
    Then output "wijziging_in_aanmerking_vanaf" has reactive provenance

  Scenario: Awir 5 vuurt niet bij een wijziging in de kosten van kinderopvang per kind
    # Een hogere uurprijs op 15 maart valt onder de uitsluiting van artikel
    # 1.3, tweede lid: Awir 5 schuift de wijziging niet naar 1 april.
    Given the following parameters:
      | kinderopvang_in_geregistreerd_kindercentrum | true               |
      | datum_wijziging                             | 2026-03-15         |
      | gewijzigde_omstandigheid                    | PRIJS_KINDEROPVANG |
    When I evaluate "heeft_aanspraak_op_kinderopvangtoeslag" of "wet_kinderopvang"
    Then the execution succeeds
    Then output "heeft_aanspraak_op_kinderopvangtoeslag" is true
    # Awir 5 draait niet: zijn uitkomst ontbreekt, die van Awir 4 niet.
    Then the result contains exactly the outputs "heeft_aanspraak_op_kinderopvangtoeslag, is_kind"

  Scenario: Voor het kindgebonden budget geldt Awir 5 ook bij een wijziging in de kosten
    # De uitsluiting van de Wet kinderopvang geldt alleen voor die wet.
    Given the following parameters:
      | kinderbijslag_betaald_of_zou_worden_betaald | true               |
      | datum_wijziging                             | 2026-03-15         |
      | gewijzigde_omstandigheid                    | PRIJS_KINDEROPVANG |
    When I evaluate "heeft_aanspraak_op_kindgebonden_budget" of "wet_op_het_kindgebonden_budget"
    Then the execution succeeds
    Then output "wijziging_in_aanmerking_vanaf" equals "2026-04-01"
