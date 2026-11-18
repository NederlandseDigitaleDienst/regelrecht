Feature: Zorgtoeslag vanaf de maand na de achttiende verjaardag
  Art. 1 lid 1 onder c van de Wet op de zorgtoeslag: een verzekerde telt mee
  "steeds vanaf de eerste dag van de kalendermaand volgende op de maand waarin
  hij achttien jaar wordt". Niet de leeftijd op de rekendatum beslist, maar de
  maand van de verjaardag.

  Background:
    Given the calculation date is "2025-02-01"
    And parameter "bsn" is "999993653"

  Scenario: Wie in de maand van de rekendatum 18 wordt, telt pas de maand erna mee
    # Achttien op 1 februari 2025 zelf, verzekerde voor de zorgtoeslag vanaf 1 maart 2025.
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | VRIJ   | GEEN            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2007-02-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "zorgtoeslagwet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Wie in de maand voor de rekendatum 18 werd, telt mee
    # Achttien op 31 januari 2025, verzekerde voor de zorgtoeslag vanaf 1 februari 2025.
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | VRIJ   | GEEN            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2007-01-31    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "zorgtoeslagwet"
    Then output "voldoet_aan_voorwaarden" is true

  Scenario: Over de jaargrens: wie in december 18 werd, telt in januari mee
    # Rekendatum 1 januari: de vorige maand is december van het jaar ervoor.
    Given the calculation date is "2025-01-01"
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | VRIJ   | GEEN            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2006-12-31    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "zorgtoeslagwet"
    Then output "voldoet_aan_voorwaarden" is true
