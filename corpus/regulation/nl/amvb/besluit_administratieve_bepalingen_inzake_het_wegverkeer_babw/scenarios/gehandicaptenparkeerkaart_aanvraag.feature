Feature: De aanvraag om een gehandicaptenparkeerkaart, met de Awb erop
  Artikel 49 vestigt een aanvraag, zoals artikel 13a van de Leerplichtwet 1969
  (RFC-045). De Awb haakt op elke aanvraag: artikel 4:2 zegt wat de aanvraag
  ten minste bevat, artikel 4:13 wanneer de redelijke beslistermijn in ieder
  geval is verstreken. Buiten de procedure van de Awb voert de engine het
  artikel in één keer uit, met het besluit erop (RFC-047). Daarom staan de
  oordelen van het college erbij.

  Background:
    Given the calculation date is "2026-10-13"
    Given law "algemene_wet_bestuursrecht" is loaded

  Scenario: Een volledige aanvraag geeft de velden van 4:2 en de beslistermijn van 4:13
    Given the following parameters:
      | bsn_aanvrager         | 999990020                 |
      | kaart_als_bestuurder  | true                      |
      | kaart_als_passagier   | false                     |
      | aanvraag_ondertekend  | true                      |
      | naam_aanvrager        | A. de Vries               |
      | adres_aanvrager       | Voorbeeldlaan 2           |
      | dagtekening           | 2026-10-12                |
      | gevraagde_beschikking | gehandicaptenparkeerkaart |
      | datum_ontvangst       | 2026-10-13                |
      | besluit_datum         | 2026-10-13                |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                        | true  |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college         | false |
      | permanent_rolstoelgebonden_volgens_college                      | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college | false |
    When I evaluate "gehandicaptenparkeerkaart_verstrekt" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "aanvraag_bevat_wat_artikel_4_2_eist" is true
    Then output "aanvraag_bevat_wat_artikel_4_2_eist" has reactive provenance
    Then output "redelijke_beslistermijn_einddatum" equals "2026-12-08"
    Then output "redelijke_beslistermijn_einddatum" has reactive provenance

  Scenario: Zonder handtekening bevat de aanvraag niet wat 4:2 eist
    Given the following parameters:
      | bsn_aanvrager         | 999990020                 |
      | kaart_als_bestuurder  | true                      |
      | kaart_als_passagier   | false                     |
      | aanvraag_ondertekend  | false                     |
      | naam_aanvrager        | A. de Vries               |
      | adres_aanvrager       | Voorbeeldlaan 2           |
      | dagtekening           | 2026-10-12                |
      | gevraagde_beschikking | gehandicaptenparkeerkaart |
      | datum_ontvangst       | 2026-10-13                |
      | besluit_datum         | 2026-10-13                |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                        | true  |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college         | false |
      | permanent_rolstoelgebonden_volgens_college                      | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college | false |
    When I evaluate "gehandicaptenparkeerkaart_verstrekt" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "aanvraag_bevat_wat_artikel_4_2_eist" is false
