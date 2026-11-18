# Converted from overig/machtigingenwet_KVK-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Machtigingenwet - Delegation provider for KVK
  Als ondernemer
  Wil ik namens mijn bedrijf kunnen handelen
  Zodat ik regelingen kan aanvragen voor mijn onderneming

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Ondernemer met actieve inschrijving heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                              |
      | "999993653" | [{"kvk_nummer":"12345678","functie":"EIGENAAR","bevoegdheid":"VOLLEDIG","handelsnaam":"Test Onderneming","rechtsvorm":"EENMANSZAAK","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 12345678 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Test Onderneming (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains BUSINESS (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains EIGENAAR (array membership not expressible in the canonical grammar)

  Scenario: Persoon zonder inschrijvingen heeft geen delegaties
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                  |
      | "999993653" | []                                                                                                                                                                                               |
      | "999111111" | [{"kvk_nummer":"00000000","functie":"EIGENAAR","bevoegdheid":"VOLLEDIG","handelsnaam":"Leeg","rechtsvorm":"EENMANSZAAK","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is false

  Scenario: Ondernemer met meerdere bedrijven heeft meerdere delegaties
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                                                                                                                                                                                                      |
      | "999993653" | [{"kvk_nummer":"11111111","functie":"EIGENAAR","bevoegdheid":"VOLLEDIG","handelsnaam":"Bedrijf Een","rechtsvorm":"EENMANSZAAK","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null},{"kvk_nummer":"22222222","functie":"VENNOOT","bevoegdheid":"BEPERKT","handelsnaam":"Bedrijf Twee","rechtsvorm":"VOF","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 11111111 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 22222222 (array membership not expressible in the canonical grammar)

  Scenario: Commissaris heeft geen vertegenwoordigingsbevoegdheid
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                               |
      | "999993653" | [{"kvk_nummer":"33333333","functie":"COMMISSARIS","bevoegdheid":"GEEN","handelsnaam":"Holdings BV","rechtsvorm":"BV","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties, subject_ids" of "machtigingenwet"
    Then output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Gemengde rollen - alleen bevoegde functies krijgen delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
      | "999993653" | [{"kvk_nummer":"44444444","functie":"EIGENAAR","bevoegdheid":"VOLLEDIG","handelsnaam":"Eigen Bedrijf","rechtsvorm":"EENMANSZAAK","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null},{"kvk_nummer":"55555555","functie":"VENNOOT","bevoegdheid":"BEPERKT","handelsnaam":"Zorggroep VOF","rechtsvorm":"VOF","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null},{"kvk_nummer":"66666666","functie":"COMMISSARIS","bevoegdheid":"GEEN","handelsnaam":"Toezicht Holdings BV","rechtsvorm":"BV","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 44444444 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 55555555 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 66666666 (negative membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Eigen Bedrijf (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Zorggroep VOF (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" does not contain Toezicht Holdings BV (negative membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains EIGENAAR (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains VENNOOT (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" does not contain COMMISSARIS (negative membership not expressible in the canonical grammar)

  Scenario: Bestuurder van BV heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                        |
      | "999993653" | [{"kvk_nummer":"77777777","functie":"BESTUURDER","bevoegdheid":"VOLLEDIG","handelsnaam":"Tech Solutions BV","rechtsvorm":"BV","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 77777777 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BESTUURDER (array membership not expressible in the canonical grammar)

  Scenario: Gemachtigde heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                       |
      | "999993653" | [{"kvk_nummer":"88888888","functie":"GEMACHTIGDE","bevoegdheid":"BEPERKT","handelsnaam":"Groot Bedrijf BV","rechtsvorm":"BV","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 88888888 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains GEMACHTIGDE (array membership not expressible in the canonical grammar)

  Scenario: Bestuurder van NV heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                       |
      | "999993653" | [{"kvk_nummer":"90000001","functie":"BESTUURDER","bevoegdheid":"VOLLEDIG","handelsnaam":"Grote Holding NV","rechtsvorm":"NV","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 90000001 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BESTUURDER (array membership not expressible in the canonical grammar)

  Scenario: Directeur van NV heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                       |
      | "999993653" | [{"kvk_nummer":"90000002","functie":"DIRECTEUR","bevoegdheid":"VOLLEDIG","handelsnaam":"Beursgenoteerd NV","rechtsvorm":"NV","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "delegation_types" contains DIRECTEUR (array membership not expressible in the canonical grammar)

  Scenario: Bestuurder van Stichting heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                    |
      | "999993653" | [{"kvk_nummer":"90000003","functie":"BESTUURDER","bevoegdheid":"VOLLEDIG","handelsnaam":"Stichting Goede Doelen","rechtsvorm":"STICHTING","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 90000003 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Stichting Goede Doelen (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BESTUURDER (array membership not expressible in the canonical grammar)

  Scenario: Stichting met beperkte bevoegdheid
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                |
      | "999993653" | [{"kvk_nummer":"90000004","functie":"BESTUURDER","bevoegdheid":"BEPERKT","handelsnaam":"Stichting Onderwijs","rechtsvorm":"STICHTING","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "permissions" contains ['LEZEN'] (array membership not expressible in the canonical grammar)

  Scenario: Bestuurder van Vereniging heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                      |
      | "999993653" | [{"kvk_nummer":"90000005","functie":"BESTUURDER","bevoegdheid":"VOLLEDIG","handelsnaam":"Sportvereniging De Hoop","rechtsvorm":"VERENIGING","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 90000005 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BESTUURDER (array membership not expressible in the canonical grammar)

  Scenario: Voorzitter van Vereniging heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                               |
      | "999993653" | [{"kvk_nummer":"90000006","functie":"VOORZITTER","bevoegdheid":"VOLLEDIG","handelsnaam":"Muziekvereniging","rechtsvorm":"VERENIGING","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "delegation_types" contains VOORZITTER (array membership not expressible in the canonical grammar)

  Scenario: Bestuurder van Coöperatie heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                     |
      | "999993653" | [{"kvk_nummer":"90000007","functie":"BESTUURDER","bevoegdheid":"VOLLEDIG","handelsnaam":"Coöperatie Boeren U.A.","rechtsvorm":"COOPERATIE","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 90000007 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BESTUURDER (array membership not expressible in the canonical grammar)

  Scenario: Maat in Maatschap heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                |
      | "999993653" | [{"kvk_nummer":"90000008","functie":"MAAT","bevoegdheid":"VOLLEDIG","handelsnaam":"Advocatenmaatschap Recht","rechtsvorm":"MAATSCHAP","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 90000008 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains MAAT (array membership not expressible in the canonical grammar)

  Scenario: Beherend vennoot van CV heeft delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                                 |
      | "999993653" | [{"kvk_nummer":"90000009","functie":"BEHEREND_VENNOOT","bevoegdheid":"VOLLEDIG","handelsnaam":"Investeer CV","rechtsvorm":"COMMANDITAIRE_VENNOOTSCHAP","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 90000009 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains BEHEREND_VENNOOT (array membership not expressible in the canonical grammar)

  Scenario: Commanditair vennoot van CV heeft GEEN delegatie
    Given the following "KVK" data with key "bsn" for law "machtigingenwet":
      | bsn         | functionarissen                                                                                                                                                                                                                |
      | "999993653" | [{"kvk_nummer":"90000010","functie":"COMMANDITAIR_VENNOOT","bevoegdheid":"GEEN","handelsnaam":"Kapitaal CV","rechtsvorm":"COMMANDITAIRE_VENNOOTSCHAP","status":"ACTIEF","datum_inschrijving":null,"datum_uitschrijving":null}] |
    When I evaluate outputs "heeft_delegaties" of "machtigingenwet"
    Then output "heeft_delegaties" is false
