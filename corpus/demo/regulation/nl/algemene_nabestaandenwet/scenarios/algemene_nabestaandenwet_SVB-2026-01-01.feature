# Converted from sociale_zekerheid/algemene_nabestaandenwet_SVB-2026-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Anw - Nabestaandenuitkering
  Als nabestaande van een verzekerde
  Wil ik weten of ik recht heb op een nabestaandenuitkering
  Zodat ik mijn financiële situatie kan plannen

  Background:
    Given the calculation date is "2026-07-01"

  Scenario: Nabestaande met kind onder 18 jaar zonder inkomen
    Given parameter "bsn" is "999300001"
    And the following "SVB" data with key "bsn" for law "algemene_nabestaandenwet":
      | bsn         | partner_verzekerd | overlijdensdatum_partner | ao_percentage |
      | "999300001" | true              | 2026-01-15               | 0             |
      | "300000101" | null              | null                     | 0             |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                  | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999300001" | 1985-06-15    | null              | null        | [{"bsn":"999300201","geboortedatum":"2020-01-01"}] | []             |               |               | null  | []           |                       |
      | "300000101" |               | null              | null        | []                                                 | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999300001" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "300000101" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999300001" | 20.5           |
      | "300000101" | 20.5           |
    When I evaluate outputs "heeft_kind_onder_18, voldoet_aan_voorwaarden, is_gerechtigd, bruto_uitkering, inkomenskorting, netto_uitkering" of "algemene_nabestaandenwet"
    Then output "heeft_kind_onder_18" is true
    And output "voldoet_aan_voorwaarden" is true
    And output "is_gerechtigd" is true
    And output "bruto_uitkering" equals 146500
    And output "inkomenskorting" equals 0
    And output "netto_uitkering" equals 146500

  Scenario: Nabestaande met arbeidsongeschiktheid 45% of meer
    Given parameter "bsn" is "300000002"
    And the following "SVB" data with key "bsn" for law "algemene_nabestaandenwet":
      | bsn         | partner_verzekerd | overlijdensdatum_partner | ao_percentage |
      | "300000002" | true              | 2026-02-01               | 50            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "300000002" | 1975-09-20    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "300000002" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "300000002" | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_gerechtigd, netto_uitkering" of "algemene_nabestaandenwet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_gerechtigd" is true
    And output "netto_uitkering" equals 146500

  Scenario: Nabestaande met inkomen boven vrijlating
    Given parameter "bsn" is "300000003"
    And the following "SVB" data with key "bsn" for law "algemene_nabestaandenwet":
      | bsn         | partner_verzekerd | overlijdensdatum_partner | ao_percentage |
      | "300000003" | true              | 2026-03-01               | 0             |
      | "300000103" | null              | null                     | 0             |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                  | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "300000003" | 1980-01-10    | null              | null        | [{"bsn":"300000103","geboortedatum":"2015-06-01"}] | []             |               |               | null  | []           |                       |
      | "300000103" |               | null              | null        | []                                                 | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "300000003" | 2400000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "300000103" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "300000003" | 20.5           |
      | "300000103" | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_gerechtigd, inkomenskorting, netto_uitkering" of "algemene_nabestaandenwet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_gerechtigd" is true
    And output "inkomenskorting" equals 123000
    And output "netto_uitkering" equals 23500

  Scenario: Nabestaande zonder kinderen en zonder arbeidsongeschiktheid
    Given parameter "bsn" is "300000004"
    And the following "SVB" data with key "bsn" for law "algemene_nabestaandenwet":
      | bsn         | partner_verzekerd | overlijdensdatum_partner | ao_percentage |
      | "300000004" | true              | 2026-04-01               | 30            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "300000004" | 1970-04-25    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "300000004" | 3000000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "300000004" | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "algemene_nabestaandenwet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Nabestaande van niet-verzekerde partner
    Given parameter "bsn" is "300000005"
    And the following "SVB" data with key "bsn" for law "algemene_nabestaandenwet":
      | bsn         | partner_verzekerd | overlijdensdatum_partner | ao_percentage |
      | "300000005" | false             | 2026-05-01               | 0             |
      | "300000105" | null              | null                     | 0             |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "300000005" | 1982-11-30    | null              | null        | []                | []             |               |               | null  | []           |                       |
      | "300000105" |               | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "300000005" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "300000105" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "300000005" | 20.5           |
      | "300000105" | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "algemene_nabestaandenwet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Nabestaande die AOW-leeftijd heeft bereikt
    Given parameter "bsn" is "300000006"
    And the following "SVB" data with key "bsn" for law "algemene_nabestaandenwet":
      | bsn         | partner_verzekerd | overlijdensdatum_partner | ao_percentage |
      | "300000006" | true              | 2026-06-01               | 0             |
      | "300000106" | null              | null                     | 0             |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "300000006" | 1958-01-15    | null              | null        | []                | []             |               |               | null  | []           |                       |
      | "300000106" |               | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "300000006" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "300000106" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "300000006" | 20.5           |
      | "300000106" | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "algemene_nabestaandenwet"
    Then output "voldoet_aan_voorwaarden" is false
