# Converted from sociale_zekerheid/participatiewet_bijstand_gemeenten_GEMEENTE_AMSTERDAM-2023-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen recht op bijstand Amsterdam
  Als inwoner van Amsterdam
  Wil ik weten of ik recht heb op bijstand
  Zodat ik weet of ik financiële ondersteuning kan krijgen

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Standaard bijstandsuitkering voor alleenstaande
    Given the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen | posities |
      | "999993653" | []             | []       |
    And the following "GEMEENTE_AMSTERDAM" data with key "bsn" for law "participatiewet/bijstand/amsterdam":
      | bsn         | arbeidsvermogen                                                                                                        |
      | "999993653" | {"arbeidsvermogen":"VOLLEDIG","ontheffing_reden":null,"ontheffing_einddatum":null,"re_integratie_traject":"Werkstage"} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000      | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, uitkeringsbedrag" of "participatiewet/bijstand/amsterdam"
    Then output "voldoet_aan_voorwaarden" is true
    And output "uitkeringsbedrag" equals 108900

  Scenario: Dakloze met briefadres krijgt extra woonkostentoeslag
    Given the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen | posities |
      | "999993653" | []             | []       |
    And the following "GEMEENTE_AMSTERDAM" data with key "bsn" for law "participatiewet/bijstand/amsterdam":
      | bsn         | arbeidsvermogen                                                                                                        |
      | "999993653" | {"arbeidsvermogen":"VOLLEDIG","ontheffing_reden":null,"ontheffing_einddatum":null,"re_integratie_traject":"Werkstage"} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                             | medebewoners | partner_geboortedatum |
      | "999993653" | 1980-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"De Regenboog Groep","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"BRIEFADRES"} | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, uitkeringsbedrag, woonkostentoeslag" of "participatiewet/bijstand/amsterdam"
    Then output "voldoet_aan_voorwaarden" is true
    And output "uitkeringsbedrag" equals 108900
    And output "woonkostentoeslag" equals 60000

  Scenario: ZZP-er met laag inkomen krijgt aanvullende bijstand en startkapitaal
    Given the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                              | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Webdesign"}] | []       |
    And the following "GEMEENTE_AMSTERDAM" data with key "bsn" for law "participatiewet/bijstand/amsterdam":
      | bsn         | arbeidsvermogen                                                                                                                   |
      | "999993653" | {"arbeidsvermogen":"VOLLEDIG","ontheffing_reden":null,"ontheffing_einddatum":null,"re_integratie_traject":"Zelfstandigentraject"} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 50000                 | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, uitkeringsbedrag, startkapitaal" of "participatiewet/bijstand/amsterdam"
    Then output "voldoet_aan_voorwaarden" is true
    And output "uitkeringsbedrag" equals 108969
    And output "startkapitaal" equals 200000

  Scenario: Persoon met medische ontheffing krijgt bijstand zonder re-integratieverplichting
    Given the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen | posities |
      | "999993653" | []             | []       |
    And the following "GEMEENTE_AMSTERDAM" data with key "bsn" for law "participatiewet/bijstand/amsterdam":
      | bsn         | arbeidsvermogen                                                                                                                                |
      | "999993653" | {"arbeidsvermogen":"MEDISCH_VOLLEDIG","ontheffing_reden":"Chronische ziekte","ontheffing_einddatum":"2026-01-01","re_integratie_traject":null} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners | partner_geboortedatum |
      | "999993653" | 1975-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden, uitkeringsbedrag" of "participatiewet/bijstand/amsterdam"
    Then output "voldoet_aan_voorwaarden" is true
    And output "uitkeringsbedrag" equals 108900

  Scenario: ZZP-er met voldoende inkomen krijgt geen bijstand
    Given the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                              | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Thuiszorg"}] | []       |
    And the following "GEMEENTE_AMSTERDAM" data with key "bsn" for law "participatiewet/bijstand/amsterdam":
      | bsn         | arbeidsvermogen |
      | "999993653" | null            |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres                                                                                                      | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 1550000               | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "participatiewet/bijstand/amsterdam"
    Then output "voldoet_aan_voorwaarden" is false
