# De keuzemomenten van de burger, elk als effect op de uitkomst van de wet.
Feature: Keuzemomenten bij het terugbetalen van studieschuld

  Background:
    Given the calculation date is "2025-01-01"
    Given law "wet_studiefinanciering_2000" is loaded

  Scenario: Zonder aangevraagde draagkrachtmeting betaalt een SF15-oud-debiteur het wettelijke maandbedrag
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype          | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000003" | alleenstaand_met_kind | false         | 2004                           | wo             | false                    | 3200000          | 3200000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000003"        |
      | restschuld                   | 3000000            |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "draagkrachtmeting_van_toepassing" of "wet_studiefinanciering_2000"
    Then output "draagkrachtmeting_van_toepassing" is false
    # Wettelijk maandbedrag: annuïteit over EUR 30.000, 180 maanden, 2,21%.
    When I evaluate "te_betalen_maandbedrag" of "wet_studiefinanciering_2000"
    Then output "te_betalen_maandbedrag" equals 19597
    # Met aangevraagde draagkrachtmeting daalt het maandbedrag: het inkomen
    # (EUR 32.000) ligt boven de voet (100% belastbaar minimumloon voor een
    # alleenstaande ouder), maar de schijven dempen fors.
    Given the following parameters:
      | parameter               | value |
      | draagkracht_aangevraagd | true  |
    When I evaluate "draagkrachtmeting_van_toepassing" of "wet_studiefinanciering_2000"
    Then output "draagkrachtmeting_van_toepassing" is true
    When I evaluate "te_betalen_maandbedrag" of "wet_studiefinanciering_2000"
    Then output "te_betalen_maandbedrag" equals 2366

  Scenario: Overstap vanuit SF15-oud leidt onder huidig recht naar het 35-jaarsregime
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000001" | alleenstaand | false         | 2005                           | hbo            | false                    | 4500000          | 4500000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000001"        |
      | restschuld                   | 2500000            |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | true               |
    When I evaluate "terugbetaalregime" of "wet_studiefinanciering_2000"
    Then output "terugbetaalregime" equals "SF35"
    When I evaluate "terugbetaalperiode_maanden" of "wet_studiefinanciering_2000"
    Then output "terugbetaalperiode_maanden" equals 420
    # Draagkracht wordt automatisch toegepast en is veel lager dan onder
    # SF15-oud (4% boven 100% van het belastbaar minimumloon).
    When I evaluate "draagkrachtmeting_van_toepassing" of "wet_studiefinanciering_2000"
    Then output "draagkrachtmeting_van_toepassing" is true
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 5531

  Scenario: Peiljaarverlegging vereist een inkomensdaling van ten minste 15 procent
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000006" | alleenstaand | false         | 2019                           | hbo            | true                     | 2100000          | 1600000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000006"        |
      | restschuld                   | 900000             |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | true               |
      | overstap_aangevraagd         | false              |
    # Daling van 2100000 naar 1600000 is ruim 15%: verlegging mogelijk.
    When I evaluate "peiljaarverlegging_mogelijk" of "wet_studiefinanciering_2000"
    Then output "peiljaarverlegging_mogelijk" is true
    When I evaluate "gehanteerd_toetsingsinkomen" of "wet_studiefinanciering_2000"
    Then output "gehanteerd_toetsingsinkomen" equals 1600000
    # Onder de voet (84% belastbaar minimumloon): draagkracht nihil.
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 0

  Scenario: Peiljaarverlegging zonder voldoende inkomensdaling verandert niets
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000006" | alleenstaand | false         | 2019                           | hbo            | true                     | 2100000          | 2000000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000006"        |
      | restschuld                   | 900000             |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | true               |
      | overstap_aangevraagd         | false              |
    When I evaluate "peiljaarverlegging_mogelijk" of "wet_studiefinanciering_2000"
    Then output "peiljaarverlegging_mogelijk" is false
    When I evaluate "gehanteerd_toetsingsinkomen" of "wet_studiefinanciering_2000"
    Then output "gehanteerd_toetsingsinkomen" equals 2100000

  Scenario: Partnerinkomen niet laten meetellen geeft draagkracht nihil maar verlengt de aflosfase
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000002" | paar         | true          | 1990                           | hbo            | false                    | 800000           | 800000                   | 6200000                  |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000002"        |
      | restschuld                   | 1800000            |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | true               |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "partner_opt_out_mogelijk" of "wet_studiefinanciering_2000"
    Then output "partner_opt_out_mogelijk" is true
    # Met partnerinkomen meegeteld: forse draagkracht.
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 54098
    # Zonder partnerinkomen: eigen inkomen (EUR 8.000) onder de voet, dus nihil,
    # maar de aflosfase wordt elk jaar verlengd.
    Given the following parameters:
      | parameter         | value |
      | partner_meetellen | false |
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 0
    When I evaluate "partner_opt_out_verlengt_aflosfase" of "wet_studiefinanciering_2000"
    Then output "partner_opt_out_verlengt_aflosfase" is true
