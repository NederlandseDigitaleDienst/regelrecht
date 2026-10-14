Feature: Het besluit op de aanvraag om een gehandicaptenparkeerkaart
  Het college van de gemeente waar de gehandicapte als ingezetene is
  ingeschreven, verstrekt de kaart overeenkomstig de criteria van de Regeling
  gehandicaptenparkeerkaart (artikel 49, eerste lid). Of de aanvrager aan die
  criteria voldoet, oordeelt het college na geneeskundig onderzoek. De kaart
  is vijf jaar geldig vanaf de afgifte (artikel 51, eerste lid). Of de
  aanvrager al een geldige kaart heeft, is een parameter: de cel van de
  gemeente leidt het af uit haar besluiten (RFC-048). Het (fictieve) beleid
  van de gemeente verstrekt dan geen nieuwe. Het besluit is een beschikking:
  de Awb vraagt een motivering (3:46) en opent een bezwaartermijn van zes
  weken (6:7).

  Background:
    Given the calculation date is "2026-10-20"
    Given law "algemene_wet_bestuursrecht" is loaded

  Scenario: Een bestuurder met een langdurige loopbeperking krijgt een kaart voor vijf jaar
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-10-20 |
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_vermeldt_bestuurder" is true
    Then output "kaart_vermeldt_passagier" is false
    Then output "kaart_geldig_tot" equals "2031-10-19"
    Then output "motivering_vereist" is true
    Then output "motivering_vereist" has reactive provenance
    Then output "bezwaartermijn_weken" equals 6
    Then output "bezwaartermijn_weken" has reactive provenance

  Scenario: Zonder loopbeperking of andere ernstige beperking wordt de kaart geweigerd
    # Het geneeskundig onderzoek toont geen beperking die artikel 1 van de
    # regeling noemt.
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2026-10-20 |
      | langdurige_loopbeperking                          | false |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
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
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
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
      | langdurige_loopbeperking                          | false |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | true |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
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
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | true |
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
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "kaart_geldig_tot" equals "2032-02-29"

  Scenario: Een kaart afgegeven op 29 februari is geldig tot en met 28 februari vijf jaar later
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | false |
      | besluit_datum                                     | 2028-02-29 |
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "kaart_geldig_tot" equals "2033-02-28"

  Scenario: Wie als bestuurder en als passagier vraagt en maar voor een in aanmerking komt, krijgt alleen die letter
    Given the following parameters:
      | bsn_aanvrager                                     | 999990020 |
      | kaart_als_bestuurder                              | true |
      | kaart_als_passagier                               | true |
      | besluit_datum                                     | 2026-10-20 |
      | langdurige_loopbeperking                          | true |
      | continu_afhankelijk_van_hulp_bestuurder           | false |
      | permanent_rolstoelgebonden                        | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen   | false |
      | aanvrager_heeft_geldige_gehandicaptenparkeerkaart | false |
    When I evaluate outputs "gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier, kaart_geldig_tot" of "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_verstrekt" is true
    Then output "kaart_vermeldt_bestuurder" is true
    Then output "kaart_vermeldt_passagier" is false
