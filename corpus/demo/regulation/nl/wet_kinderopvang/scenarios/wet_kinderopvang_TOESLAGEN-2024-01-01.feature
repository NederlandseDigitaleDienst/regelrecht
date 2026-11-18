# Converted from toeslagen/wet_kinderopvang_TOESLAGEN-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Berekening Kinderopvangtoeslag
  Als ouder
  Wil ik weten of ik recht heb op kinderopvangtoeslag
  Zodat ik de juiste toeslag kan ontvangen

  Background:
    Given the calculation date is "2025-01-15"
    And parameter "bsn" is "999888888"

  Scenario: Alleenstaande ouder met jonge kinderen heeft recht op kinderopvangtoeslag
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                                                                   | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1990-05-15    | GEEN              | null        | [{"bsn":"999111111","geboortedatum":"2019-03-01"},{"bsn":"999222222","geboortedatum":"2021-06-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 3600000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                                 | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2024-01-15","end_date":"2024-01-30"}] | []                 |
    # POC phase dropped: "wet_kinderopvang" was run only to establish that required data was missing (ontbreken er verplichte gegevens; is niet voldaan aan de voorwaarden)
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    Given the following parameters:
      | kinderopvang_kvk       | "12345678"                                                                                                                                                                          |
      | is_geregistreerd       | true                                                                                                                                                                                |
      | aanvraag_soort_opvang  | DAGOPVANG                                                                                                                                                                           |
      | binnen_aanloopperiode  | false                                                                                                                                                                               |
      | aangegeven_uren        | [{"kind_bsn":"999111111","uren_per_jaar":2000,"uurtarief":850,"soort_opvang":"DAGOPVANG"},{"kind_bsn":"999222222","uren_per_jaar":1500,"uurtarief":900,"soort_opvang":"DAGOPVANG"}] |
      | verwachte_partner_uren | 0                                                                                                                                                                                   |
    When I evaluate outputs "is_gerechtigd, jaarbedrag" of "wet_kinderopvang"
    # POC: no required inputs missing (application-level check, not engine behaviour)
    Then output "is_gerechtigd" is true
    And output "jaarbedrag" equals 2440000

  Scenario: Tweeverdieners met hoger inkomen ontvangen lagere toeslag
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                  | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1985-03-10    | HUWELIJK          | "999999999" | [{"bsn":"333333333","geboortedatum":"2019-03-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
      | "999999999" |               | null              | null        | []                                                 | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 4500000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "999999999" | 3800000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
      | "999999999" | 20.5           |
    # Art. 33 Wet SUWI: een lopend dienstverband heeft geen einddatum (null); de POC-lege tekst "" is geen datum
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                         | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2023-01-01","end_date":null}] | []                 |
      | "999999999" | [{"start_date":"2023-01-01","end_date":null}] | []                 |
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    And the following parameters:
      | kinderopvang_kvk       | "87654321"                                                                                                                     |
      | is_geregistreerd       | true                                                                                                                           |
      | aanvraag_soort_opvang  | DAGOPVANG                                                                                                                      |
      | binnen_aanloopperiode  | false                                                                                                                          |
      | aangegeven_uren        | [{"kind_bsn":"333333333","uren_per_jaar":2500,"uurtarief":895,"soort_opvang":"DAGOPVANG","LRK_registratienummer":"123456789"}] |
      | verwachte_partner_uren | 30                                                                                                                             |
    When I evaluate outputs "is_gerechtigd, jaarbedrag" of "wet_kinderopvang"
    Then output "is_gerechtigd" is true
    And output "jaarbedrag" equals 738375

  Scenario: Ouder met BSO en overschrijding van het maximale uurtarief
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                                                                   | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1988-11-21    | GEEN              | null        | [{"bsn":"444444444","geboortedatum":"2019-03-01"},{"bsn":"555555555","geboortedatum":"2021-06-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 3200000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
    # Art. 33 Wet SUWI: een lopend dienstverband heeft geen einddatum (null); de POC-lege tekst "" is geen datum
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                         | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2023-03-01","end_date":null}] | []                 |
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    And the following parameters:
      | kinderopvang_kvk       | "23456789"                                                                                                                                                                                                                                      |
      | is_geregistreerd       | true                                                                                                                                                                                                                                            |
      | aanvraag_soort_opvang  | DAGOPVANG                                                                                                                                                                                                                                       |
      | binnen_aanloopperiode  | false                                                                                                                                                                                                                                           |
      | aangegeven_uren        | [{"kind_bsn":"444444444","uren_per_jaar":1200,"uurtarief":790,"soort_opvang":"BSO","LRK_registratienummer":"234567890"},{"kind_bsn":"555555555","uren_per_jaar":1200,"uurtarief":790,"soort_opvang":"BSO","LRK_registratienummer":"234567890"}] |
      | verwachte_partner_uren | 0                                                                                                                                                                                                                                               |
    When I evaluate outputs "is_gerechtigd, jaarbedrag" of "wet_kinderopvang"
    Then output "is_gerechtigd" is true
    And output "jaarbedrag" equals 1764864

  Scenario: Partner werkt minder dan vereiste uren, geen recht op toeslag
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                  | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1990-07-05    | HUWELIJK          | "999777777" | [{"bsn":"666666666","geboortedatum":"2019-03-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
      | "999777777" |               | null              | null        | []                                                 | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 2900000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "999777777" | 1200000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
      | "999777777" | 20.5           |
    # Art. 33 Wet SUWI: een lopend dienstverband heeft geen einddatum (null); de POC-lege tekst "" is geen datum
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                                 | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2022-09-01","end_date":null}]         | []                 |
      | "999777777" | [{"start_date":"2023-01-01","end_date":"2023-01-15"}] | []                 |
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    And the following parameters:
      | kinderopvang_kvk       | "34567890"                                                                                                                     |
      | is_geregistreerd       | true                                                                                                                           |
      | aanvraag_soort_opvang  | DAGOPVANG                                                                                                                      |
      | binnen_aanloopperiode  | false                                                                                                                          |
      | aangegeven_uren        | [{"kind_bsn":"666666666","uren_per_jaar":1800,"uurtarief":880,"soort_opvang":"DAGOPVANG","LRK_registratienummer":"345678901"}] |
      | verwachte_partner_uren | 15                                                                                                                             |
    When I evaluate outputs "is_gerechtigd" of "wet_kinderopvang"
    Then output "is_gerechtigd" is false

  Scenario: Gezin met meerdere soorten opvang
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                                                                   | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1987-02-18    | HUWELIJK          | "888888880" | [{"bsn":"888888881","geboortedatum":"2019-03-01"},{"bsn":"888888882","geboortedatum":"2021-06-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
      | "888888880" |               | null              | null        | []                                                                                                  | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 2800000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
      | "888888880" | 2100000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
      | "888888880" | 20.5           |
    # Art. 33 Wet SUWI: een lopend dienstverband heeft geen einddatum (null); de POC-lege tekst "" is geen datum
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                         | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2020-02-01","end_date":null}] | []                 |
      | "888888880" | [{"start_date":"2020-02-01","end_date":null}] | []                 |
    # POC: the citizen submitted these values as claims; they override the inputs of the same name
    And the following parameters:
      | kinderopvang_kvk       | "56789012"                                                                                                                                                                                                                                            |
      | is_geregistreerd       | true                                                                                                                                                                                                                                                  |
      | aanvraag_soort_opvang  | DAGOPVANG                                                                                                                                                                                                                                             |
      | binnen_aanloopperiode  | false                                                                                                                                                                                                                                                 |
      | aangegeven_uren        | [{"kind_bsn":"888888881","uren_per_jaar":2000,"uurtarief":899,"soort_opvang":"DAGOPVANG","LRK_registratienummer":"567890123"},{"kind_bsn":"888888882","uren_per_jaar":1000,"uurtarief":750,"soort_opvang":"BSO","LRK_registratienummer":"567890124"}] |
      | verwachte_partner_uren | 30                                                                                                                                                                                                                                                    |
    When I evaluate outputs "is_gerechtigd, jaarbedrag" of "wet_kinderopvang"
    Then output "is_gerechtigd" is true
    And output "jaarbedrag" equals 2038400

  Scenario: Opvang niet geregistreerd in het LRK geeft geen aanspraak
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                  | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1990-05-15    | GEEN              | null        | [{"bsn":"999111111","geboortedatum":"2019-03-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 3600000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                                 | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2024-01-15","end_date":"2024-01-30"}] | []                 |
    # Art. 1.5 lid 1 Wko: de opvang is niet geregistreerd in het LRK, dus geen aanspraak,
    # ongeacht de overige voorwaarden.
    And the following parameters:
      | kinderopvang_kvk       | "45678901"                                                                                 |
      | is_geregistreerd       | false                                                                                      |
      | aanvraag_soort_opvang  | DAGOPVANG                                                                                  |
      | binnen_aanloopperiode  | false                                                                                      |
      | aangegeven_uren        | [{"kind_bsn":"999111111","uren_per_jaar":2000,"uurtarief":850,"soort_opvang":"DAGOPVANG"}] |
      | verwachte_partner_uren | 0                                                                                          |
    When I evaluate outputs "is_gerechtigd" of "wet_kinderopvang"
    Then output "is_gerechtigd" is false

  Scenario: Ouderparticipatiecrèche in de aanloopperiode geeft geen aanspraak
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens                                  | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999888888" | 1990-05-15    | GEEN              | null        | [{"bsn":"999111111","geboortedatum":"2019-03-01"}] | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999888888" | 3600000                   | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999888888" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes                                 | uitkeringsperiodes |
      | "999888888" | [{"start_date":"2024-01-15","end_date":"2024-01-30"}] | []                 |
    # Art. 1.5 lid 2 Wko: ouderparticipatieopvang in een ouderparticipatiecrèche binnen de
    # aanloopperiode van art. 1.60b geeft, in afwijking van het eerste lid, geen aanspraak - ook
    # niet als de opvang zelf wel geregistreerd is.
    And the following parameters:
      | kinderopvang_kvk       | "56789013"                                                                                 |
      | is_geregistreerd       | true                                                                                       |
      | aanvraag_soort_opvang  | OUDERPARTICIPATIECRECHE                                                                    |
      | binnen_aanloopperiode  | true                                                                                       |
      | aangegeven_uren        | [{"kind_bsn":"999111111","uren_per_jaar":2000,"uurtarief":850,"soort_opvang":"DAGOPVANG"}] |
      | verwachte_partner_uren | 0                                                                                          |
    When I evaluate outputs "is_gerechtigd" of "wet_kinderopvang"
    Then output "is_gerechtigd" is false
