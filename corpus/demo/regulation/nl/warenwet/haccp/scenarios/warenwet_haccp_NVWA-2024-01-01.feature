# Converted from overig/warenwet_haccp_NVWA-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen HACCP-voedselveiligheidsverplichting
  Als horecaondernemer
  Wil ik weten of de HACCP-verplichting op mijn bedrijf van toepassing is
  Zodat ik kan voldoen aan de voedselveiligheidseisen van de Warenwet

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Horecabedrijf (SBI 56102) met hygiënecode - voldoet aan verplichting
    Given parameter "kvk_nummer" is "85234567"
    And the following "KVK" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | sbi_code |
      | 85234567   | "56102"  |
    And the following "NVWA" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | is_geregistreerd_nvwa | heeft_haccp_systeem | type_haccp_systeem |
      | 85234567   | true                  | true                | hygienecode        |
    And the following parameters:
      | bereidt_of_serveert_voedsel | true |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_levensmiddelenbedrijf, heeft_haccp_verplichting, is_geregistreerd, heeft_haccp_systeem, voldoet_aan_verplichting, type_haccp_systeem, toezichthouder" of "warenwet/haccp"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_levensmiddelenbedrijf" is true
    And output "heeft_haccp_verplichting" is true
    And output "is_geregistreerd" is true
    And output "heeft_haccp_systeem" is true
    And output "voldoet_aan_verplichting" is true
    And output "type_haccp_systeem" equals "hygienecode"
    And output "toezichthouder" equals "NVWA"

  Scenario: Restaurant (SBI 56101) zonder HACCP-systeem - voldoet niet
    Given parameter "kvk_nummer" is "85234567"
    And the following "KVK" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | sbi_code |
      | 85234567   | "56101"  |
    And the following "NVWA" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | is_geregistreerd_nvwa | heeft_haccp_systeem | type_haccp_systeem |
      | 85234567   | true                  | false               | null               |
    And the following parameters:
      | bereidt_of_serveert_voedsel | true |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_levensmiddelenbedrijf, heeft_haccp_verplichting, heeft_haccp_systeem, voldoet_aan_verplichting" of "warenwet/haccp"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_levensmiddelenbedrijf" is true
    And output "heeft_haccp_verplichting" is true
    And output "heeft_haccp_systeem" is false
    And output "voldoet_aan_verplichting" is false

  Scenario: Supermarkt (SBI 4711) met eigen HACCP-plan - voldoet
    Given parameter "kvk_nummer" is "85234567"
    And the following "KVK" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | sbi_code |
      | 85234567   | "4711"  |
    And the following "NVWA" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | is_geregistreerd_nvwa | heeft_haccp_systeem | type_haccp_systeem |
      | 85234567   | true                  | true                | eigen_haccp_plan   |
    And the following parameters:
      | bereidt_of_serveert_voedsel | true |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_levensmiddelenbedrijf, voldoet_aan_verplichting, type_haccp_systeem" of "warenwet/haccp"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_levensmiddelenbedrijf" is true
    And output "voldoet_aan_verplichting" is true
    And output "type_haccp_systeem" equals "eigen_haccp_plan"

  Scenario: Bedrijf met niet-voedings SBI maar serveert wel voedsel - verplichting geldt
    Given parameter "kvk_nummer" is "85234567"
    And the following "KVK" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | sbi_code |
      | 85234567   | "8230"  |
    And the following "NVWA" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | is_geregistreerd_nvwa | heeft_haccp_systeem | type_haccp_systeem |
      | 85234567   | false                 | false               | null               |
    And the following parameters:
      | bereidt_of_serveert_voedsel | true |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_levensmiddelenbedrijf, heeft_haccp_verplichting, is_geregistreerd, voldoet_aan_verplichting" of "warenwet/haccp"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_levensmiddelenbedrijf" is true
    And output "heeft_haccp_verplichting" is true
    And output "is_geregistreerd" is false
    And output "voldoet_aan_verplichting" is false

  Scenario: Kledingwinkel - geen levensmiddelenbedrijf
    Given parameter "kvk_nummer" is "99999999"
    And the following "KVK" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | sbi_code |
      | 99999999   | "4771"  |
    And the following "NVWA" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | is_geregistreerd_nvwa | heeft_haccp_systeem | type_haccp_systeem |
      | 99999999   | null                  | null                | null               |
    And the following parameters:
      | bereidt_of_serveert_voedsel | false |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "warenwet/haccp"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: IT-bedrijf - geen levensmiddelenbedrijf
    Given parameter "kvk_nummer" is "99999999"
    And the following "KVK" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | sbi_code |
      | 99999999   | "6201"  |
    And the following "NVWA" data with key "kvk_nummer" for law "warenwet/haccp":
      | kvk_nummer | is_geregistreerd_nvwa | heeft_haccp_systeem | type_haccp_systeem |
      | 99999999   | null                  | null                | null               |
    And the following parameters:
      | bereidt_of_serveert_voedsel | false |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "warenwet/haccp"
    Then output "voldoet_aan_voorwaarden" is false
