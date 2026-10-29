Feature: Het besluit op de aanvraag om een gehandicaptenparkeerkaart
  Het college van de gemeente waar de gehandicapte als ingezetene is
  ingeschreven, verstrekt de kaart overeenkomstig de criteria van de Regeling
  gehandicaptenparkeerkaart (artikel 49, eerste lid). Of de aanvrager aan die
  criteria voldoet, oordeelt het college na geneeskundig onderzoek. De kaart
  is vijf jaar geldig vanaf de afgifte (artikel 51, eerste lid). Hoe het
  college zijn bevoegdheid gebruikt, zegt het (fictieve) beleid van de
  gemeente: geen kaart naast een geldige. Of de aanvrager er al een heeft,
  stelt het college vast. Hier is dat een vaststelling; in de cel van de
  gemeente koppelt het beleid dat open begrip aan het antwoord van de cel,
  dat haar reductie uit haar besluiten haalt (RFC-048). Bij een weigering op
  de criteria vraagt de engine het niet. Het besluit is een beschikking: de Awb vraagt een
  motivering (3:46) en opent een bezwaartermijn van zes weken (6:7).
  Een kaart wordt niet afgegeven voordat een geneeskundig onderzoek heeft
  plaatsgehad (artikel 2, eerste lid, van de regeling). Hier stelt het
  college dat vast; in de cel van de gemeente vraagt het beleid het aan de
  cel van de GGD, die het uit de onderzoeken haalt die zij vastlegde. Weet
  niemand het, dan faalt het besluit.

  Background:
    Given the calculation date is "2026-10-20"
    Given law "algemene_wet_bestuursrecht" is loaded

  Scenario: Een bestuurder met een langdurige loopbeperking krijgt een kaart voor vijf jaar
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-10-20 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_vermeldt_bestuurder" is true
    Then output "kaart_vermeldt_passagier" is false
    Then output "kaart_geldig_tot" equals "2031-10-19"
    Then output "motivering_vereist" is true
    Then output "motivering_vereist" has reactive provenance
    Then output "termijn_weken" equals 6
    Then output "termijn_weken" has reactive provenance

  Scenario: Zonder loopbeperking of andere ernstige beperking wordt de kaart geweigerd
    # Het geneeskundig onderzoek toont geen beperking die artikel 1 van de
    # regeling noemt.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-10-20 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | langdurige_loopbeperking_volgens_college                          | false |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is false
    Then output "kaart_vermeldt_bestuurder" is false
    Then output "kaart_geldig_tot" is absent
    Then output "motivering_vereist" is true

  Scenario: Een passagier die niet afhankelijk is van de hulp van de bestuurder krijgt geen kaart
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | false |
      | kaart_als_passagier                               | true |
      | besluit_datum                                     | 2026-10-20 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is false
    Then output "kaart_vermeldt_passagier" is false

  Scenario: Wie permanent rolstoelgebonden is, krijgt een kaart als bestuurder en als passagier
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | true |
      | besluit_datum                                     | 2026-10-20 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                          | false |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | true |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_vermeldt_bestuurder" is true
    Then output "kaart_vermeldt_passagier" is true

  Scenario: Wie al een geldige kaart heeft, krijgt geen tweede
    # Het fictieve beleid van de gemeente vult de bevoegdheid van lid 1 zo in.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-10-20 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | true |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is false
    Then output "kaart_geldig_tot" is absent

  Scenario: De geldigheidsduur loopt vijf jaar vanaf de dag van afgifte, ook over een schrikkeldag
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2027-03-01 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "kaart_geldig_tot" equals "2032-02-29"

  Scenario: Een kaart afgegeven op 29 februari is geldig tot en met 28 februari vijf jaar later
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2028-02-29 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "kaart_geldig_tot" equals "2033-02-28"

  Scenario: Wie als bestuurder en als passagier vraagt en maar voor een in aanmerking komt, krijgt alleen die letter
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | true |
      | besluit_datum                                     | 2026-10-20 |
      | dagtekening                                       | 2026-10-12 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college | false |
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college          | true  |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_vermeldt_bestuurder" is true
    Then output "kaart_vermeldt_passagier" is false

  Scenario: Zonder geneeskundig onderzoek wordt de kaart nog niet afgegeven
    # Artikel 2, eerste lid, van de regeling: "wordt niet afgegeven alvorens
    # een geneeskundig onderzoek heeft plaatsgehad". Geen weigering: zonder
    # vaststelling dat het onderzoek plaatsvond, kan het college niet
    # beslissen.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2031-10-20 |
      | dagtekening                                       | 2031-10-14 |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college   | false |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution fails with "geneeskundig_onderzoek_heeft_plaatsgehad"

  Scenario: Wie eerder een kaart kreeg en nog steeds voldoet, krijgt een kaart zonder nieuw onderzoek
    # Artikel 2, tweede lid, onder a: het college weet dat de aanvrager nog
    # steeds voldoet en laat het onderzoek achterwege. Of het onderzoek
    # plaatsvond, vraagt de engine dan niet.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2031-10-20 |
      | dagtekening                                       | 2031-10-14 |
      | geneeskundig_onderzoek_achterwege_gelaten         | true |
      | bekend_dat_aanvrager_nog_steeds_voldoet           | true |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college   | false |
      | aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt_volgens_college | true |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_geldig_tot" equals "2036-10-19"

  Scenario: Op het oordeel van de keurende instantie blijft het onderzoek ook achterwege
    # Artikel 2, tweede lid, onder b.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2031-10-20 |
      | dagtekening                                       | 2031-10-14 |
      | geneeskundig_onderzoek_achterwege_gelaten         | true |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | true |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college   | false |
      | aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt_volgens_college | true |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true

  Scenario: Zonder eerdere kaart mag het onderzoek niet achterwege blijven
    # Het college laat het onderzoek achterwege, maar de aanvrager kreeg
    # nooit eerder een kaart: lid 2 staat het niet toe, en lid 1 geldt.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2031-10-20 |
      | dagtekening                                       | 2031-10-14 |
      | geneeskundig_onderzoek_achterwege_gelaten         | true |
      | bekend_dat_aanvrager_nog_steeds_voldoet           | true |
    Given the following findings:
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college   | false |
      | aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt_volgens_college | false |
      | langdurige_loopbeperking_volgens_college                          | true |
      | continu_afhankelijk_van_hulp_bestuurder_volgens_college           | false |
      | permanent_rolstoelgebonden_volgens_college                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college   | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution fails with "geneeskundig_onderzoek_heeft_plaatsgehad"
