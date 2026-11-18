Feature: Wie voor een gehandicaptenparkeerkaart in aanmerking komt
  Artikel 1, eerste lid, onder a tot en met d. Een bestuurder met een
  langdurige loopbeperking komt in aanmerking, een passagier alleen als hij
  ook voor het vervoer van deur tot deur continu afhankelijk is van de hulp
  van de bestuurder. Wie permanent rolstoelgebonden is of ernstige
  beperkingen heeft, andere dan loopbeperkingen, komt als bestuurder en als
  passagier in aanmerking. De gegevens blijken uit het geneeskundig
  onderzoek (artikel 2); het college oordeelt erover bij het besluit
  (artikel 49 van het BABW).

  Background:
    Given the calculation date is "2026-10-20"

  Scenario: Een langdurige loopbeperking geeft een kaart als bestuurder, niet als passagier
    Given the following parameters:
      | langdurige_loopbeperking                        | true  |
      | continu_afhankelijk_van_hulp_bestuurder         | false |
      | permanent_rolstoelgebonden                      | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate outputs "komt_in_aanmerking_als_bestuurder, komt_in_aanmerking_als_passagier" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "komt_in_aanmerking_als_bestuurder" is true
    Then output "komt_in_aanmerking_als_passagier" is false

  Scenario: Met continue hulp van de bestuurder komt de passagier ook in aanmerking
    Given the following parameters:
      | langdurige_loopbeperking                        | true  |
      | continu_afhankelijk_van_hulp_bestuurder         | true  |
      | permanent_rolstoelgebonden                      | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate outputs "komt_in_aanmerking_als_bestuurder, komt_in_aanmerking_als_passagier" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "komt_in_aanmerking_als_bestuurder" is true
    Then output "komt_in_aanmerking_als_passagier" is true

  Scenario: Ernstige beperkingen, andere dan loopbeperkingen, geven beide
    Given the following parameters:
      | langdurige_loopbeperking                        | false |
      | continu_afhankelijk_van_hulp_bestuurder         | false |
      | permanent_rolstoelgebonden                      | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen | true  |
    When I evaluate outputs "komt_in_aanmerking_als_bestuurder, komt_in_aanmerking_als_passagier" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "komt_in_aanmerking_als_bestuurder" is true
    Then output "komt_in_aanmerking_als_passagier" is true

  Scenario: Zonder een van de beperkingen komt niemand in aanmerking
    Given the following parameters:
      | langdurige_loopbeperking                        | false |
      | continu_afhankelijk_van_hulp_bestuurder         | true  |
      | permanent_rolstoelgebonden                      | false |
      | ernstige_beperkingen_andere_dan_loopbeperkingen | false |
    When I evaluate outputs "komt_in_aanmerking_als_bestuurder, komt_in_aanmerking_als_passagier" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "komt_in_aanmerking_als_bestuurder" is false
    Then output "komt_in_aanmerking_als_passagier" is false
