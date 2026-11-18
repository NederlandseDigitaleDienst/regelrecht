# Converted from sociale_zekerheid/besluit_bijstandverlening_zelfstandigen_SZW-2025-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen recht op bijstand voor zelfstandigen (Bbz 2004)
  Als zelfstandige ondernemer
  Wil ik weten of ik recht heb op bijstand volgens de Bbz 2004
  Zodat ik weet welke financiële ondersteuning ik kan krijgen

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"

  Scenario: Gevestigde zelfstandige met levensvatbaar bedrijf krijgt bijstand
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                         |
      | "999993653" | {"type_zelfstandige":"GEVESTIGD","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":5,"uren_per_week":40,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                              | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Thuiszorg"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000000   | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige, max_duur_maanden, bedrijfskapitaal_max, bedrijfskapitaal_type" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "GEVESTIGD"
    And output "max_duur_maanden" equals 12
    And output "bedrijfskapitaal_max" equals 25342000
    And output "bedrijfskapitaal_type" equals "LENING_RENTE"

  # Was "Gevestigde zelfstandige met te hoog vermogen krijgt geen bijstand": art. 2 lid 1 onder a
  # stelt geen vermogenstoets voor algemene bijstand (zie audit getrouwheid); lid 2 beperkt het
  # vermogen alleen voor de categorie bedrijfskapitaal. Levensvatbaar bedrijf, gevestigde
  # zelfstandige: voldoet, ongeacht het vermogen.
  Scenario: Gevestigde zelfstandige krijgt bijstand ongeacht vermogen (vermogen is geen lid-1-toets)
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                          |
      | "999993653" | {"type_zelfstandige":"GEVESTIGD","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":10,"uren_per_week":50,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                                 | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Adviesbureau"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 250000000 | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true

  Scenario: Gevestigde zelfstandige met niet-levensvatbaar bedrijf krijgt geen bijstand
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                          |
      | "999993653" | {"type_zelfstandige":"GEVESTIGD","bedrijf_levensvatbaar":false,"jaren_ondernemerschap":3,"uren_per_week":35,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                            | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Webshop"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 1000000   | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Beginnende zelfstandige na WW-uitkering krijgt 36 maanden bijstand
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999993653" | {"pensioenleeftijd":67} |
    And the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                         |
      | "999993653" | {"type_zelfstandige":"BEGINNEND","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":0,"uren_per_week":30,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                                     | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Grafisch ontwerp"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                 |
      | "999993653" | {"gemiddeld_uren_per_week":40,"huidige_uren_per_week":0,"gewerkte_weken_36":30,"arbeidsverleden_jaren":5,"jaarloon":3600000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 500000    | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "999993653" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige, max_duur_maanden, bedrijfskapitaal_max, bedrijfskapitaal_type" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "BEGINNEND"
    And output "max_duur_maanden" equals 36
    And output "bedrijfskapitaal_max" equals 4665600
    And output "bedrijfskapitaal_type" equals "LENING_RENTE"

  Scenario: Beginnende zelfstandige zonder WW-uitkering krijgt geen bijstand
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999993653" | {"pensioenleeftijd":67} |
    And the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                         |
      | "999993653" | {"type_zelfstandige":"BEGINNEND","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":0,"uren_per_week":25,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                               | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Fotografie"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                         |
      | "999993653" | {"gemiddeld_uren_per_week":0,"huidige_uren_per_week":0,"gewerkte_weken_36":0,"arbeidsverleden_jaren":0,"jaarloon":0} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | GEEN              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 500000    | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "999993653" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Oudere zelfstandige (geboren voor 1960) met niet-levensvatbaar bedrijf krijgt onbeperkte bijstand
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                       |
      | "999993653" | {"type_zelfstandige":"OUDER","bedrijf_levensvatbaar":false,"jaren_ondernemerschap":25,"uren_per_week":30,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                                  | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Timmerbedrijf"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-06-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 10000000  | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige, max_duur_maanden, bedrijfskapitaal_max, bedrijfskapitaal_type" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "OUDER"
    And output "max_duur_maanden" equals 0
    And output "bedrijfskapitaal_max" equals 1267100
    And output "bedrijfskapitaal_type" equals "OM_NIET"

  # Was "... met te hoog vermogen krijgt geen bijstand": lid 1 stelt geen vermogenstoets voor
  # algemene bijstand (die stond hier ten onrechte, zie audit getrouwheid); lid 2 beperkt het
  # vermogen alleen voor de categorie bedrijfskapitaal (dat scenario blijft ongewijzigd bij
  # bedrijfskapitaal_max). Deze zelfstandige voldoet aan de overige onderdeel-c-voorwaarden en
  # krijgt dus wel bijstand, ongeacht het vermogen.
  Scenario: Oudere zelfstandige krijgt bijstand ongeacht vermogen (vermogen is geen lid-1-toets)
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                       |
      | "999993653" | {"type_zelfstandige":"OUDER","bedrijf_levensvatbaar":false,"jaren_ondernemerschap":30,"uren_per_week":25,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                                     | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Schildersbedrijf"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-06-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 180000000 | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "OUDER"

  Scenario: Oudere zelfstandige geboren na 1960 komt niet in aanmerking als oudere
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                       |
      | "999993653" | {"type_zelfstandige":"OUDER","bedrijf_levensvatbaar":false,"jaren_ondernemerschap":20,"uren_per_week":30,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                               | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Loodgieter"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1965-03-20    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000000   | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Oudere zelfstandige met minder dan 10 jaar ondernemerschap komt niet in aanmerking
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                      |
      | "999993653" | {"type_zelfstandige":"OUDER","bedrijf_levensvatbaar":false,"jaren_ondernemerschap":8,"uren_per_week":35,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                             | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Bakkerij"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-06-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 5000000   | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Beeindigende zelfstandige krijgt uitloopbijstand zonder bedrijfskapitaal
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                                    |
      | "999993653" | {"type_zelfstandige":"BEEINDIGEND","bedrijf_levensvatbaar":false,"jaren_ondernemerschap":7,"uren_per_week":25,"beeindigingsdatum":"2025-12-01"} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                               | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Restaurant"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1980-01-01    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 2000000   | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige, max_duur_maanden, bedrijfskapitaal_max, bedrijfskapitaal_type" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "BEEINDIGEND"
    And output "max_duur_maanden" equals 12
    And output "bedrijfskapitaal_max" equals 0
    And output "bedrijfskapitaal_type" equals "GEEN"

  # Was "... voldoet niet aan urencriterium": art. 2 stelt geen urencriterium (dit bestand
  # bevat geen ander artikel dat het stelt, zie audit getrouwheid). Uren_per_week is verder
  # onbelangrijk voor lid 1 onder a; deze zelfstandige voldoet aan de overige voorwaarden.
  Scenario: Gevestigde zelfstandige krijgt bijstand ongeacht aantal gewerkte uren
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                         |
      | "999993653" | {"type_zelfstandige":"GEVESTIGD","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":5,"uren_per_week":20,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                               | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Consultant"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 500000    | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "GEVESTIGD"

  Scenario: Persoon zonder actieve onderneming krijgt geen Bbz
    Given the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                         |
      | "999993653" | {"type_zelfstandige":"GEVESTIGD","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":3,"uren_per_week":40,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                              | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"OPGEHEVEN","activiteit":"Winkel"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1985-01-01    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 500000    | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering |
      | "999993653" | null         |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is false

  # Art. 2 lid 1 onder b: "de persoon of de echtgenoot van de persoon die uit hoofde van
  # werkloosheid een uitkering ontvangt" - de aanvrager zelf heeft geen WW, de echtgenoot wel.
  Scenario: Beginnende zelfstandige zonder eigen WW maar met echtgenoot met WW krijgt bijstand
    Given the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999993653" | {"pensioenleeftijd":67} |
      | "999993872" | {"pensioenleeftijd":67} |
    And the following "SZW" data with key "bsn" for law "besluit_bijstandverlening_zelfstandigen":
      | bsn         | bbz_aanvraag                                                                                                                         |
      | "999993653" | {"type_zelfstandige":"BEGINNEND","bedrijf_levensvatbaar":true,"jaren_ondernemerschap":0,"uren_per_week":30,"beeindigingsdatum":null} |
    And the following "KVK" data with key "bsn" for law "handelsregisterwet":
      | bsn         | inschrijvingen                                                                                     | posities |
      | "999993653" | [{"kvk_nummer":null,"rechtsvorm":"EENMANSZAAK","status":"ACTIEF","activiteit":"Grafisch ontwerp"}] | []       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
      | "999993872" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                 |
      | "999993653" | {"gemiddeld_uren_per_week":0,"huidige_uren_per_week":0,"gewerkte_weken_36":0,"arbeidsverleden_jaren":0,"jaarloon":0}         |
      | "999993872" | {"gemiddeld_uren_per_week":40,"huidige_uren_per_week":0,"gewerkte_weken_36":30,"arbeidsverleden_jaren":5,"jaarloon":3600000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens                                                                                  | eu_inschrijving |
      | "999993653" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
      | "999993872" | {"type":"ONBEPAALDE_TIJD_REGULIER","status":"VERLEEND","ingangsdatum":"2015-01-01","einddatum":null} | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | HUWELIJK          | "999993872" | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           | 1988-01-01            |
      | "999993872" | 1988-01-01    | HUWELIJK          | "999993653" | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           | 1990-01-01            |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 500000    | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
      | "999993872" | 20.5           |
    And the following "DUO" data with key "bsn" for law "wet_studiefinanciering":
      | bsn         | onderwijstype | aantal_studerend_gezin | partner_onderwijstype | partner_aantal_studerend_gezin |
      | "999993653" | null          | 0                      | null                  | 0                              |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999993653" | null                 |
      | "999993872" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "999993653" | {"heeft_ziektewet_uitkering":false} |
      | "999993872" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "voldoet_aan_voorwaarden, categorie_zelfstandige, max_duur_maanden" of "besluit_bijstandverlening_zelfstandigen"
    Then output "voldoet_aan_voorwaarden" is true
    And output "categorie_zelfstandige" equals "BEGINNEND"
    And output "max_duur_maanden" equals 36
