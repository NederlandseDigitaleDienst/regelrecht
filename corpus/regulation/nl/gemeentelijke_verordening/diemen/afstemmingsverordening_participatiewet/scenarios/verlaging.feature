Feature: Verlaging bijstand bij verwijtbare gedraging (Diemen)

  # Art. 7 leidt de gedragscategorie af uit de feitelijke criteria van lid
  # a t/m c. Art. 9 leest die categorie (intra-law) en stelt het
  # verlagingspercentage en de duur vast. Zonder een van de gedragingen is er
  # geen categorie: dat is afwezigheid, geen categorie "0" (RFC-036).

  Background:
    Given the calculation date is "2024-06-01"
    Given law "afstemmingsverordening_participatiewet_diemen" is loaded

  Scenario: Niet tijdig geregistreerd bij UWV geeft categorie 1
    # Art. 7 onderdeel a
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | true        |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | false       |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | false       |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | false       |
    When I evaluate "gedragscategorie" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "gedragscategorie" equals 1
    When I evaluate "verlaging_percentage" of "afstemmingsverordening_participatiewet_diemen"
    Then output "verlaging_percentage" equals 5
    When I evaluate "duur_maanden" of "afstemmingsverordening_participatiewet_diemen"
    Then output "duur_maanden" equals 1

  Scenario: Niet meewerken aan het plan van aanpak geeft categorie 2
    # Art. 7 onderdeel b, sub 1
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | false       |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | true        |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | false       |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | false       |
    When I evaluate "gedragscategorie" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "gedragscategorie" equals 2
    When I evaluate "verlaging_percentage" of "afstemmingsverordening_participatiewet_diemen"
    Then output "verlaging_percentage" equals 30

  Scenario: Tegenprestatie niet verricht geeft ook categorie 2
    # Art. 7 onderdeel b, sub 4 - elk van de vier sub-gedragingen leidt op
    # zichzelf al tot categorie 2.
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | false       |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | false       |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | true        |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | false       |
    When I evaluate "gedragscategorie" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "gedragscategorie" equals 2

  Scenario: Niet naar vermogen arbeid getracht te verkrijgen geeft categorie 3
    # Art. 7 onderdeel c
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | false       |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | false       |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | false       |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | true        |
    When I evaluate "gedragscategorie" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "gedragscategorie" equals 3
    When I evaluate "verlaging_percentage" of "afstemmingsverordening_participatiewet_diemen"
    Then output "verlaging_percentage" equals 100

  Scenario: Samenloop van categorie 1 en categorie 3 geeft de zwaarste categorie
    # Geen samenloopregel in de tekst; de indeling in toenemende ernst
    # (5% / 30% / 100%) maakt categorie 3 de zwaarste.
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | true        |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | false       |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | false       |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | true        |
    When I evaluate "gedragscategorie" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "gedragscategorie" equals 3

  Scenario: Geen van de gedragingen geeft geen gedragscategorie
    # Afwezigheid van elke gedraging uit art. 7 is afwezigheid van een
    # categorie, niet categorie "0" (RFC-036).
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | false       |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | false       |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | false       |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | false       |
    When I evaluate "gedragscategorie" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "gedragscategorie" is absent
    When I evaluate "verlaging_percentage" of "afstemmingsverordening_participatiewet_diemen"
    Then output "verlaging_percentage" is absent
    When I evaluate "duur_maanden" of "afstemmingsverordening_participatiewet_diemen"
    Then output "duur_maanden" is absent

  Scenario: Art. 9 leest de categorie via art. 7 zonder losse parameter
    # Geen gedragscategorie-parameter meegegeven: art. 9 valt terug op de
    # afgeleide waarde van art. 7 (intra-law source).
    Given the following parameters:
      | bsn                                                        | "123456789" |
      | heeft_niet_tijdig_geregistreerd_uwv                        | false       |
      | heeft_niet_meegewerkt_plan_van_aanpak                      | true        |
      | heeft_niet_meegewerkt_onderzoek_jongeren                   | false       |
      | heeft_als_alleenstaande_ouder_geen_deeltijdarbeid_getracht | false       |
      | heeft_tegenprestatie_niet_verricht                         | false       |
      | heeft_niet_naar_vermogen_arbeid_getracht_te_verkrijgen     | false       |
    When I evaluate "verlaging_percentage" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "verlaging_percentage" equals 30
    When I evaluate "duur_maanden" of "afstemmingsverordening_participatiewet_diemen"
    Then output "duur_maanden" equals 1

  Scenario: Rechtstreeks meegegeven gedragscategorie overschrijft de afleiding via art. 7
    # Parameter overschrijft source (test_service_parameter_override):
    # bestaande aanroepers (bv. participatiewet/bijstand.feature) die de
    # categorie als los getal meegeven, blijven werken.
    Given the following parameters:
      | bsn              | "123456789" |
      | gedragscategorie | 2           |
    When I evaluate "verlaging_percentage" of "afstemmingsverordening_participatiewet_diemen"
    Then the execution succeeds
    Then output "verlaging_percentage" equals 30
