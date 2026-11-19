# Converted from burgerlijk_wetboek/burgerlijk_wetboek_gezag_RvIG-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Gezag - Delegation provider for minors
  Als ouder of voogd
  Wil ik namens mijn minderjarige kind kunnen handelen
  Zodat ik regelingen kan aanvragen voor mijn kind

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Ouder met ouderlijk gezag heeft delegatie
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                   |
      | "999993653" | [{"bsn_kind":"888887654","naam_kind":"Jan Jansen","geboortedatum_kind":"2015-03-15","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2015-03-15","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "888887654" | []                                                                                                                                                                                                               |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 888887654 (array membership not expressible in the canonical grammar)
    # POC: output "subject_names" contains Jan Jansen (array membership not expressible in the canonical grammar)
    # POC: output "subject_types" contains CITIZEN (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains OUDERLIJK_GEZAG (array membership not expressible in the canonical grammar)

  Scenario: Persoon zonder gezagsrelaties heeft geen delegaties
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                  |
      | "999993653" | []                                                                                                                                                                                                              |
      | "999111111" | [{"bsn_kind":"999222222","naam_kind":"Kind Test","geboortedatum_kind":"2015-03-15","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2015-03-15","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "999222222" | []                                                                                                                                                                                                              |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is false

  Scenario: Ouder met meerdere kinderen heeft meerdere delegaties
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                                                                                                                                                                                                                                    |
      | "999993653" | [{"bsn_kind":"888887654","naam_kind":"Jan Jansen","geboortedatum_kind":"2015-03-15","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2015-03-15","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false},{"bsn_kind":"777776543","naam_kind":"Marie Jansen","geboortedatum_kind":"2018-06-20","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2018-06-20","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "888887654" | []                                                                                                                                                                                                                                                                                                                                                                                                                                |
      | "777776543" | []                                                                                                                                                                                                                                                                                                                                                                                                                                |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 888887654 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" contains 777776543 (array membership not expressible in the canonical grammar)

  Scenario: Kind van 18 jaar of ouder wordt niet meegenomen
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                    |
      | "999993653" | [{"bsn_kind":"666665432","naam_kind":"Piet Jansen","geboortedatum_kind":"2006-01-01","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2006-01-01","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "666665432" | []                                                                                                                                                                                                                |
    When I evaluate outputs "heeft_delegaties, subject_ids" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is false
    # POC: output is an empty list
    And output "subject_ids" equals "[]"

  Scenario: Voogd heeft delegatie
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                              |
      | "999993653" | [{"bsn_kind":"555554321","naam_kind":"Anna de Vries","geboortedatum_kind":"2017-09-10","type_gezag":"VOOGDIJ","datum_ingang":"2020-01-15","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "555554321" | []                                                                                                                                                                                                          |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 555554321 (array membership not expressible in the canonical grammar)
    # POC: output "delegation_types" contains VOOGDIJ (array membership not expressible in the canonical grammar)

  Scenario: Gezamenlijk gezag - ouder heeft delegatie
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                       |
      | "999993653" | [{"bsn_kind":"444443210","naam_kind":"Sophie Klein","geboortedatum_kind":"2019-04-25","type_gezag":"GEZAMENLIJK_GEZAG","datum_ingang":"2019-04-25","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "444443210" | []                                                                                                                                                                                                                   |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is true
    # POC: output "delegation_types" contains GEZAMENLIJK_GEZAG (array membership not expressible in the canonical grammar)

  Scenario: Inactief gezag wordt niet meegenomen
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                     |
      | "999993653" | [{"bsn_kind":"333332109","naam_kind":"Tim Bakker","geboortedatum_kind":"2016-11-30","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2016-11-30","datum_einde":null,"status":"INACTIEF","heeft_handlichting":false}] |
      | "333332109" | []                                                                                                                                                                                                                 |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is false

  Scenario: Beeindigd gezag wordt niet meegenomen
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                          |
      | "999993653" | [{"bsn_kind":"222220987","naam_kind":"Lisa Smit","geboortedatum_kind":"2014-08-12","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2014-08-12","datum_einde":"2023-06-01","status":"ACTIEF","heeft_handlichting":false}] |
      | "222220987" | []                                                                                                                                                                                                                      |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is false

  Scenario: Mix van minderjarige en meerderjarige kinderen
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                                                                                                                                                                                                                                   |
      | "999993653" | [{"bsn_kind":"111110876","naam_kind":"Mark Jansen","geboortedatum_kind":"2003-02-28","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2003-02-28","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false},{"bsn_kind":"999990765","naam_kind":"Eva Jansen","geboortedatum_kind":"2012-07-14","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2012-07-14","datum_einde":null,"status":"ACTIEF","heeft_handlichting":false}] |
      | "111110876" | []                                                                                                                                                                                                                                                                                                                                                                                                                               |
      | "999990765" | []                                                                                                                                                                                                                                                                                                                                                                                                                               |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 999990765 (array membership not expressible in the canonical grammar)
    # POC: output "subject_ids" does not contain 111110876 (negative membership not expressible in the canonical grammar)

  Scenario: Kind met handlichting - ouder heeft beperkte rechten
    Given the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_gezag":
      | bsn         | gezag_relaties                                                                                                                                                                                                   |
      | "999993653" | [{"bsn_kind":"123456789","naam_kind":"Tom de Jong","geboortedatum_kind":"2008-06-15","type_gezag":"OUDERLIJK_GEZAG","datum_ingang":"2008-06-15","datum_einde":null,"status":"ACTIEF","heeft_handlichting":true}] |
      | "123456789" | []                                                                                                                                                                                                               |
    When I evaluate outputs "heeft_delegaties" of "burgerlijk_wetboek_gezag"
    Then output "heeft_delegaties" is true
    # POC: output "subject_ids" contains 123456789 (array membership not expressible in the canonical grammar)
    # POC: output "permissions" contains ['LEZEN'] (array membership not expressible in the canonical grammar)
