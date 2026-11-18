Feature: Bepalen recht op bijstand landelijk
  Als burger
  Wil ik weten of ik recht heb op algemene bijstand
  Zodat ik weet of ik financiële ondersteuning kan krijgen

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Alleenstaande met recht op bijstand heeft kostendelersnorm 1.0
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []           |                       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000      | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, kostendelersnorm" of "participatiewet/bijstand"
    Then output "voldoet_aan_voorwaarden" is true
    And output "kostendelersnorm" equals 1.0

  Scenario: Huishouden van drie kostendelende personen heeft kostendelersnorm 0.43
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners                              | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | [{"bsn":"999993655"},{"bsn":"999993656"}] |                       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000      | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "kostendelersnorm" of "participatiewet/bijstand"
    Then output "kostendelersnorm" equals 0.43

  Scenario: Huishouden van vijf of meer kostendelende personen heeft geen bekende kostendelersnorm
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners                                                                      | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | [{"bsn":"999993655"},{"bsn":"999993656"},{"bsn":"999993657"},{"bsn":"999993658"}] |                       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000      | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "kostendelersnorm" of "participatiewet/bijstand"
    Then output "kostendelersnorm" is absent

  Scenario: Vreemdeling zonder EU-, permanente of nareis-vergunning en zonder AMvB-gelijkstelling heeft geen recht
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      # Art. 11 lid 2 Pw gaat over een vreemdeling: de nationaliteit is bekend en niet Nederlands.
      # Een lege cel zou onbekend zijn en de uitkomst onbekend maken in plaats van false.
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             |               | TURKS         | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []           |                       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                  | eu_inschrijving |
      | "999993653" | {"type":"TIJDELIJK_ARBEID","status":"VERLEEND","ingangsdatum":null,"einddatum":null} | null            |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "participatiewet/bijstand"
    Then output "voldoet_aan_voorwaarden" is false
