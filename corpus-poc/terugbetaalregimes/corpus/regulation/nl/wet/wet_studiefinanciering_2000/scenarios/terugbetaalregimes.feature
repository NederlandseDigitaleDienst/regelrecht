# Scenario's per terugbetaalregime, peildatum 2025.
#
# Referentie: Stand van de Uitvoering OCW 2026, hoofdstuk 2. Het rekenvoorbeeld
# daar (schuld EUR 25.000, inkomen EUR 45.000, 180 maanden, rente 2,21%) noemt
# EUR 527,28 (SF15-oud) en EUR 244,41 (SF15-nieuw) als aflosbedrag naar
# draagkracht, berekend met DUO's interne (niet-openbare) voetbedragen. De
# waarden hieronder volgen de wettelijke afleiding: WML art. 8 lid 1 sub b
# (jan 2025: EUR 2.191,80) -> Besluit studiefinanciering 2000 art. 2 (108% x 12
# = belastbaar minimumloon EUR 28.405,73) -> voeten en percentages per regime.
# Dat geeft EUR 467,99 (SF15-oud) en EUR 211,39 (SF15-nieuw): dezelfde
# mechaniek en dezelfde conclusie (meer dan twee keer zo hoog), met een
# transparante herleiding.
#
# De discontofactor is (1 + 0,0221/12)^-180 = 0.7180650479405961; de engine
# kent geen machtsverheffen, de aanroeper rekent deze factor voor.
Feature: Terugbetaalregimes studieschuld

  Background:
    Given the calculation date is "2025-01-01"
    Given law "wet_studiefinanciering_2000" is loaded

  Scenario: SF15-oud met draagkrachtmeting (rekenvoorbeeld, wettelijke afleiding)
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000001" | alleenstaand | false         | 2005                           | hbo            | false                    | 4500000          | 4500000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000001"        |
      | restschuld                   | 2500000            |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | true               |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "terugbetaalregime" of "wet_studiefinanciering_2000"
    Then output "terugbetaalregime" equals "SF15_OUD"
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 46799
    When I evaluate "draagkrachtvrije_voet" of "wet_studiefinanciering_2000"
    Then output "draagkrachtvrije_voet" equals 1420287
    When I evaluate "wettelijk_maandbedrag" of "wet_studiefinanciering_2000"
    Then output "wettelijk_maandbedrag" equals 16331
    When I evaluate "te_betalen_maandbedrag" of "wet_studiefinanciering_2000"
    Then output "te_betalen_maandbedrag" equals 16331
    When I evaluate "terugbetaalperiode_maanden" of "wet_studiefinanciering_2000"
    Then output "terugbetaalperiode_maanden" equals 180

  Scenario: SF15-nieuw heeft mildere draagkrachtregels dan SF15-oud (rekenvoorbeeld)
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000010" | alleenstaand | false         | 2013                           | hbo            | false                    | 4500000          | 4500000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000010"        |
      | restschuld                   | 2500000            |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "terugbetaalregime" of "wet_studiefinanciering_2000"
    Then output "terugbetaalregime" equals "SF15_NIEUW"
    # Automatische draagkrachtmeting: ook zonder aanvraag van toepassing.
    When I evaluate "draagkrachtmeting_van_toepassing" of "wet_studiefinanciering_2000"
    Then output "draagkrachtmeting_van_toepassing" is true
    # 12% boven 84% van het belastbaar minimumloon: minder dan de helft van
    # de SF15-oud-schijvenuitkomst (46799).
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 21139
    When I evaluate "draagkrachtvrije_voet" of "wet_studiefinanciering_2000"
    Then output "draagkrachtvrije_voet" equals 2386081

  Scenario: SF35 rekent 4 procent boven 100 procent van het belastbaar minimumloon
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000020" | alleenstaand | false         | 2018                           | wo             | false                    | 4500000          | 4500000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000020"        |
      | restschuld                   | 4000000            |
      | resterende_maanden           | 420                |
      | discontofactor               | 0.4617237046355462 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "terugbetaalregime" of "wet_studiefinanciering_2000"
    Then output "terugbetaalregime" equals "SF35"
    When I evaluate "terugbetaalperiode_maanden" of "wet_studiefinanciering_2000"
    Then output "terugbetaalperiode_maanden" equals 420
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 5531
    When I evaluate "draagkrachtvrije_voet" of "wet_studiefinanciering_2000"
    Then output "draagkrachtvrije_voet" equals 2840573

  Scenario: Levenlanglerenkrediet kent geen aflosvrije periode
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000030" | alleenstaand | false         | 2019                           | hbo            | true                     | 4500000          | 4500000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000030"        |
      | restschuld                   | 900000             |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7180650479405961 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "terugbetaalregime" of "wet_studiefinanciering_2000"
    Then output "terugbetaalregime" equals "SF15_LLLK"
    When I evaluate "aflosvrije_periode_toegestaan" of "wet_studiefinanciering_2000"
    Then output "aflosvrije_periode_toegestaan" is false
    When I evaluate "terugbetaalperiode_maanden" of "wet_studiefinanciering_2000"
    Then output "terugbetaalperiode_maanden" equals 180
    When I evaluate "termijn_naar_draagkracht" of "wet_studiefinanciering_2000"
    Then output "termijn_naar_draagkracht" equals 21139

  Scenario: Rentepercentages 2026 verschillen per regime
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | huishoudtype | heeft_partner | eerste_studiefinanciering_jaar | onderwijssoort | is_levenlanglerenkrediet | toetsingsinkomen | toetsingsinkomen_actueel | toetsingsinkomen_partner |
      | "100000001" | alleenstaand | false         | 2005                           | hbo            | false                    | 4500000          | 4500000                  | 0                        |
      | "100000020" | alleenstaand | false         | 2018                           | wo             | false                    | 4500000          | 4500000                  | 0                        |
    Given the following parameters:
      | parameter                    | value              |
      | bsn                          | "100000001"        |
      | restschuld                   | 2500000            |
      | resterende_maanden           | 180                |
      | discontofactor               | 0.7095156959093744 |
      | draagkracht_aangevraagd      | false              |
      | partner_meetellen            | true               |
      | peiljaarverlegging_toegepast | false              |
      | overstap_aangevraagd         | false              |
    When I evaluate "rentepercentage" of "wet_studiefinanciering_2000"
    Then output "rentepercentage" equals 0.0229
    Given parameter "bsn" is "100000020"
    When I evaluate "rentepercentage" of "wet_studiefinanciering_2000"
    Then output "rentepercentage" equals 0.0233
