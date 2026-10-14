Feature: Het bezwaar tegen de weigering van een gehandicaptenparkeerkaart
  Op een ontvankelijk bezwaar heroverweegt het college het besluit (Awb 7:11,
  eerste lid): het past artikel 49 opnieuw toe, op de feiten van die dag. Komt
  er iets anders uit dan in het bestreden besluit, dan geeft de heroverweging
  aanleiding het te herroepen (tweede lid). Deze scenario's tonen alleen de
  heroverweging; wat artikel 7:11 ermee doet, staat in bezwaar.feature bij de
  Awb, en de keten van bezwaarschrift naar beslissing en nieuw besluit
  staat in de tests van de cel (packages/cel/tests/parkeerkaart.rs).

  Background:
    Given the calculation date is "2026-11-20"
    Given law "algemene_wet_bestuursrecht" is loaded

  Scenario: De heroverweging met hetzelfde onderzoek weigert de kaart opnieuw
    # Het bestreden besluit weigerde de kaart op dit onderzoek; de
    # heroverweging geeft hetzelfde, dus geen aanleiding tot herroeping.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-11-20 |
      | langdurige_loopbeperking                          | false |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is false

  Scenario: De heroverweging na een nieuw onderzoek verstrekt de kaart alsnog
    # Een nieuw geneeskundig onderzoek toont een langdurige loopbeperking. Het
    # nieuwe besluit heeft zijn eigen geldigheidsduur, vanaf zijn eigen afgifte.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-11-20 |
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_geldig_tot" equals "2031-11-19"
