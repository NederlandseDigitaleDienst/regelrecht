# Converted from toeslagen/wet_op_de_huurtoeslag_TOESLAGEN-2025-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Berekening Huurtoeslag
  Als burger
  Wil ik weten of ik recht heb op huurtoeslag
  Zodat ik de juiste toeslag kan ontvangen

  Background:
    Given the calculation date is "2025-02-01"

  Scenario: Persoon onder 18 heeft geen recht op huurtoeslag
    Given parameter "bsn" is "999111111"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999111111" | 2008-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999111111" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999111111" | 20.5           |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "wet_op_de_huurtoeslag"
    Then output "voldoet_aan_voorwaarden" is false

  # Art. 19 lid 3: de normhuur wordt naar boven afgerond op hele euro's. Bij een
  # inkomen tussen de twee ankerpunten (lid 2) levert de lineaire interpolatie
  # een niet-hele-euro normhuur op (hier 355,6435... euro); de wet schrijft
  # afronding naar boven voor, dus 356 euro (35600 eurocent), niet 355 euro.
  Scenario: Basishuur wordt naar boven afgerond op hele euro's
    Given parameter "bsn" is "999444444"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999444444" | 1985-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999444444" | 2500000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999444444" | 20.5           |
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    And the following parameters:
      | huurprijs                  | 60000 |
      | servicekosten              | 5000  |
      | subsidiabele_servicekosten | 4800  |
    When I evaluate outputs "basishuur, subsidiebedrag" of "wet_op_de_huurtoeslag"
    Then output "basishuur" equals 35600
    And output "subsidiebedrag" equals 23222

  Scenario: Alleenstaande met laag inkomen en hogere huur
    Given parameter "bsn" is "999222222"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999222222" | 1990-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999222222" | 1400000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999222222" | 20.5           |
    # POC phase dropped: "wet_op_de_huurtoeslag" was run only to establish that required data was missing (ontbreken er verplichte gegevens; is niet voldaan aan de voorwaarden)
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    Given the following parameters:
      | huurprijs                  | 72000 |
      | servicekosten              | 5000  |
      | subsidiabele_servicekosten | 4800  |
    When I evaluate outputs "voldoet_aan_voorwaarden, subsidiebedrag" of "wet_op_de_huurtoeslag"
    # POC: no required inputs missing (application-level check, not engine behaviour)
    Then output "voldoet_aan_voorwaarden" is true
    And output "subsidiebedrag" equals 40695

  Scenario: Te hoog inkomen voor huurtoeslag
    Given parameter "bsn" is "333333333"
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "333333333" | 1980-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "333333333" | 4500000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "333333333" | 20.5           |
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    And the following parameters:
      | huurprijs                  | 65000 |
      | servicekosten              | 5000  |
      | subsidiabele_servicekosten | 4800  |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "wet_op_de_huurtoeslag"
    Then output "voldoet_aan_voorwaarden" is false
