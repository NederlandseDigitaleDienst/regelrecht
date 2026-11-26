Feature: Uitzicht op inkomensverbetering (Beleidsregels IIT Groningen)
  Het college beoordeelt of belanghebbende uitzicht heeft op inkomensverbetering
  (artikel 2). Wie studeert of kort geleden studeerde, heeft dat uitzicht en geen recht
  (artikel 3). Wie op de peildatum in een van de situaties van artikel 4 zit, heeft dat
  uitzicht niet en heeft recht, tenzij hij in het peiljaar een maatregel kreeg wegens het
  niet nakomen van de arbeidsverplichtingen (artikel 5). In andere situaties beslist het
  college per geval (de derde leden en artikel 2, eerste lid).

  # Bron: toelichting bij de beleidsregels, Gemeenteblad 2015, 124380 (de authentieke
  #   tekst, niet de CVDR-consolidatie: die keert drie zinnen om, bevinding W14)
  #   (dossiers/iit-groningen/bronnen/gemeente/tekst/gmb-2015-124380.txt)
  # URL: https://zoek.officielebekendmakingen.nl/gmb-2015-124380.html
  #
  # De uitkomst is alleen de beoordeling van uitzicht en inspanningen. Leeftijd,
  # vermogen en langdurig laag inkomen (artikel 36 Participatiewet en de verordening)
  # staan hier niet in.

  Background:
    Given the calculation date is "2023-05-01"
    Given the following parameters:
      | heeft_aanvraag_om_iit_ingediend                      | true       |
      | datum_aanvraag                                       | 2023-05-01 |
      | volgt_op_peildatum_opleiding_wtos                    | false      |
      | volgt_op_peildatum_studie_wsf_2000                   | false      |
      | laatste_dag_opleiding_wtos                           | null       |
      | laatste_dag_studie_wsf_2000                          | null       |
      | datum_maatregel_artikel_18_vierde_lid                | null       |
      | datum_maatregel_afstemmingsverordening_artikel_6_of_7 | null       |

  # === Artikel 3: uitzicht op inkomensverbetering, geen recht ===

  Scenario: Een student heeft uitzicht op inkomensverbetering en geen recht
    # Bron: toelichting, regel 59: "Dat is het geval bij scholieren en studenten." en
    # regel 60: "Er is in die gevallen geen recht op individuele inkomenstoeslag."
    Given the following parameters:
      | volgt_op_peildatum_studie_wsf_2000 | true |
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | false |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is false

  Scenario: Wie in de referteperiode zijn studie afrondde, heeft nog uitzicht op inkomensverbetering
    # Bron: toelichting, regel 59: het uitzicht op inkomensverbetering valt "niet meteen
    # weg ... nadat zij hun opleiding of studie hebben voltooid".
    Given parameter "laatste_dag_studie_wsf_2000" is "2022-06-30"
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | false |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is false

  Scenario: Een werkervaringsplaats met baangarantie: het college kan afwijzen
    # Bron: toelichting, regel 61: "Dat zou bijvoorbeeld het geval kunnen zijn als
    # belanghebbende een werkervaringsplaats met baangarantie is aangeboden". Een open norm:
    # het college stelt dit vast (artikel 3, derde lid).
    Given the following parameters:
      | afwijzing_op_grond_van_artikel_3_derde_lid | true |
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | false |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is false

  # === Artikel 4: geen uitzicht op inkomensverbetering, wel recht ===

  Scenario: Wie duurzaam volledig arbeidsongeschikt is, heeft recht
    # Bron: toelichting, regel 63: "duurzaam volledig arbeidsongeschikt zijn"; regel 70: "Belanghebbende heeft in deze gevallen recht
    # op individuele inkomenstoeslag, tenzij artikel 5 van toepassing is."
    Given the following parameters:
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid | true |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: Een ontheffing van de arbeidsverplichtingen voor meer dan zes maanden geeft recht
    # Bron: toelichting, regel 64: "een tijdelijke ontheffing van de arbeidsverplichtingen hebben"; regel 70: "Belanghebbende heeft in deze gevallen recht
    # op individuele inkomenstoeslag, tenzij artikel 5 van toepassing is."
    Given the following parameters:
      | ontheffing_artikel_9_tweede_lid | true |
    Given parameter "duur_ontheffing_maanden" is 12
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: Wie langer dan een jaar ondersteuning bij re-integratie nodig heeft, heeft recht
    # Bron: toelichting, regel 67: "langer dan een jaar ondersteuning bij hun re-integratie nodig hebben"; regel 70: "Belanghebbende heeft in deze gevallen recht
    # op individuele inkomenstoeslag, tenzij artikel 5 van toepassing is."
    Given the following parameters:
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a | true |
    Given parameter "duur_aangewezen_op_ondersteuning_maanden" is 18
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: Wie is aangewezen op een participatieplaats, heeft recht
    # Bron: toelichting, regel 68: "(vooralsnog) zijn aangewezen op een participatiebaan of sociale activering of
    # vrijwilligerswerk"; regel 70: "Belanghebbende heeft in deze gevallen recht
    # op individuele inkomenstoeslag, tenzij artikel 5 van toepassing is."
    Given the following parameters:
      | aangewezen_op_participatieplaats_artikel_10a | true |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: Wie is aangewezen op vrijwilligerswerk, heeft recht
    # Bron: toelichting, regel 68; regel 70: "Belanghebbende heeft in deze gevallen recht
    # op individuele inkomenstoeslag, tenzij artikel 5 van toepassing is."
    Given the following parameters:
      | aangewezen_op_vrijwilligerswerk | true |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: Een ontheffing van vier maanden is geen situatie van artikel 4
    # Bron: tekst van artikel 4, eerste lid, onder c: "voor meer dan zes maanden". Zonder
    # oordeel van het college (artikel 2, eerste lid) is er geen uitkomst.
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | true  |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | false |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    Given parameter "duur_ontheffing_maanden" is 4
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is absent

  Scenario: Een medische urenbeperking met de vrijlating van onderdeel y valt buiten artikel 4 (W1)
    # Bron: toelichting, regel 66: "weliswaar deeltijdbaan hebben maar door een medische
    # urenbeperking niet meer uren kunnen werken". De tekst van artikel 4, onder e, vraagt
    # de vrijlating van artikel 31, tweede lid, onderdeel z; sinds 2016 is dat de vrijlating
    # bij loonkostensubsidie en staat die bij een medische urenbeperking in onderdeel y.
    # Het model volgt de tekst, dus de toelichting en de uitkomst lopen hier uiteen
    # (bevinding W1). Zonder oordeel van het college is er geen uitkomst.
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | true  |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    Given the following parameters:
      | vrijlating_artikel_31_tweede_lid_onderdeel_z | false |
      | vrijlating_artikel_8_zevende_lid_ioaw        | false |
      | vrijlating_artikel_8_elfde_lid_ioaz          | false |
    When I evaluate "geen_uitzicht_op_inkomensverbetering_artikel_4" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "geen_uitzicht_op_inkomensverbetering_artikel_4" is false

  Scenario: Bij een medische urenbeperking met onderdeel y beslist het college zelf (W1)
    # Bron: toelichting, regel 66: "weliswaar deeltijdbaan hebben maar door een medische
    # urenbeperking niet meer uren kunnen werken". De tekst van artikel 4, onder e, vraagt
    # de vrijlating van artikel 31, tweede lid, onderdeel z; sinds 2016 is dat de vrijlating
    # bij loonkostensubsidie en staat die bij een medische urenbeperking in onderdeel y.
    # Het model volgt de tekst, dus de toelichting en de uitkomst lopen hier uiteen
    # (bevinding W1). Zonder oordeel van het college is er geen uitkomst.
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | true  |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    Given the following parameters:
      | vrijlating_artikel_31_tweede_lid_onderdeel_z | false |
      | vrijlating_artikel_8_zevende_lid_ioaw        | false |
      | vrijlating_artikel_8_elfde_lid_ioaz          | false |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is absent

  # === Artikel 5: onvoldoende inspanningen, geen recht ===

  Scenario: Een maatregel in het peiljaar gaat voor artikel 4
    # Bron: toelichting, regel 70 ("tenzij artikel 5 van toepassing is") en regel 71:
    # "Belanghebbende heeft in ieder geval niet voldoende inspanningen verricht als hij een
    # maatregel opgelegd heeft gekregen die te maken heeft met zijn arbeidsverplichtingen".
    Given the following parameters:
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid | true |
    Given parameter "datum_maatregel_artikel_18_vierde_lid" is "2022-10-01"
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is false

  Scenario: Een maatregel van voor het peiljaar telt niet
    # Bron: tekst van artikel 5, eerste lid: "in het peiljaar"; het peiljaar is de twaalf
    # maanden voor de peildatum (artikel 1, onder g).
    Given the following parameters:
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid | true |
    Given parameter "datum_maatregel_artikel_18_vierde_lid" is "2022-03-01"
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: In deeltijd blijven werken terwijl meer uren kunnen: het college kan afwijzen
    # Bron: toelichting, regel 74: "dat belanghebbende in deeltijd werkt en er voor kiest om
    # hetzelfde aantal uren te blijven werken, ook al zijn er mogelijkheden om zijn inkomen
    # te verbeteren door langer te gaan werken." Een open norm (artikel 5, derde lid).
    Given the following parameters:
      | afwijzing_op_grond_van_artikel_5_derde_lid | true |
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | false |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is false

  # === Uit de tekst, niet uit de toelichting ===

  Scenario: Wie studeert en volledig arbeidsongeschikt is, valt onder artikel 3 en artikel 4 (W9)
    # Bron: tekst van artikel 3, tweede lid ("geen recht") en artikel 4, tweede lid ("heeft
    # recht"). De beleidsregels geven geen voorrang; het model geeft dan geen uitkomst.
    Given the following parameters:
      | volgt_op_peildatum_opleiding_wtos | true |
    Given the following parameters:
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid | true |
    When I evaluate "artikel_3_en_artikel_4_botsen" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "artikel_3_en_artikel_4_botsen" is true

  Scenario: Bij de botsing van artikel 3 en artikel 4 is er geen uitkomst (W9)
    # Bron: tekst van artikel 3, tweede lid ("geen recht") en artikel 4, tweede lid ("heeft
    # recht"). De beleidsregels geven geen voorrang; het model geeft dan geen uitkomst.
    Given the following parameters:
      | volgt_op_peildatum_opleiding_wtos | true |
    Given the following parameters:
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid | true |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is absent

  Scenario: In een andere situatie beslist het college
    # Bron: tekst van artikel 2, eerste lid: "Het college moet in het individuele geval
    # beoordelen".
    Given the following parameters:
      | ontvangt_arbeidsongeschiktheidsuitkering_volledig_arbeidsongeschikt     | false |
      | volledig_arbeidsongeschikt_artikel_9_vijfde_lid                         | false |
      | ontheffing_artikel_9_eerste_lid_behalve_onderdeel_c                     | false |
      | ontheffing_artikel_9_tweede_lid                                         | false |
      | ontheffing_artikel_37a_ioaw                                             | false |
      | ontheffing_artikel_37a_ioaz                                             | false |
      | ontheffing_arbeidsinschakeling_artikel_9a                               | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaw                          | false |
      | ontheffing_arbeidsinschakeling_artikel_38_ioaz                          | false |
      | medisch_urenbeperkt_artikel_6b                                          | false |
      | medisch_urenbeperkt_artikel_4b_ioaw                                     | false |
      | medisch_urenbeperkt_artikel_4b_ioaz                                     | false |
      | aangewezen_op_ondersteuning_artikel_7_eerste_lid_onderdeel_a            | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaw                  | false |
      | aangewezen_op_ondersteuning_artikel_36_eerste_lid_ioaz                  | false |
      | aangewezen_op_participatieplaats_artikel_10a                            | false |
      | aangewezen_op_sociale_activering_artikel_6_eerste_lid_onderdeel_a_sub_7 | false |
      | aangewezen_op_artikel_4a_ioaw                                           | false |
      | aangewezen_op_artikel_4a_ioaz                                           | false |
      | aangewezen_op_vrijwilligerswerk                                         | false |
    Given the following parameters:
      | oordeel_college_recht_op_iit | true |
    When I evaluate "recht_op_iit_volgens_beleidsregels" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "recht_op_iit_volgens_beleidsregels" is true

  Scenario: Ambtshalve verstrekken kan als het college de informatie heeft
    # Bron: tekst van artikel 5a. Zonder aanvraag is er geen peildatum (W8) en geldt tot
    # 2027 artikel 36 Participatiewet met zijn aanvraag (W5); dit scenario toetst alleen 5a.
    Given the following parameters:
      | college_beschikt_over_benodigde_informatie | true |
    When I evaluate "kan_ambtshalve_worden_verstrekt" of "beleidsregels_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "kan_ambtshalve_worden_verstrekt" is true
