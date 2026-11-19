# Converted from toeslagen/zorgtoeslagwet_TOESLAGEN-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Berekening Zorgtoeslag 2024
  Als burger
  Wil ik weten of ik recht heb op zorgtoeslag
  Zodat ik de juiste toeslag kan ontvangen

  Background:
    Given the calculation date is "2024-02-01"
    And parameter "bsn" is "999993653"

  Scenario: Persoon onder 18 heeft geen recht op zorgtoeslag
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | VRIJ   | GEEN            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2007-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "DJI" data with key "bsn" for law "wet_forensische_zorg":
      | bsn         | zorgtype | juridische_titel |
      | "999993653" | null     | null             |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.4           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "voldoet_aan_voorwaarden, hoogte_toeslag" of "zorgtoeslagwet"
    Then output "voldoet_aan_voorwaarden" is false
    And output "hoogte_toeslag" is absent

  Scenario: Persoon boven 18 heeft recht op zorgtoeslag
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2005-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "DJI" data with key "bsn" for law "wet_forensische_zorg":
      | bsn         | zorgtype | juridische_titel |
      | "999993653" | null     | null             |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 79547                     | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.4           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "hoogte_toeslag" of "zorgtoeslagwet"
    Then output "hoogte_toeslag" equals 197205

  Scenario: Alleenstaande met laag inkomen heeft recht op zorgtoeslag
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1998-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "DJI" data with key "bsn" for law "wet_forensische_zorg":
      | bsn         | zorgtype | juridische_titel |
      | "999993653" | null     | null             |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 20000                     | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 10000     | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.4           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "is_verzekerde_zorgtoeslag, hoogte_toeslag" of "zorgtoeslagwet"
    Then output "is_verzekerde_zorgtoeslag" is true
    And output "hoogte_toeslag" equals 198324

  Scenario: Persoon met studiefinanciering heeft recht op zorgtoeslag
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2004-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "DJI" data with key "bsn" for law "wet_forensische_zorg":
      | bsn         | zorgtype | juridische_titel |
      | "999993653" | null     | null             |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 15000                     | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.4           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "is_verzekerde_zorgtoeslag, hoogte_toeslag" of "zorgtoeslagwet"
    Then output "is_verzekerde_zorgtoeslag" is true
    And output "hoogte_toeslag" equals 198418

  Scenario: Alleenstaande met vermogen boven de grens heeft geen aanspraak op zorgtoeslag
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1998-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "DJI" data with key "bsn" for law "wet_forensische_zorg":
      | bsn         | zorgtype | juridische_titel |
      | "999993653" | null     | null             |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 20000                     | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 15000000  | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.4           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "voldoet_aan_voorwaarden, hoogte_toeslag" of "zorgtoeslagwet"
    Then output "voldoet_aan_voorwaarden" is false
    And output "hoogte_toeslag" is absent
