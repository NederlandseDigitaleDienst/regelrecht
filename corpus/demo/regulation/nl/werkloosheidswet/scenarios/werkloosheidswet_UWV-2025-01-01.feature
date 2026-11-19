# Converted from sociale_zekerheid/werkloosheidswet_UWV-2025-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Berekening Werkloosheidsuitkering (WW)
  Als werknemer
  Wil ik weten of ik recht heb op WW
  Zodat ik mijn financiële situatie kan inschatten

  Background:
    Given the calculation date is "2025-02-01"

  Scenario: Werknemer met voldoende arbeidsverleden krijgt WW
    Given parameter "bsn" is "999100001"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100001" | {"pensioenleeftijd":67} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999100001" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                  |
      | "999100001" | {"gemiddeld_uren_per_week":40,"huidige_uren_per_week":0,"gewerkte_weken_36":30,"arbeidsverleden_jaren":10,"jaarloon":4200000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens | eu_inschrijving |
      | "999100001" | null                | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100001" | 1985-05-15    | null              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999100001" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "999100001" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "heeft_recht_op_ww, ww_duur_maanden, ww_uitkering_per_maand" of "werkloosheidswet"
    Then output "heeft_recht_op_ww" is true
    And output "ww_duur_maanden" equals 10
    And output "ww_uitkering_per_maand" equals 262500

  Scenario: Werknemer met te weinig arbeidsverleden krijgt geen WW
    Given parameter "bsn" is "999100002"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100002" | {"pensioenleeftijd":67} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999100002" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                 |
      | "999100002" | {"gemiddeld_uren_per_week":32,"huidige_uren_per_week":0,"gewerkte_weken_36":20,"arbeidsverleden_jaren":1,"jaarloon":2800000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens | eu_inschrijving |
      | "999100002" | null                | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100002" | 1998-03-20    | null              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999100002" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "999100002" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "heeft_recht_op_ww" of "werkloosheidswet"
    Then output "heeft_recht_op_ww" is false

  Scenario: Werknemer boven AOW-leeftijd krijgt geen WW
    Given parameter "bsn" is "999100003"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100003" | {"pensioenleeftijd":67} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999100003" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                  |
      | "999100003" | {"gemiddeld_uren_per_week":40,"huidige_uren_per_week":0,"gewerkte_weken_36":35,"arbeidsverleden_jaren":40,"jaarloon":5000000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens | eu_inschrijving |
      | "999100003" | null                | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100003" | 1955-08-10    | null              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999100003" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "999100003" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "heeft_recht_op_ww" of "werkloosheidswet"
    Then output "heeft_recht_op_ww" is false

  Scenario: Werknemer met ziektewet krijgt geen WW
    Given parameter "bsn" is "999100004"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100004" | {"pensioenleeftijd":67} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999100004" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                 |
      | "999100004" | {"gemiddeld_uren_per_week":36,"huidige_uren_per_week":0,"gewerkte_weken_36":32,"arbeidsverleden_jaren":8,"jaarloon":3800000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens | eu_inschrijving |
      | "999100004" | null                | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999100004" | 1990-11-25    | null              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "999100004" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                       |
      | "999100004" | {"heeft_ziektewet_uitkering":true} |
    When I evaluate outputs "heeft_recht_op_ww" of "werkloosheidswet"
    Then output "heeft_recht_op_ww" is false

  Scenario: Maximale WW-uitkering bij hoog inkomen
    Given parameter "bsn" is "100000005"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "100000005" | {"pensioenleeftijd":67} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "100000005" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                   |
      | "100000005" | {"gemiddeld_uren_per_week":40,"huidige_uren_per_week":0,"gewerkte_weken_36":36,"arbeidsverleden_jaren":15,"jaarloon":10000000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens | eu_inschrijving |
      | "100000005" | null                | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "100000005" | 1980-02-14    | null              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "100000005" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "100000005" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "heeft_recht_op_ww, ww_uitkering_per_maand, ww_duur_maanden" of "werkloosheidswet"
    Then output "heeft_recht_op_ww" is true
    And output "ww_uitkering_per_maand" equals 474155
    And output "ww_duur_maanden" equals 12

  Scenario: Deeltijd werkloosheid (minimaal 5 uur minder)
    Given parameter "bsn" is "100000006"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "100000006" | {"pensioenleeftijd":67} |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "100000006" | null   | null            |
    And the following "UWV" data with key "bsn" for law "uwv_werkgegevens":
      | bsn         | werkgegevens                                                                                                                  |
      | "100000006" | {"gemiddeld_uren_per_week":32,"huidige_uren_per_week":26,"gewerkte_weken_36":28,"arbeidsverleden_jaren":6,"jaarloon":3400000} |
    And the following "IND" data with key "bsn" for law "vreemdelingenwet":
      | bsn         | vergunning_gegevens | eu_inschrijving |
      | "100000006" | null                | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "100000006" | 1992-07-08    | null              | null        | []                | []             | NEDERLAND     | NEDERLANDS    | null  | []           |                       |
    And the following "UWV" data with key "bsn" for law "wet_werk_en_inkomen_naar_arbeidsvermogen":
      | bsn         | wia_uitkering_status |
      | "100000006" | null                 |
    And the following "UWV" data with key "bsn" for law "ziektewet":
      | bsn         | zw_uitkering                        |
      | "100000006" | {"heeft_ziektewet_uitkering":false} |
    When I evaluate outputs "heeft_recht_op_ww, ww_duur_maanden" of "werkloosheidswet"
    Then output "heeft_recht_op_ww" is true
    And output "ww_duur_maanden" equals 6
