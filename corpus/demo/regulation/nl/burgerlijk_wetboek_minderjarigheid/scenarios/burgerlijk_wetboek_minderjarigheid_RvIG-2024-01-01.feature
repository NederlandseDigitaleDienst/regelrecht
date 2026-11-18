# Converted from burgerlijk_wetboek/burgerlijk_wetboek_minderjarigheid_RvIG-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Burgerlijk Wetboek Minderjarigheid - Handelingsbekwaamheid
  Als minderjarige
  Wil ik weten wat mijn rechten zijn
  Zodat ik weet wat ik zelf kan doen

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: Minderjarige (14 jaar) heeft alleen leesrecht
    Given parameter "bsn" is "999200001"
    And the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_minderjarigheid":
      | bsn         | persoonsgegevens                                                                       |
      | "999200001" | {"geboortedatum":"2011-06-15","heeft_handlichting":false,"handlichting_gebieden":null} |
    When I evaluate outputs "is_minderjarig, leeftijd" of "burgerlijk_wetboek_minderjarigheid"
    Then output "is_minderjarig" is true
    And output "leeftijd" equals 13
    # POC: output "base_permissions" contains LEZEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" does not contain CLAIMS_INDIENEN (negative membership not expressible in the canonical grammar)

  Scenario: Meerderjarige (19 jaar) heeft volledige rechten
    Given parameter "bsn" is "999300001"
    And the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_minderjarigheid":
      | bsn         | persoonsgegevens                                                                       |
      | "999300001" | {"geboortedatum":"2006-01-01","heeft_handlichting":false,"handlichting_gebieden":null} |
    When I evaluate outputs "is_minderjarig, leeftijd" of "burgerlijk_wetboek_minderjarigheid"
    Then output "is_minderjarig" is false
    And output "leeftijd" equals 19
    # POC: output "base_permissions" contains CLAIMS_INDIENEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" contains BESLUITEN_ONTVANGEN (array membership not expressible in the canonical grammar)

  Scenario: Minderjarige (17 jaar) met handlichting heeft volledige rechten
    Given parameter "bsn" is "999400001"
    And the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_minderjarigheid":
      | bsn         | persoonsgegevens                                                                                    |
      | "999400001" | {"geboortedatum":"2008-03-20","heeft_handlichting":true,"handlichting_gebieden":["BEROEP_BEDRIJF"]} |
    When I evaluate outputs "is_minderjarig, leeftijd, heeft_handlichting" of "burgerlijk_wetboek_minderjarigheid"
    Then output "is_minderjarig" is true
    And output "leeftijd" equals 16
    And output "heeft_handlichting" is true
    # POC: output "base_permissions" contains CLAIMS_INDIENEN (array membership not expressible in the canonical grammar)
    # POC: output "handlichting_gebieden" contains BEROEP_BEDRIJF (array membership not expressible in the canonical grammar)

  Scenario: Net 18 geworden - meerderjarig
    Given parameter "bsn" is "999500001"
    And the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_minderjarigheid":
      | bsn         | persoonsgegevens                                                                       |
      | "999500001" | {"geboortedatum":"2007-03-01","heeft_handlichting":false,"handlichting_gebieden":null} |
    When I evaluate outputs "is_minderjarig, leeftijd" of "burgerlijk_wetboek_minderjarigheid"
    Then output "is_minderjarig" is false
    And output "leeftijd" equals 18
    # POC: output "base_permissions" contains CLAIMS_INDIENEN (array membership not expressible in the canonical grammar)

  Scenario: Bijna 18 - nog minderjarig
    Given parameter "bsn" is "999600001"
    And the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_minderjarigheid":
      | bsn         | persoonsgegevens                                                                       |
      | "999600001" | {"geboortedatum":"2007-03-02","heeft_handlichting":false,"handlichting_gebieden":null} |
    When I evaluate outputs "is_minderjarig, leeftijd" of "burgerlijk_wetboek_minderjarigheid"
    Then output "is_minderjarig" is true
    And output "leeftijd" equals 17
    # POC: output "base_permissions" contains LEZEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" does not contain CLAIMS_INDIENEN (negative membership not expressible in the canonical grammar)

  Scenario: Kind van 5 jaar heeft alleen leesrecht
    Given parameter "bsn" is "999200002"
    And the following "RvIG" data with key "bsn" for law "burgerlijk_wetboek_minderjarigheid":
      | bsn         | persoonsgegevens                                                                       |
      | "999200002" | {"geboortedatum":"2020-01-01","heeft_handlichting":false,"handlichting_gebieden":null} |
    When I evaluate outputs "is_minderjarig, leeftijd" of "burgerlijk_wetboek_minderjarigheid"
    Then output "is_minderjarig" is true
    And output "leeftijd" equals 5
    # POC: output "base_permissions" contains LEZEN (array membership not expressible in the canonical grammar)
    # POC: output "base_permissions" does not contain CLAIMS_INDIENEN (negative membership not expressible in the canonical grammar)
