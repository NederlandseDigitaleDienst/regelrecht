Feature: Het college oordeelt zoals het geneeskundig advies
  Fictief beleid van de gemeente Wilgendaal. Wie het geneeskundig onderzoek
  verricht, brengt het college een advies uit, met per gegeven dat artikel
  1 van de Regeling gehandicaptenparkeerkaart vraagt of het voor de
  aanvrager geldt (artikel 5). Het college oordeelt zoals dat advies; zonder
  advies oordeelt het zelf (artikel 6). In een cel geeft de GGD het advies
  als antwoord uit haar kroniek (artikel 7); hier geeft het scenario het
  oordeel van de GGD als bevinding.

  Background:
    Given the calculation date is "2026-10-20"
    Given the following parameters:
      | bsn_aanvrager | 999990020  |
      | dagtekening   | 2026-10-05 |
      | besluit_datum | 2026-10-20 |

  Scenario: Het advies vindt een langdurige loopbeperking
    Given the following findings:
      | advies_langdurige_loopbeperking                        | true  |
      | advies_continu_afhankelijk_van_hulp_bestuurder         | false |
      | advies_permanent_rolstoelgebonden                      | false |
      | advies_ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate "langdurige_loopbeperking" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "langdurige_loopbeperking" is true
    When I evaluate "medische_grond_voor_kaart" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "medische_grond_voor_kaart" is true

  Scenario: Het advies vindt geen enkel criterium
    Given the following findings:
      | advies_langdurige_loopbeperking                        | false |
      | advies_continu_afhankelijk_van_hulp_bestuurder         | false |
      | advies_permanent_rolstoelgebonden                      | false |
      | advies_ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate "langdurige_loopbeperking" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "langdurige_loopbeperking" is false
    When I evaluate "medische_grond_voor_kaart" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "medische_grond_voor_kaart" is false

  Scenario: Hulp van de bestuurder alleen is geen medische grond
    # Onder b: een passagier heeft ook een langdurige loopbeperking nodig.
    Given the following findings:
      | advies_langdurige_loopbeperking                        | false |
      | advies_continu_afhankelijk_van_hulp_bestuurder         | true  |
      | advies_permanent_rolstoelgebonden                      | false |
      | advies_ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate "continu_afhankelijk_van_hulp_bestuurder" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "continu_afhankelijk_van_hulp_bestuurder" is true
    When I evaluate "medische_grond_voor_kaart" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "medische_grond_voor_kaart" is false

  Scenario: Een rolstoelgebonden aanvrager heeft een medische grond
    Given the following findings:
      | advies_langdurige_loopbeperking                        | false |
      | advies_continu_afhankelijk_van_hulp_bestuurder         | false |
      | advies_permanent_rolstoelgebonden                      | true  |
      | advies_ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate "medische_grond_voor_kaart" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "medische_grond_voor_kaart" is true

  Scenario: Zonder advies zwijgt het beleid en oordeelt het college zelf
    # Artikel 6, tweede lid: dan geldt het oordeel van het college bij
    # artikel 49 van het BABW.
    When I evaluate "langdurige_loopbeperking" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "langdurige_loopbeperking" is absent
    When I evaluate "medische_grond_voor_kaart" of "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "medische_grond_voor_kaart" is absent
