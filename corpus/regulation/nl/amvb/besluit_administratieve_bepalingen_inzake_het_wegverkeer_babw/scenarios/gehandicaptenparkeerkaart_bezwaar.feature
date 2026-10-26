Feature: Het bezwaar tegen een besluit op een gehandicaptenparkeerkaart
  Op een ontvankelijk bezwaar heroverweegt het college het besluit (Awb 7:11,
  eerste lid): het past artikel 49 opnieuw toe, op de feiten van die dag. Komt
  er een andere uitkomst uit dan in het bestreden besluit, dan geeft de
  heroverweging aanleiding het te herroepen (tweede lid). De uitkomst is of
  het college een kaart verstrekt, en met welke letters. De laatste geldige
  dag is afgeleid (derived, RFC-050): een heroverweging op een latere dag
  geeft een kaart die later afloopt, maar dat alleen is geen aanleiding. De
  eerste twee scenario's tonen alleen de heroverweging, het derde ook wat
  artikel 7:11 ermee doet. De keten van bezwaarschrift naar beslissing en
  nieuw besluit staat in de tests van de cel (packages/cel/tests/parkeerkaart.rs).

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
      | dagtekening                                       | 2026-10-12 |
      | langdurige_loopbeperking                          | false |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
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
      | dagtekening                                       | 2026-10-12 |
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad          | true  |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_geldig_tot" equals "2031-11-19"

  Scenario: Een bezwaar tegen een verstrekte kaart zonder nieuw oordeel laat het besluit in stand
    # Verstrekt op 20 oktober, geldig tot en met 19 oktober 2031. Van de
    # uitvoer bevat een besluitwaarde alleen de uitkomst, dus geen
    # kaart_geldig_tot. Als besluit_datum staat de dag van de heroverweging,
    # zoals de cel die invult: die geeft een kaart tot en met 19 november
    # 2031, maar dezelfde uitkomst, dus geen aanleiding. Het bestreden besluit
    # telt bij de heroverweging niet mee als geldige kaart. In de cel leest de
    # reductie die het antwoord geeft de kroniek zonder het bestreden besluit;
    # hier is het een vaststelling van het college.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49", "taken_on": "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49", "fields": {"bsn_aanvrager": "999990020", "kaart_als_bestuurder": true, "kaart_als_passagier": false, "besluit_datum": "2026-11-20", "dagtekening": "2026-10-12"}, "findings": {"langdurige_loopbeperking": true, "continu_afhankelijk_van_hulp_bestuurder": false, "permanent_rolstoelgebonden": false, "ernstige_beperkingen_andere_dan_loopbeperkingen": false}, "outputs": {"gehandicaptenparkeerkaart_verstrekt": true, "kaart_vermeldt_bestuurder": true, "kaart_vermeldt_passagier": false}} |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad          | true  |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_ontvankelijk" is true
    Then output "heroverweging_geeft_aanleiding" is false
    Then output "bestreden_besluit_herroepen" is false
    Then output "nieuw_besluit" is absent
