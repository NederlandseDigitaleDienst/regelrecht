Feature: Informatieplicht Energiebesparing
  Als RVO
  Wil ik vaststellen of een organisatie energiebesparingsplicht, informatieplicht en
  onderzoeksplicht heeft
  Zodat grootverbruikers hun energiebesparingsmaatregelen rapporteren

  Background:
    Given the calculation date is "2024-01-01"

  Scenario: Grootverbruiker zonder woonfunctie boven de elektriciteitsdrempel heeft energiebesparings- en informatieplicht
    Given parameter "kvk_nummer" is "85234567"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                                       |
      | "85234567" | {"kvk_nummer":"85234567","rechtsvorm":"BV","status":"Actief","aantal_werknemers":50,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Nieuwe Binnenweg 225A, 3021GC Rotterdam"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                     | verblijfsobject                                                                    |
      | Nieuwe Binnenweg 225A, 3021GC Rotterdam   | {"gebruiksdoel":"bijeenkomstfunctie","oppervlakte":85,"status":"Actief","bouwjaar":1920} |
    And the following parameters:
      | elektriciteit_kwh | 62000 |
      | gasverbruik_m3    | 8000  |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_energiebesparingsplicht, heeft_informatieplicht, heeft_onderzoeksplicht" of "omgevingswet/energiebesparing/informatieplicht"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_energiebesparingsplicht" is true
    And output "heeft_informatieplicht" is true
    And output "heeft_onderzoeksplicht" is false

  Scenario: Woonfunctie op het BAG-adres is uitgezonderd van de energiebesparingsplicht
    Given parameter "kvk_nummer" is "12345678"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/bedrijfsgegevens":
      | kvk_nummer | organisatie_gegevens                                                                                                                                                                   |
      | "12345678" | {"kvk_nummer":"12345678","rechtsvorm":"BV","status":"Actief","aantal_werknemers":10,"datum_telling":null,"datum_aanvang":null,"vestigingsadres":"Dorpsstraat 1, 1234AB Voorbeeldstad"} |
    And the following "KADASTER" data with key "adres" for law "wet_bag":
      | adres                                | verblijfsobject                                                                |
      | Dorpsstraat 1, 1234AB Voorbeeldstad  | {"gebruiksdoel":"woonfunctie","oppervlakte":120,"status":"Actief","bouwjaar":1990} |
    And the following parameters:
      | elektriciteit_kwh | 62000 |
      | gasverbruik_m3    | 8000  |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_energiebesparingsplicht" of "omgevingswet/energiebesparing/informatieplicht"
    Then output "voldoet_aan_voorwaarden" is false
