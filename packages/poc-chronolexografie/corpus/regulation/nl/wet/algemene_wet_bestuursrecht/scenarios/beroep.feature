Feature: Het beroep bij de bestuursrechter en de uitspraak erop
  Een belanghebbende kan tegen een besluit beroep instellen bij de
  bestuursrechter (artikel 8:1), door een beroepschrift bij die rechter in te
  dienen (artikel 6:4, derde lid). Wie beroep instelt, maakt eerst bezwaar
  (artikel 7:1, eerste lid): het bestreden besluit is dan de beslissing op
  het bezwaar, of het nieuwe besluit dat zij nam. Het beroepschrift bevat ten
  minste wat artikel 6:5 noemt en is tijdig als het voor het einde van de
  termijn is ontvangen (artikelen 6:7 tot en met 6:9). De uitspraak strekt tot
  niet-ontvankelijkverklaring, ongegrondverklaring of gegrondverklaring van
  het beroep (artikel 8:70). Bij een gegrond beroep vernietigt de
  bestuursrechter het besluit en vermeldt hij welke rechtsregel geschonden
  is (artikelen 8:72, eerste lid, en 8:77, tweede lid). Hij kan de
  rechtsgevolgen in stand laten (artikel 8:72, derde lid, onder a); anders
  draagt hij het bestuursorgaan een nieuw besluit op, zo nodig binnen een
  termijn (vierde lid).

  Het bestreden besluit is een besluitwaarde, zoals in bezwaar.feature. Wat
  de bestuursrechter oordeelt, geeft de uitvoering apart van de parameters
  mee (findings): of de indiener belanghebbende is, of het besluit een
  rechtsregel schendt en welke, of de rechtsgevolgen in stand blijven, en de
  termijn voor het nieuwe besluit. De eerste twee scenario's rekenen artikel
  6:9 bij het beroepschrift ook uit en geven dezelfde uitkomst mee aan
  artikel 8:70; de scenario's daarna geven alleen die uitkomst mee.

  Background:
    Given the calculation date is "2027-01-11"
    Given law "leerplichtwet_1969" is loaded

  Scenario: Een beroepschrift ontvangen op de laatste dag van de termijn is tijdig
    # De beslissing op bezwaar is bekendgemaakt op 15 november 2026; de
    # termijn van zes weken eindigt op 27 december (artikelen 6:7 en 6:8).
    Given the following parameters:
      | bezwaar_of_beroepschrift_ondertekend | true |
      | naam_indiener | J. Jansen |
      | adres_indiener | Voorbeeldstraat 1 |
      | dagtekening_bezwaar_of_beroepschrift | 2026-12-20 |
      | omschrijving_bestreden_besluit | beslissing op bezwaar tegen de weigering van vakantieverlof |
      | gronden_bezwaar_of_beroep | de weigering is niet deugdelijk gemotiveerd |
      | datum_ontvangst_bezwaar_of_beroepschrift | 2026-12-27 |
      | termijn_einddatum | 2026-12-27 |
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | false |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_of_beroepschrift_tijdig_ingediend" is true
    Then output "bezwaar_of_beroepschrift_bevat_wat_artikel_6_5_eist" is true
    Then output "beroep_ontvankelijk" is true
    Then output "uitspraak_strekt_tot" equals "ongegrondverklaring van het beroep"

  Scenario: Een beroepschrift ontvangen na het einde van de termijn is niet-ontvankelijk
    # Artikel 6:11 (verschoonbare termijnoverschrijding) is niet gemodelleerd.
    # Op een niet-ontvankelijk beroep oordeelt de bestuursrechter niet over
    # de gronden, dus hij stelt niets vast.
    Given the following parameters:
      | datum_ontvangst_bezwaar_of_beroepschrift | 2026-12-28 |
      | termijn_einddatum | 2026-12-27 |
      | bezwaar_of_beroepschrift_tijdig_ingediend | false |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_of_beroepschrift_tijdig_ingediend" is false
    Then output "beroep_ontvankelijk" is false
    Then output "beroep_gegrond" is absent
    Then output "uitspraak_strekt_tot" equals "niet-ontvankelijkverklaring van het beroep"
    Then output "bestreden_besluit_vernietigd" is false
    Then output "opdracht_nieuw_besluit" is false

  Scenario: Beroep tegen het nieuwe besluit dat op bezwaar is genomen, is ontvankelijk
    # Het besluit berust op artikel 13a van de Leerplichtwet 1969, maar is
    # genomen op het bezwaar (taken_on): artikel 7:1, eerste lid, onder a.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": true, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": true}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | false |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "beroep_ontvankelijk" is true
    Then output "beroep_gegrond" is false

  Scenario: Beroep zonder voorafgaand bezwaar is niet-ontvankelijk
    # Artikel 7:1, eerste lid: wie beroep instelt, maakt eerst bezwaar. Dit
    # besluit is genomen op de aanvraag. De uitzonderingen onder b tot en met
    # g zijn niet gemodelleerd.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "beroep_ontvankelijk" is false
    Then output "uitspraak_strekt_tot" equals "niet-ontvankelijkverklaring van het beroep"
    Then output "bestreden_besluit_vernietigd" is false

  Scenario: Wie geen belanghebbende is, kan geen beroep instellen
    # Artikel 8:1 met 1:2, eerste lid.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | false |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "beroep_ontvankelijk" is false
    Then output "uitspraak_strekt_tot" equals "niet-ontvankelijkverklaring van het beroep"

  Scenario: Een ongegrond beroep laat het besluit staan
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | false |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "beroep_gegrond" is false
    Then output "uitspraak_strekt_tot" equals "ongegrondverklaring van het beroep"
    Then output "geschonden_geoordeelde_rechtsregel" is absent
    Then output "bestreden_besluit_vernietigd" is false
    Then output "rechtsgevolgen_vernietigd" is false
    Then output "opdracht_nieuw_besluit" is false
    Then output "termijn_nieuw_besluit_einddatum" is absent

  Scenario: Een gegrond beroep vernietigt het besluit en draagt een nieuw besluit op binnen een termijn
    # De beslissing op bezwaar is niet deugdelijk gemotiveerd (artikel 7:12,
    # eerste lid). De bestuursrechter laat de rechtsgevolgen niet in stand en
    # geeft het bestuursorgaan twee weken.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | false |
      | termijn_nieuw_besluit | 2027-01-25 |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "beroep_gegrond" is true
    Then output "uitspraak_strekt_tot" equals "gegrondverklaring van het beroep"
    Then output "geschonden_geoordeelde_rechtsregel" equals "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht"
    Then output "bestreden_besluit_vernietigd" is true
    Then output "rechtsgevolgen_vernietigd" is true
    Then output "opdracht_nieuw_besluit" is true
    Then output "termijn_nieuw_besluit_einddatum" equals "2027-01-25"
    Then output "uitspraak_in_de_plaats" is absent

  Scenario: Een gegrond beroep zonder termijn draagt een nieuw besluit op
    # Artikel 8:72, vierde lid, onder b: de bestuursrechter "kan" een termijn
    # stellen.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | false |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "opdracht_nieuw_besluit" is true
    Then output "termijn_nieuw_besluit_einddatum" is absent

  Scenario: Een gegrond beroep waarbij de rechtsgevolgen in stand blijven, draagt geen nieuw besluit op
    # Artikel 8:72, derde lid, onder a. Lid 4 geldt alleen als het derde lid
    # niet is toegepast.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 3:46 van de Algemene wet bestuursrecht |
      | rechtsgevolgen_in_stand | true |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "beroep_gegrond" is true
    Then output "bestreden_besluit_vernietigd" is true
    Then output "rechtsgevolgen_vernietigd" is false
    Then output "opdracht_nieuw_besluit" is false
    Then output "termijn_nieuw_besluit_einddatum" is absent

  Scenario: Zonder het oordeel van de bestuursrechter over de gronden is er geen uitspraak
    # Een verplicht oordeel dat ontbreekt, valt niet stil weg: zonder zou het
    # beroep ongegrond lijken.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution fails with "rechtsregel_geschonden"

  Scenario: Zonder het oordeel over de rechtsgevolgen is er geen uitspraak op een gegrond beroep
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution fails with "rechtsgevolgen_in_stand"

  Scenario: Een gegrond beroep waarin de bestuursrechter zelf in de zaak voorziet
    # Artikel 8:72, derde lid, onder b. De uitspraak treedt in de plaats van
    # de beslissing op bezwaar: de bestuursrechter neemt die beslissing zelf
    # opnieuw, met zijn oordeel over het beroep van de ouder. Het bestreden
    # besluit bevat de weigering waartegen het bezwaar ging als besluitwaarde,
    # zoals de school haar in het afschrift meegeeft. Of het hoofd dit
    # schooljaar al verlof verleende, stelt de bestuursrechter zelf vast. Er
    # volgt geen opdracht (vierde lid).
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bestreden_besluit": {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}}, "bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | geschonden_rechtsregel | artikel 7:12, eerste lid, van de Algemene wet bestuursrecht |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | true |
      | specifieke_aard_beroep_ouder | true |
      | verlof_al_verleend_in_schooljaar_volgens_hoofd | false |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "uitspraak_strekt_tot" equals "gegrondverklaring van het beroep"
    Then output "bestreden_besluit_vernietigd" is true
    Then output "rechtsgevolgen_vernietigd" is true
    Then output "opdracht_nieuw_besluit" is false
    Then output "termijn_nieuw_besluit_einddatum" is absent

  Scenario: Wie zelf in de zaak voorziet, stelt ook vast wat het bestuursorgaan anders zelf invulde
    # In de cel van de school vult haar reductie in of het hoofd dit
    # schooljaar al verlof verleende; dat staat dus niet in de besluitwaarde.
    # De bestuursrechter kan de feiten aanvullen (artikel 8:69, derde lid);
    # zonder dat oordeel is er geen uitspraak.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bestreden_besluit": {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}}, "bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    Given the following findings:
      | indiener_is_belanghebbende | true |
      | rechtsregel_geschonden | true |
      | rechtsgevolgen_in_stand | false |
      | uitspraak_treedt_in_de_plaats | true |
      | specifieke_aard_beroep_ouder | true |
    When I evaluate outputs "uitspraak_strekt_tot, geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd, rechtsgevolgen_vernietigd, opdracht_nieuw_besluit, termijn_nieuw_besluit_einddatum, uitspraak_in_de_plaats" of "algemene_wet_bestuursrecht"
    Then the execution fails with "verlof_al_verleend_in_schooljaar"
