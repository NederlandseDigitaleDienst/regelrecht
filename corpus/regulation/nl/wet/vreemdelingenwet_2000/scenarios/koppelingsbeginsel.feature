Feature: Het koppelingsbeginsel haakt in op de toekenning van een tegemoetkoming
  Vreemdelingenwet 2000 artikel 10, eerste lid: "De vreemdeling die geen
  rechtmatig verblijf heeft, kan geen aanspraak maken op toekenning van
  verstrekkingen, voorzieningen en uitkeringen bij wege van een beschikking
  van een bestuursorgaan." De beschikking van Awir artikel 14 die een
  zorgtoeslag toekent, is zo'n toekenning. Voor een vreemdeling zonder
  rechtmatig verblijf ontstaat de toegekende tegemoetkoming niet.

  Background:
    Given the calculation date is "2025-01-01"
    Given law "wet_basisregistratie_personen" is loaded
    Given law "zorgverzekeringswet" is loaded
    Given law "penitentiaire_beginselenwet" is loaded
    Given law "regeling_standaardpremie" is loaded
    Given law "algemene_wet_inkomensafhankelijke_regelingen" is loaded
    Given law "algemene_wet_inzake_rijksbelastingen" is loaded
    Given law "wet_inkomstenbelasting_2001" is loaded
    Given law "wet_forensische_zorg" is loaded
    Given law "vreemdelingenwet_2000" is loaded

  Scenario: Een Nederlander krijgt de zorgtoeslag toegekend
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn       | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | 999993653 | true                                       | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given the following parameters:
      | bsn            | 999993653                  |
      | tegemoetkoming | wet_op_de_zorgtoeslag#2    |
      | is_vreemdeling | false                      |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "toegekende_tegemoetkoming" has direct provenance
    Then output "geen_aanspraak" is false

  Scenario: Een vreemdeling met rechtmatig verblijf krijgt de zorgtoeslag toegekend
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn       | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | 999993653 | true                                       | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given the following parameters:
      | bsn                       | 999993653               |
      | tegemoetkoming            | wet_op_de_zorgtoeslag#2 |
      | is_vreemdeling            | true                    |
      | heeft_rechtmatig_verblijf | true                    |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "toegekende_tegemoetkoming" has direct provenance
    Then output "geen_aanspraak" is false

  Scenario: Een vreemdeling zonder rechtmatig verblijf kan geen aanspraak maken
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres | land_verblijf |
      | 999993653 | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn       | partnerschap_type | partner_bsn |
      | 999993653 | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn       | polis_status | verdragsinschrijving |
      | 999993653 | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn       | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | 999993653 | 79547                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn       | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | 999993653 | true                                       | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn       | reguliere_voordelen | vervreemdingsvoordelen |
      | 999993653 | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn       | spaargeld | beleggingen | onroerend_goed | schulden |
      | 999993653 | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn       | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | 999993653 | null           | null            | null     | null                 |
    Given the following parameters:
      | bsn                       | 999993653               |
      | tegemoetkoming            | wet_op_de_zorgtoeslag#2 |
      | is_vreemdeling            | true                    |
      | heeft_rechtmatig_verblijf | false                   |
    When I evaluate "toegekende_tegemoetkoming" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution fails with "kan geen aanspraak maken op toekenning"
