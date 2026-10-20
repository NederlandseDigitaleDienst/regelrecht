# Converted from overig/alcoholwet_GEMEENTE_ROTTERDAM-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen recht op Alcoholwetvergunning horeca Rotterdam
  Als horecaondernemer in Rotterdam
  Wil ik weten of ik een Alcoholwetvergunning kan krijgen
  Zodat ik alcoholhoudende dranken mag verstrekken voor gebruik ter plaatse

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Succesvolle aanvraag - alle eisen artikel 8 en 10 voldaan
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning |
      | 85234567   | 999999990 | 50                                | horecabedrijf | null                    |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                  |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"VOF","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1999-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning" of "alcoholwet/vergunning/rotterdam"
    # Art. 8 lid 1 onder b Alcoholwet: de demo heeft geen levensgedrag-register, dus het levensgedrag is onbekend en de uitkomst ook (was true via een constante false)
    Then output "voldoet_aan_voorwaarden" is unknown
    And output "heeft_recht_op_vergunning" is unknown

  Scenario: Succesvolle aanvraag - precies 21 jaar (grenswaarde artikel 8 lid 1 onder a)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning |
      | 85234567   | 999999990 | 40                                | horecabedrijf | null                    |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 2003-06-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning" of "alcoholwet/vergunning/rotterdam"
    # Art. 8 lid 1 onder b Alcoholwet: de demo heeft geen levensgedrag-register, dus het levensgedrag is onbekend en de uitkomst ook (was true via een constante false)
    Then output "voldoet_aan_voorwaarden" is unknown
    And output "heeft_recht_op_vergunning" is unknown

  Scenario: Succesvolle aanvraag - precies 35 m2 vloeroppervlakte (grenswaarde artikel 10 lid 2)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning |
      | 85234567   | 999999990 | 35                                | horecabedrijf | null                    |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning" of "alcoholwet/vergunning/rotterdam"
    # Art. 8 lid 1 onder b Alcoholwet: de demo heeft geen levensgedrag-register, dus het levensgedrag is onbekend en de uitkomst ook (was true via een constante false)
    Then output "voldoet_aan_voorwaarden" is unknown
    And output "heeft_recht_op_vergunning" is unknown

  # In elke afwijzing is precies één voorwaarde niet vervuld: de SVH-registratie
  # is geldig, het levensgedrag is bekend en in orde, en de weigeringsgrond
  # bevestigt welke voorwaarde de doorslag gaf. De weigeringsgrond wordt los van
  # voldoet_aan_voorwaarden berekend; het controlescenario na de Bibob-afwijzing
  # bewijst dat dezelfde gegevens zonder afwijzingsgrond wel slagen.
  # NB: alcoholwet/vergunning zet voldoet_aan_nationale_eisen op een constante
  # true, dus een weigeringsgrond uit artikel 8 of 10 bereikt deze wet niet.
  # Vroeger wees dit scenario af omdat de SVH-registratie ontbrak; nu de andere
  # voorwaarden vervuld zijn, legt het de gewenste uitkomst vast.
  @wip
  Scenario: Afwijzing - exploitant is 20 jaar (niet voldaan aan artikel 8 lid 1 onder a)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 50                                | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 2004-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Leidinggevende heeft de leeftijd van 21 jaar niet bereikt (artikel 8 lid 1 onder a)"

  # NB: alcoholwet/vergunning zet voldoet_aan_nationale_eisen op een constante
  # true, dus een weigeringsgrond uit artikel 8 of 10 bereikt deze wet niet.
  # Vroeger wees dit scenario af omdat de SVH-registratie ontbrak; nu de andere
  # voorwaarden vervuld zijn, legt het de gewenste uitkomst vast.
  @wip
  Scenario: Afwijzing - exploitant is 18 jaar (significant onder minimumleeftijd)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 50                                | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 2006-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Leidinggevende heeft de leeftijd van 21 jaar niet bereikt (artikel 8 lid 1 onder a)"

  # NB: alcoholwet/vergunning zet voldoet_aan_nationale_eisen op een constante
  # true, dus een weigeringsgrond uit artikel 8 of 10 bereikt deze wet niet.
  # Vroeger wees dit scenario af omdat de SVH-registratie ontbrak; nu de andere
  # voorwaarden vervuld zijn, legt het de gewenste uitkomst vast.
  @wip
  Scenario: Afwijzing - exploitant staat onder curatele (artikel 8 lid 1 onder c)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 50                                | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus                                                                                                                                   |
      | 999999990 | [{"bsn_curator":"999410001","bsn_curandus":"999999990","naam_curandus":"Exploitant","datum_ingang":"2022-01-15","datum_einde":null,"status":"ACTIEF"}] |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1985-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Leidinggevende staat onder curatele (artikel 8 lid 1 onder c)"

  # NB: alcoholwet/vergunning zet voldoet_aan_nationale_eisen op een constante
  # true, dus een weigeringsgrond uit artikel 8 of 10 bereikt deze wet niet.
  # Vroeger wees dit scenario af omdat de SVH-registratie ontbrak; nu de andere
  # voorwaarden vervuld zijn, legt het de gewenste uitkomst vast.
  @wip
  Scenario: Afwijzing - vloeroppervlakte horecalokaliteit kleiner dan 35 m2 (artikel 10 lid 2)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 30                                | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Horecalokaliteit heeft minder dan 35 m² vloeroppervlakte (artikel 10 lid 2)"

  # NB: alcoholwet/vergunning zet voldoet_aan_nationale_eisen op een constante
  # true, dus een weigeringsgrond uit artikel 8 of 10 bereikt deze wet niet.
  # Vroeger wees dit scenario af omdat de SVH-registratie ontbrak; nu de andere
  # voorwaarden vervuld zijn, legt het de gewenste uitkomst vast.
  @wip
  Scenario: Afwijzing - vloeroppervlakte net onder minimum (34 m2)
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 34                                | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                          |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Horecalokaliteit heeft minder dan 35 m² vloeroppervlakte (artikel 10 lid 2)"

  Scenario: Afwijzing - ernstig gevaar volgens Bibob-advies
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 100                               | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                 |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"BV","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | true               | ernstig_gevaar         | 2024-03-01   | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1980-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Ernstig gevaar volgens Bibob-advies"

  # Controle bij de Bibob-afwijzing: dezelfde gegevens zonder Bibob-advies
  # voldoen aan alle voorwaarden. Zo wijst dat scenario af om het advies, en
  # niet om een SVH-registratie of levensgedrag dat ongemerkt ongeldig werd.
  Scenario: Controle - zelfde gegevens zonder Bibob-advies voldoen aan alle voorwaarden
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 100                               | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                 |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"BV","status":"Actief","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1980-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is true

  Scenario: Afwijzing - onderneming is uitgeschreven uit handelsregister
    Given parameter "kvk_nummer" is "85234567"
    And the following "SVH" data with key "bsn" for law "alcoholwet/register_sociale_hygiene":
      | bsn       | registratie                                                                                              |
      | 999999990 | {"bsn":"999999990","is_geregistreerd":true,"registratienummer":null,"naam":null,"registratiedatum":null} |
    And the following "GEMEENTE_ROTTERDAM" data with key "kvk_nummer" for law "alcoholwet/vergunning/rotterdam":
      | kvk_nummer | bsn       | vloeroppervlakte_horecalokaliteit | type_bedrijf  | heeft_alcoholvergunning | slecht_levensgedrag_bij_verlening |
      | 85234567   | 999999990 | 50                                | horecabedrijf | null                    | false                             |
    And the following "RECHTSPRAAK" data with key "bsn" for law "burgerlijk_wetboek_handelingsonbekwaamheid":
      | bsn       | curatele_als_curandus |
      | 999999990 | []                    |
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                 |
      | 85234567   | {"kvk_nummer":"85234567","rechtsvorm":"Eenmanszaak","status":"Uitgeschreven","aantal_werknemers":0,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":null} |
    And the following "LBB" data with key "kvk_nummer" for law "wet_bibob":
      | kvk_nummer | advies_uitgebracht | advies_mate_van_gevaar | advies_datum | relatie_tot_strafbare_feiten | financieringsrisico | voorschriften_geadviseerd |
      | 85234567   | null               | null                   | null         | null                         | null                | null                      |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn       | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | 999999990 | 1990-01-01    | null              | null        | []                | []             |               |               | null  | []           |                       |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_recht_op_vergunning, weigeringsgrond" of "alcoholwet/vergunning/rotterdam"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_recht_op_vergunning" is false
    And output "weigeringsgrond" equals "Onderneming is niet actief in het handelsregister"
