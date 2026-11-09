Feature: Het bezwaar en de beslissing erop
  Wie bezwaar maakt, dient een bezwaarschrift in bij het bestuursorgaan dat
  het besluit nam (artikel 6:4). Het bezwaarschrift bevat ten minste wat
  artikel 6:5 noemt en is tijdig als het voor het einde van de termijn is
  ontvangen (artikel 6:9). Op een ontvankelijk bezwaar volgt een
  heroverweging: het bestuursorgaan past het artikel van het bestreden
  besluit opnieuw toe, met de gegevens en bevindingen van dat besluit en wat
  het opnieuw vaststelt. Geeft dat een andere uitkomst, dan herroept het
  bestuursorgaan het bestreden besluit en neemt het in zijn plaats het
  besluit dat de heroverweging gaf (artikel 7:11). Het beslist binnen zes
  weken na het einde van de bezwaartermijn (artikel 7:10).

  Het bestreden besluit is hier een weigering van vakantieverlof (artikel
  13a van de Leerplichtwet 1969), als besluitwaarde. Die bevat het artikel
  waarop het besluit berust, het artikel van de indiening waarop het is
  genomen (taken_on), de gegevens waarop het is genomen, wat het hoofd per
  geval vaststelde (findings) en de uitkomst. Wat het hoofd bij de
  heroverweging opnieuw vaststelt, geeft de uitvoering apart van de
  parameters mee (findings); een bevinding die geen artikel in de uitvoering
  vraagt, is een fout. Het hoofd oordeelde dat het beroep van de ouder niet
  van de aard is die artikel 11, onder f, vraagt. De eerste twee scenario's
  rekenen artikel 6:9 zelf uit. De scenario's daarna geven artikel 7:11
  buiten een procedure de uitkomst van artikel 6:9 mee, niet de gegevens
  daarvan. De keten van bezwaarschrift naar beslissing, met het nieuwe
  besluit in de kroniek, staat in de tests van de cel (packages/cel).

  Background:
    Given the calculation date is "2026-11-15"
    Given law "leerplichtwet_1969" is loaded

  Scenario: Een bezwaarschrift ontvangen op de laatste dag van de termijn is tijdig
    # Artikel 6:9, eerste lid: "voor het einde van de termijn" is uiterlijk op
    # de laatste dag.
    Given the following parameters:
      | datum_ontvangst_bezwaar_of_beroepschrift | 2026-12-01 |
      | termijn_einddatum                        | 2026-12-01 |
    When I evaluate outputs "bezwaar_of_beroepschrift_tijdig_ingediend" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_of_beroepschrift_tijdig_ingediend" is true

  Scenario: Een bezwaarschrift ontvangen op de dag na de termijn is niet tijdig
    Given the following parameters:
      | datum_ontvangst_bezwaar_of_beroepschrift | 2026-12-02 |
      | termijn_einddatum                        | 2026-12-01 |
    When I evaluate outputs "bezwaar_of_beroepschrift_tijdig_ingediend" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_of_beroepschrift_tijdig_ingediend" is false

  Scenario: Een tijdig bezwaar waarop de heroverweging aanleiding geeft, herroept het besluit
    # Bij de heroverweging oordeelt het hoofd opnieuw over het beroep van de
    # ouder; artikel 13a verleent dan het verlof.
    Given the following parameters:
      | bezwaar_of_beroepschrift_ondertekend      | true                     |
      | naam_indiener                             | J. Jansen                |
      | adres_indiener                            | Voorbeeldstraat 1        |
      | dagtekening_bezwaar_of_beroepschrift      | 2026-11-01               |
      | omschrijving_bestreden_besluit            | weigering vakantieverlof |
      | gronden_bezwaar_of_beroep                 | verklaring werkgever     |
      | termijn_einddatum                         | 2026-12-01               |
      | bezwaar_of_beroepschrift_tijdig_ingediend | true                     |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    Given the following findings:
      | specifieke_aard_beroep_ouder | true |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_ontvankelijk" is true
    Then output "heroverweging_geeft_aanleiding" is true
    Then output "bestreden_besluit_herroepen" is true
    Then output "bezwaar_of_beroepschrift_bevat_wat_artikel_6_5_eist" is true
    Then output "beslistermijn_bezwaar_einddatum" equals "2027-01-12"

  Scenario: Geeft de heroverweging geen aanleiding, dan blijft het besluit in stand
    # Het hoofd oordeelt niets opnieuw: artikel 13a weigert opnieuw.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_ontvankelijk" is true
    Then output "heroverweging_geeft_aanleiding" is false
    Then output "bestreden_besluit_herroepen" is false
    Then output "nieuw_besluit" is absent

  Scenario: Een bezwaarschrift na het einde van de termijn is niet ontvankelijk
    # Artikel 6:9 gaf: niet tijdig ingediend. Artikel 6:11 (verschoonbare
    # termijnoverschrijding) is niet gemodelleerd. Er vindt geen
    # heroverweging plaats (lid 1), dus het hoofd stelt niets opnieuw vast.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | false |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_ontvankelijk" is false
    Then output "heroverweging_geeft_aanleiding" is absent
    Then output "bestreden_besluit_herroepen" is false
    Then output "nieuw_besluit" is absent

  Scenario: Tegen een beslissing op bezwaar staat geen bezwaar open
    # Daartegen staat beroep open (artikel 7:1, eerste lid, onder a).
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_ontvankelijk" is false
    Then output "heroverweging_geeft_aanleiding" is absent
    Then output "bestreden_besluit_herroepen" is false
    Then output "nieuw_besluit" is absent

  Scenario: Tegen het nieuwe besluit dat op bezwaar is genomen staat geen bezwaar open
    # Het besluit berust op artikel 13a, maar is genomen op het bezwaar: in
    # de plaats van een herroepen weigering. Ook daartegen staat beroep open
    # (artikel 7:1, eerste lid, onder a).
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "algemene_wet_bestuursrecht#7:11", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": true, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": true}} |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution succeeds
    Then output "bezwaar_ontvankelijk" is false
    Then output "heroverweging_geeft_aanleiding" is absent
    Then output "bestreden_besluit_herroepen" is false
    Then output "nieuw_besluit" is absent

  Scenario: Een opnieuw vastgestelde bevinding die geen artikel vraagt, is een fout
    # Een verschreven naam valt niet stil weg: de heroverweging zou dan de
    # oude bevinding houden.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    Given the following findings:
      | specifieke_aard_beroep | true |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution fails with "'specifieke_aard_beroep'"

  Scenario: Een bevinding bij een niet-ontvankelijk bezwaar is een fout
    # Er vindt geen heroverweging plaats (lid 1), dus geen artikel vraagt
    # haar.
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | false |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    Given the following findings:
      | specifieke_aard_beroep_ouder | true |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution fails with "'specifieke_aard_beroep_ouder'"

  Scenario: Een opnieuw vastgestelde bevinding van een ander type is een fout
    Given the following parameters:
      | bezwaar_of_beroepschrift_tijdig_ingediend | true |
      | bestreden_besluit | {"basis": "leerplichtwet_1969#13a", "taken_on": "leerplichtwet_1969#13a", "fields": {"bsn_jongere": "999990019", "eerste_verlofdag": "2027-02-01", "laatste_verlofdag": "2027-02-05"}, "findings": {"verlof_al_verleend_in_schooljaar_volgens_hoofd": false, "specifieke_aard_beroep_ouder": false, "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true}, "outputs": {"verlofdagen": 5, "verlof_raakt_eerste_twee_lesweken": false, "verlof_verleend": false}} |
    Given the following findings:
      | specifieke_aard_beroep_ouder | ja |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution fails with "a boolean for the finding 'specifieke_aard_beroep_ouder'"

  Scenario: Een bestreden besluit zonder taken_on is een fout
    # Het artikel van de indiening waarop het besluit is genomen ontbreekt.
    # Telde het als een besluit dat niet op bezwaar is genomen, dan was een
    # bezwaar tegen een beslissing op bezwaar stil ontvankelijk.
    Given the following parameters:
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution fails with "parameter 'bestreden_besluit' of algemene_wet_bestuursrecht#7:11 is no decision value {basis, taken_on, fields, findings, outputs}: it lacks 'taken_on'"

  Scenario: Een bestreden besluit met taken_on null is een fout
    # Null is geen artikel van een indiening. Telde het als een besluit dat
    # niet op bezwaar is genomen, dan was een bezwaar tegen een beslissing op
    # bezwaar stil ontvankelijk.
    Given the following parameters:
      | bestreden_besluit | {"basis": "algemene_wet_bestuursrecht#7:11", "taken_on": null, "fields": {"bezwaar_of_beroepschrift_tijdig_ingediend": true}, "findings": {}, "outputs": {"bezwaar_ontvankelijk": true, "heroverweging_geeft_aanleiding": false, "bestreden_besluit_herroepen": false, "nieuw_besluit": null}} |
    When I evaluate outputs "bestreden_besluit_herroepen, nieuw_besluit" of "algemene_wet_bestuursrecht"
    Then the execution fails with "parameter 'bestreden_besluit' of algemene_wet_bestuursrecht#7:11 is no decision value {basis, taken_on, fields, findings, outputs}: it holds no article reference in 'taken_on'"
