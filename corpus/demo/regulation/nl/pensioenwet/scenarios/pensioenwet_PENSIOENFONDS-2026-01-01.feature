# Converted from sociale_zekerheid/pensioenwet_PENSIOENFONDS-2026-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Pensioenwet - Pensioenuitkering berekening
  Als pensioengerechtigde
  Wil ik weten wat mijn pensioenuitkering is
  Zodat ik mijn inkomen na pensionering kan plannen

  Background:
    Given the calculation date is "2026-07-01"

  Scenario: Gepensioneerde met beschikbare premieregeling
    Given parameter "bsn" is "999100001"
    And the following "PENSIOENFONDS" data with key "bsn" for law "pensioenwet":
      | bsn         | pensioenkapitaal | pensioenjaren | type_regeling      | franchise | pensioengevend_loon | pensioen_leeftijd_fonds |
      | "999100001" | 30000000         | 40            | beschikbare_premie | 1750000   | 6000000             | 67                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100001" | 1959-03-15    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_gepensioneerd, pensioenkapitaal, pensioen_uitkering_maandelijks" of "pensioenwet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_gepensioneerd" is true
    And output "pensioenkapitaal" equals 30000000
    And output "pensioen_uitkering_maandelijks" equals 125000

  Scenario: Gepensioneerde met middelloonregeling
    Given parameter "bsn" is "999100002"
    And the following "PENSIOENFONDS" data with key "bsn" for law "pensioenwet":
      | bsn         | pensioenkapitaal | pensioenjaren | type_regeling | franchise | pensioengevend_loon | pensioen_leeftijd_fonds |
      | "999100002" | 25000000         | 35            | middelloon    | 1750000   | 5500000             | 67                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100002" | 1958-06-20    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_gepensioneerd, pensioen_uitkering_maandelijks" of "pensioenwet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_gepensioneerd" is true
    And output "pensioen_uitkering_maandelijks" equals 191406

  Scenario: Gepensioneerde met eindloonregeling
    Given parameter "bsn" is "999100003"
    And the following "PENSIOENFONDS" data with key "bsn" for law "pensioenwet":
      | bsn         | pensioenkapitaal | pensioenjaren | type_regeling | franchise | pensioengevend_loon | pensioen_leeftijd_fonds |
      | "999100003" | 40000000         | 40            | eindloon      | 1750000   | 7000000             | 67                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100003" | 1957-01-10    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_gepensioneerd, pensioen_uitkering_maandelijks" of "pensioenwet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_gepensioneerd" is true
    And output "pensioen_uitkering_maandelijks" equals 292250

  Scenario: Persoon nog niet pensioengerechtigd
    Given parameter "bsn" is "999100004"
    And the following "PENSIOENFONDS" data with key "bsn" for law "pensioenwet":
      | bsn         | pensioenkapitaal | pensioenjaren | type_regeling      | franchise | pensioengevend_loon | pensioen_leeftijd_fonds |
      | "999100004" | 15000000         | 20            | beschikbare_premie | 1750000   | 5000000             | 67                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100004" | 1970-08-25    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "pensioenwet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Persoon zonder pensioenkapitaal
    Given parameter "bsn" is "100000005"
    And the following "PENSIOENFONDS" data with key "bsn" for law "pensioenwet":
      | bsn         | pensioenkapitaal | pensioenjaren | type_regeling      | franchise | pensioengevend_loon | pensioen_leeftijd_fonds |
      | "100000005" | 0                | 0             | beschikbare_premie | 0         | 0                   | 67                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "100000005" | 1958-12-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "pensioenwet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Vervroegd pensioen bij lagere pensioenleeftijd
    Given parameter "bsn" is "100000006"
    And the following "PENSIOENFONDS" data with key "bsn" for law "pensioenwet":
      | bsn         | pensioenkapitaal | pensioenjaren | type_regeling      | franchise | pensioengevend_loon | pensioen_leeftijd_fonds |
      | "100000006" | 20000000         | 30            | beschikbare_premie | 1750000   | 5000000             | 65                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "100000006" | 1961-04-15    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, pensioenleeftijd, pensioen_uitkering_maandelijks" of "pensioenwet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "pensioenleeftijd" equals 65
    And output "pensioen_uitkering_maandelijks" equals 83333
