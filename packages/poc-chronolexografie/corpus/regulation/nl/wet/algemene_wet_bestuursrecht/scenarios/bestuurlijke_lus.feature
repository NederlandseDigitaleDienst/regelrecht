Feature: De bestuurlijke lus: tussenuitspraak, herstel en einduitspraak
  Ziet de bestuursrechter een gebrek in het bestreden besluit, dan kan hij
  het bestuursorgaan in de gelegenheid stellen het te herstellen (artikel
  8:51a). Hij doet dan een tussenuitspraak (artikel 8:80a). Die noemt de
  geschonden rechtsregel, de termijn voor het herstel en zo mogelijk hoe het
  herstel kan. Het bestuursorgaan deelt mee of en hoe het herstelde (artikel
  8:51b). Herstelt het met een nieuw besluit, dan heeft het beroep daar van
  rechtswege mede betrekking op (artikel 6:19, eerste lid). Daarna doet de
  bestuursrechter einduitspraak op hetzelfde beroep.

  De tussenuitspraak en de einduitspraak zijn allebei een uitspraak onder
  artikel 8:70. Een einduitspraak na herstel met een nieuw besluit krijgt dat
  besluit mee als vervangend_besluit, zoals de mededeling het bracht. Zonder
  zo'n besluit is vervangend_besluit leeg.

  Background:
    Given the calculation date is "2027-01-11"
    Given law "leerplichtwet_1969" is loaded

  Scenario: Een tussenuitspraak stelt het bestuursorgaan in de gelegenheid het gebrek te herstellen
    # De beslissing op bezwaar is niet deugdelijk gemotiveerd (artikel 7:12,
    # eerste lid). De rechtbank geeft de school zes weken. Een
    # tussenuitspraak verklaart het beroep nog niet gegrond of ongegrond en
    # vernietigt niets.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | null |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | gelegenheid_tot_herstel | true |
      | termijn_herstel | 2027-02-22 |
      | wijze_waarop_gebrek_kan_worden_hersteld | een nieuwe beslissing op bezwaar met een deugdelijke motivering |
    When I evaluate outputs "tussenuitspraak, uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, opdracht_nieuw_besluit, termijn_herstel_einddatum, wijze_van_herstel, uitspraak_over_vervangend_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "tussenuitspraak" is true
    Then output "beroep_gegrond" is absent
    Then output "uitspraak_strekt_tot" is absent
    Then output "geschonden_geoordeelde_rechtsregel" equals "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht"
    Then output "bestreden_besluit_vernietigd" is false
    Then output "opdracht_nieuw_besluit" is false
    Then output "termijn_herstel_einddatum" equals "2027-02-22"
    Then output "wijze_van_herstel" equals "een nieuwe beslissing op bezwaar met een deugdelijke motivering"
    Then output "uitspraak_over_vervangend_besluit" is absent

  Scenario: Zonder termijn is er geen tussenuitspraak
    # Artikel 8:51a, tweede lid: de bestuursrechter "bepaalt" de termijn.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | null |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | gelegenheid_tot_herstel | true |
    When I evaluate outputs "tussenuitspraak, termijn_herstel_einddatum" of "algemene_wet_bestuursrecht"
    Then the execution fails with "termijn_herstel"

  Scenario: Zonder gebrek is er geen bestuurlijke lus
    # Artikel 8:51a gaat over een gebrek in het bestreden besluit. Schendt
    # het besluit geen rechtsregel, dan leest artikel 8:70 dat artikel niet:
    # de gelegenheid tot herstel doet dan niets.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | null |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | false |
    When I evaluate outputs "tussenuitspraak, uitspraak_strekt_tot, termijn_herstel_einddatum" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "tussenuitspraak" is false
    Then output "uitspraak_strekt_tot" equals "ongegrondverklaring van het beroep"
    Then output "termijn_herstel_einddatum" is absent

  Scenario: Na herstel met een aanvulling op de motivering blijven de rechtsgevolgen in stand
    # De school vulde de motivering aan zonder nieuw besluit. In de
    # einduitspraak verklaart de rechtbank het beroep gegrond, want de
    # beslissing op bezwaar had het gebrek, maar laat zij de rechtsgevolgen
    # in stand (artikel 8:72, derde lid, onder a).
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | null |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | rechtsgevolgen_in_stand | true |
    When I evaluate outputs "tussenuitspraak, uitspraak_strekt_tot, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, uitspraak_over_vervangend_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "tussenuitspraak" is false
    Then output "uitspraak_strekt_tot" equals "gegrondverklaring van het beroep"
    Then output "bestreden_besluit_vernietigd" is true
    Then output "rechtsgevolgen_vernietigd" is false
    Then output "opdracht_nieuw_besluit" is false
    Then output "uitspraak_over_vervangend_besluit" is absent

  Scenario: Na herstel met een nieuw besluit oordeelt de einduitspraak ook over dat besluit
    # De school nam een nieuwe beslissing op hetzelfde bezwaar, met dezelfde
    # uitkomst en nu een deugdelijke motivering. Het beroep heeft daar van
    # rechtswege mede betrekking op (artikel 6:19, eerste lid). Het beroep
    # tegen de eerste beslissing is gegrond en die wordt vernietigd; omdat het
    # vervangende besluit er al is, volgt geen opdracht. Het beroep tegen het
    # vervangende besluit is ongegrond.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | false |
      | rechtsregel_geschonden_door_vervangend_besluit | false |
    When I evaluate outputs "tussenuitspraak, uitspraak_strekt_tot, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, uitspraak_over_vervangend_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "uitspraak_strekt_tot" equals "gegrondverklaring van het beroep"
    Then output "bestreden_besluit_vernietigd" is true
    Then output "rechtsgevolgen_vernietigd" is true
    Then output "opdracht_nieuw_besluit" is false
    Then output "uitspraak_over_vervangend_besluit" equals "ongegrondverklaring van het beroep tegen het vervangende besluit"

  Scenario: Zonder oordeel over het vervangende besluit is er geen einduitspraak
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | false |
    When I evaluate outputs "uitspraak_over_vervangend_besluit" of "algemene_wet_bestuursrecht"
    Then the execution fails with "rechtsregel_geschonden_door_vervangend_besluit"

  Scenario: Laat het bestuursorgaan het herstel na, dan draagt de einduitspraak een nieuw besluit op
    # De school maakte geen gebruik van de gelegenheid. Het gebrek blijft, de
    # rechtsgevolgen blijven niet in stand en de rechtbank draagt een nieuw
    # besluit op (artikel 8:72, vierde lid).
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
      | vervangend_besluit | null |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | gelegenheid_tot_herstel | false |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | false |
      | termijn_nieuw_besluit | 2027-04-05 |
    When I evaluate outputs "tussenuitspraak, uitspraak_strekt_tot, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "tussenuitspraak" is false
    Then output "uitspraak_strekt_tot" equals "gegrondverklaring van het beroep"
    Then output "opdracht_nieuw_besluit" is true
    Then output "termijn_nieuw_besluit_einddatum" equals "2027-04-05"
