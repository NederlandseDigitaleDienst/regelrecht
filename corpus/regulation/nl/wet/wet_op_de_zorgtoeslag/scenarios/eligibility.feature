Feature: Zorgtoeslag eligibility

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

  Scenario: Meerderjarige met actieve polis heeft recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 79547                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  # NB: Engine currently returns true for minors — age check was removed (#375)
  # because AWIR Art 10 (verzekeringsplicht vs meeverzekerd) is not yet modeled.
  # This scenario asserts false as the desired outcome, not the current engine result.
  @wip
  Scenario: Minderjarige heeft geen recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 2008-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 0               | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false

  Scenario: Laag inkomen alleenstaande heeft recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1998-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 20000                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 20000           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 10000     | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  Scenario: Student met studiefinanciering heeft recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 2004-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 15000                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 15000           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given the following "inschrijvingen" data with key "bsn":
      | bsn         | onderwijstype |
      | "999993653" | WO            |
    Given the following "studiefinanciering" data with key "bsn":
      | bsn         | aantal_studerend_gezin |
      | "999993653" | 0                      |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  # Artikel 2 lid 2, tweede zin: voor een verzekerde met een partner telt het
  # gezamenlijke toetsingsinkomen. Samen 35.000 + 20.000 = 55.000 euro.
  # Normpremie (lid 3, partnerpercentages): 4,273% x 28.200,96 = 1.205,03,
  # plus 13,7% x (55.000 - 28.200,96) = 3.671,47, samen 4.876,50 euro. Dat is
  # meer dan tweemaal de standaardpremie (2 x 2.112 = 4.224), dus geen
  # verschil en geen aanspraak. Met alleen het eigen inkomen (35.000) kwam hier
  # 2.087,50 euro uit; dat was de fout van #377.
  Scenario: Partner met gecombineerd inkomen boven de inkomensgrens heeft geen recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn | heeft_gehele_berekeningsjaar_dezelfde_partner |
      | "999993653" | HUWELIJK          | "999993654" | true                                          |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 3500000                   | 0                         | 0                     | 0                               | 0            |
      | "999993654" | 2000000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 3500000         | 0                                    |
      | "999993654" | true                                       | 2000000         | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
      | "999993654" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
      | "999993654" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false
    Then output "in_aanmerking_genomen_toetsingsinkomen" equals 5500000
    Then output "hoogte_zorgtoeslag" equals 0

  # Samen 25.000 + 15.000 = 40.000 euro, beide zonder vermogen. Normpremie:
  # 1.205,03 + 13,7% x (40.000 - 28.200,96) = 1.205,03 + 1.616,47 = 2.821,50
  # euro (exact 2.821,4955008). Toeslag: 4.224 - 2.821,4955 = 1.402,5045,
  # afgerond 1.402,50 euro. Alleen het eigen inkomen (25.000, onder de
  # drempel) zou 4.224 - 1.205,03 = 3.018,97 euro hebben gegeven.
  Scenario: Partner zonder vermogen met gezamenlijk inkomen heeft recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn | heeft_gehele_berekeningsjaar_dezelfde_partner |
      | "999993653" | HUWELIJK          | "999993654" | true                                          |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2500000                   | 0                         | 0                     | 0                               | 0            |
      | "999993654" | 1500000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2500000         | 0                                    |
      | "999993654" | true                                       | 1500000         | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
      | "999993654" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
      | "999993654" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "in_aanmerking_genomen_toetsingsinkomen" equals 4000000
    Then output "hoogte_zorgtoeslag" equals 140250

  # Artikel 3 lid 1: de verzekerde heeft 100.000 euro, onder zijn eigen grens
  # van 141.896. Hij heeft het hele jaar dezelfde partner, met 90.000 euro;
  # samen 190.000, meer dan 179.429. Dus geen aanspraak. Het inkomen (20.000,
  # onder de drempel) had anders 4.224 - 1.205,03 = 3.018,97 euro gegeven.
  Scenario: Vermogen van de partner brengt het paar boven de gezamenlijke vermogensgrens
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn | heeft_gehele_berekeningsjaar_dezelfde_partner |
      | "999993653" | HUWELIJK          | "999993654" | true                                          |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2000000                   | 0                         | 0                     | 0                               | 0            |
      | "999993654" | 0                         | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2000000         | 0                                    |
      | "999993654" | true                                       | 0               | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
      | "999993654" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 10000000  | 0           | 0              | 0        |
      | "999993654" | 9000000   | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false

  # Dezelfde vermogens, maar de partner is niet het hele berekeningsjaar
  # dezelfde. De gezamenlijke toets van artikel 3 lid 1 geldt dan niet; alleen
  # de eigen grondslag (100.000 <= 141.896) wordt getoetst. De toeslag voor
  # een verzekerde met partner op de peildatum (artikel 2): gezamenlijk
  # inkomen 20.000, onder de drempel, dus 4.224 - 1.205,027 = 3.018,973,
  # afgerond 3.018,97 euro.
  Scenario: Partner niet het hele jaar: alleen het eigen vermogen telt
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn | heeft_gehele_berekeningsjaar_dezelfde_partner |
      | "999993653" | HUWELIJK          | "999993654" | false                                         |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2000000                   | 0                         | 0                     | 0                               | 0            |
      | "999993654" | 0                         | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2000000         | 0                                    |
      | "999993654" | true                                       | 0               | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
      | "999993654" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 10000000  | 0           | 0              | 0        |
      | "999993654" | 9000000   | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 301897

  # Artikel 3 lid 1 naar de letter: twee gronden verbonden door "of". De eigen
  # grondslag van de verzekerde (150.000) is meer dan 141.896, dus geen
  # aanspraak, hoewel het gezamenlijke vermogen (150.000) onder de 179.429
  # blijft. Dienst Toeslagen toetst bij een toeslagpartner alleen het
  # gezamenlijke vermogen en zou hier wel toekennen; dit scenario legt vast
  # dat het model de letter volgt (zie de toelichting bij artikel 3).
  Scenario: Eigen vermogen boven de grens sluit uit, ook met een partner
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn | heeft_gehele_berekeningsjaar_dezelfde_partner |
      | "999993653" | HUWELIJK          | "999993654" | true                                          |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2000000                   | 0                         | 0                     | 0                               | 0            |
      | "999993654" | 0                         | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2000000         | 0                                    |
      | "999993654" | true                                       | 0               | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
      | "999993654" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 15000000  | 0           | 0              | 0        |
      | "999993654" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false

  # Artikel 3 lid 1, eerste grond, voor een verzekerde zonder partner: een
  # rendementsgrondslag van 150.000 euro is meer dan 141.896. Geen partner,
  # dus de gezamenlijke toets doet niet mee; de registratie hoeft daarvoor
  # ook niets te zeggen over "het gehele berekeningsjaar dezelfde partner".
  Scenario: Alleenstaande met vermogen boven de grens heeft geen recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2000000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2000000         | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 15000000  | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false

  # NB: Toetsingsinkomen excludes box3 — Art 5.2a forfaitair rendement is not
  # yet implemented (#383). Only box1 income counts toward the toeslag amount.
  Scenario: Alleenstaande met box3 vermogen heeft recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2000000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2000000         | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 7000000   | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  Scenario: Verdragsinschrijving geeft verzekeringsdekking bij inactieve polis
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1985-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | VERLOPEN     | true                 |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 25000                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 25000           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  # === Standaardpremie (regeling_standaardpremie) ===

  Scenario: Standaardpremie 2025 uit regeling_standaardpremie
    Given the calculation date is "2025-01-01"
    When I evaluate "standaardpremie" of "regeling_standaardpremie"
    Then the execution succeeds
    Then output "standaardpremie" equals 211200

  Scenario: Standaardpremie 2024 uit regeling_standaardpremie
    Given the calculation date is "2024-01-01"
    When I evaluate "standaardpremie" of "regeling_standaardpremie"
    Then the execution succeeds
    Then output "standaardpremie" equals 198700

  # === 2024 toeslagbedragen ===

  Scenario: Meerderjarige heeft recht op zorgtoeslag (2024)
    Given the calculation date is "2024-01-01"
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 79547                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    # NB: ~197205 eurocent = EUR 1.972,05. Exact-decimal arithmetic (RFC-024)
    # yields a sub-cent amount; the model applies no whole-cent rounding (that
    # would be an explicit ROUND op, RFC-023/024), so the exact value stands.
    Then output "hoogte_zorgtoeslag" equals 197205.31187

  # De 2024-versie van artikel 2 lid 2 en artikel 3 met een partner. Het
  # bedrag wordt hier bewust niet getoetst: de drempel in die versie is nog
  # een inkomensgrens (#1564). Wel dat het toetsingsinkomen gezamenlijk is
  # (25.000 + 15.000 = 40.000 euro) en dat het gezamenlijke vermogen
  # (100.000 + 70.000 = 170.000 euro) boven de partnergrens van dat bestand
  # (161.329 euro) de aanspraak uitsluit, terwijl het eigen vermogen onder de
  # grens zonder partner (127.582 euro) blijft.
  Scenario: Partner met gezamenlijk inkomen en vermogen boven de grens (2024)
    Given the calculation date is "2024-01-01"
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | HUWELIJK          | "999993654" |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 2500000                   | 0                         | 0                     | 0                               | 0            |
      | "999993654" | 1500000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 2500000         | 0                                    |
      | "999993654" | true                                       | 1500000         | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
      | "999993654" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 10000000  | 0           | 0              | 0        |
      | "999993654" | 7000000   | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "in_aanmerking_genomen_toetsingsinkomen" equals 4000000
    Then output "heeft_recht_op_zorgtoeslag" is false

  # NB: Art 2 no longer checks age directly — that was a scope violation.
  # The Zvw also does not check age for is_verzekerd (minors ARE verzekerd
  # per Art 2 lid 3 Zvw). So an under-18 with active insurance IS entitled.
  # The under-18 person with zero income gets the maximum toeslag
  # (standaardpremie 2024 = EUR 1.987,00).
  Scenario: Minderjarige met actieve polis heeft recht op zorgtoeslag (2024)
    Given the calculation date is "2024-01-01"
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 2007-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 0               | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    # NB: 198700 eurocent = EUR 1.987,00
    Then output "hoogte_zorgtoeslag" equals 198700

  # NB: Forensische zorg exclusion was removed as scope violation (#375).
  # It belongs in Zvw Art 24 or Wfz, not in the zorgtoeslag law.
  Scenario: Forensische zorg heeft geen invloed op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1985-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 25000                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 25000           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | GGZ      | TBS                  |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  # De afbouwtak van artikel 2 lid 3: boven het drempelinkomen loopt de
  # zorgtoeslag terug met 13,700% van het meerdere. Zonder een scenario hier
  # bleef onopgemerkt dat de wet inkomensonafhankelijk was geworden (#1429):
  # elk inkomen onder de grens gaf hetzelfde bedrag, en alle scenario's zaten
  # onder die grens. Deze twee liggen er bewust boven.
  Scenario: Inkomen boven het drempelinkomen bouwt de zorgtoeslag af
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 3000000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 3000000         | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 133084

  # Bij de inkomensgrens is de normpremie gelijk aan de standaardpremie en
  # houdt het recht op. Voor 2025 ligt die grens op 39.719 euro, het bedrag dat
  # VWS publiceert; dat de berekening daar op nul uitkomt is de controle.
  # Er is een aanslag, dus het toetsingsinkomen is het verzamelinkomen
  # (artikel 21, onderdeel e, 1°, AWR); het lage belastbare loon mag de
  # uitkomst niet raken.
  Scenario: Boven de inkomensgrens bestaat geen recht op zorgtoeslag
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 4500000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false
    Then output "hoogte_zorgtoeslag" equals 0

  # Artikel 8, eerste lid, Awir: het toetsingsinkomen is het inkomensgegeven.
  # Zonder aanslag of navorderingsaanslag inkomstenbelasting is dat het
  # belastbare loon (artikel 21, onderdeel e, 2°, AWR), niet het
  # verzamelinkomen. De box 1-gegevens staan hier boven de inkomensgrens en
  # zouden tot nul leiden; het belastbare loon is dat van het eerste scenario,
  # en de uitkomst is daarom ook die van het eerste scenario.
  Scenario: Zonder aanslag inkomstenbelasting telt het belastbare loon als toetsingsinkomen
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 2005-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 4500000                   | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | false                                      | 79547           | 0                                    |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731

  # Artikel 8, tweede lid, Awir: niet in Nederland belastbaar inkomen, zoals
  # dat bij beschikking is vastgesteld, telt mee. 795,47 euro inkomensgegeven
  # plus 44.204,53 euro van die beschikking is 45.000 euro, het inkomen van het
  # scenario hierboven dat boven de grens valt.
  Scenario: Niet in Nederland belastbaar inkomen telt mee in het toetsingsinkomen
    Given the following "personal_data" data with key "bsn":
      | bsn         | geboortedatum | verblijfsadres | land_verblijf |
      | "999993653" | 1990-01-01    | Amsterdam      | NEDERLAND     |
    Given the following "relationship_data" data with key "bsn":
      | bsn         | partnerschap_type | partner_bsn |
      | "999993653" | GEEN              | null        |
    Given the following "insurance" data with key "bsn":
      | bsn         | polis_status | verdragsinschrijving |
      | "999993653" | ACTIEF       | false                |
    Given the following "box1" data with key "bsn":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning |
      | "999993653" | 79547                     | 0                         | 0                     | 0                               | 0            |
    Given the following "inkomensgegevens" data with key "bsn":
      | bsn         | aanslag_of_navorderingsaanslag_vastgesteld | belastbaar_loon | niet_in_nederland_belastbaar_inkomen |
      | "999993653" | true                                       | 79547           | 4420453                              |
    Given the following "box2" data with key "bsn":
      | bsn         | reguliere_voordelen | vervreemdingsvoordelen |
      | "999993653" | 0                   | 0                      |
    Given the following "box3" data with key "bsn":
      | bsn         | spaargeld | beleggingen | onroerend_goed | schulden |
      | "999993653" | 0         | 0           | 0              | 0        |
    Given the following "detenties" data with key "bsn":
      | bsn         | detentiestatus | inrichting_type | zorgtype | juridische_grondslag |
      | "999993653" | null           | null            | null     | null                 |
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is false
    Then output "hoogte_zorgtoeslag" equals 0
