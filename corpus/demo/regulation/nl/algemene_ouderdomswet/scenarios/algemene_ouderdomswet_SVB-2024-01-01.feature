# Converted from sociale_zekerheid/algemene_ouderdomswet_SVB-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: AOW Pensioen Berekening 2025
  Als burger die de pensioenleeftijd nadert
  Wil ik weten of ik recht heb op AOW pensioen
  Zodat ik mijn pensioenfinanciën kan plannen

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Persoon met gemengde verzekeringsjaren ontvangt gedeeltelijk pensioen
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet":
      | bsn         | woonperiodes |
      | "999993653" | 35           |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-02-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                                                                                     | uitkeringsperiodes                                    |
      | "999993653" | [{"start_date":"2012-01-01","end_date":"2013-01-01"},{"start_date":"2014-01-01","end_date":"2024-01-01"}] | [{"start_date":"2012-01-01","end_date":"2014-01-01"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden, pensioenbedrag" of "algemene_ouderdomswet"
    Then output "voldoet_aan_voorwaarden" is true
    And output "pensioenbedrag" equals 132480

  Scenario: Persoon met volledige opbouw ontvangt volledig pensioen
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet":
      | bsn         | woonperiodes |
      | "999993653" | 50           |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-02-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes | uitkeringsperiodes |
      | "999993653" | []                    | []                 |
    When I evaluate outputs "pensioenbedrag" of "algemene_ouderdomswet"
    Then output "pensioenbedrag" equals 138000

  Scenario: Persoon met partner ontvangt lager basisbedrag
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet":
      | bsn         | woonperiodes |
      | "999993653" | 50           |
      | "999993654" | 0            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-02-15    | HUWELIJK          | "999993654" | []                | []             |               |               | null  | []           | 1956-07-02            |
      | "999993654" | 1956-07-02    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "999993654" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
      | "999993654" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes | uitkeringsperiodes |
      | "999993653" | []                    | []                 |
      | "999993654" | []                    | []                 |
    When I evaluate outputs "pensioenbedrag" of "algemene_ouderdomswet"
    Then output "pensioenbedrag" equals 95200

  Scenario: Te jonge persoon is nog niet AOW-gerechtigd
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet":
      | bsn         | woonperiodes |
      | "999993653" | 0            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1960-02-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes | uitkeringsperiodes |
      | "999993653" | []                    | []                 |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "algemene_ouderdomswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Partner onder AOW-leeftijd geeft recht op toeslag
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet":
      | bsn         | woonperiodes |
      | "999993653" | 50           |
      | "999993654" | 0            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-02-15    | HUWELIJK          | "999993654" | []                | []             |               |               | null  | []           | 1990-10-11            |
      | "999993654" | 1990-10-11    | null              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "999993654" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
      | "999993654" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes | uitkeringsperiodes |
      | "999993653" | []                    | []                 |
      | "999993654" | []                    | []                 |
    When I evaluate outputs "pensioenbedrag" of "algemene_ouderdomswet"
    Then output "pensioenbedrag" equals 121000
