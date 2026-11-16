Feature: Bezwaar tegen een beschikking (AWB hoofdstuk 6 en 7)
  De vereisten aan het bezwaarschrift (6:5), het verzuimherstel (6:6), de
  tijdigheid met verzendtheorie (6:9), het afzien van horen (7:3) en de
  beslistermijn (7:10) komen uit de wet; de orchestratie levert feiten.

  Scenario: Niet-ontvankelijk kan pas na onbenutte herstelgelegenheid
    Given the calculation date is "2026-06-01"
    And the following facts:
      | naam_en_adres_vermeld           | true  |
      | dagtekening_vermeld             | true  |
      | besluit_omschreven              | true  |
      | gronden_vermeld                 | false |
      | ondertekend                     | true  |
      | herstelgelegenheid_geboden      | true  |
      | binnen_hersteltermijn_aangevuld | false |
    When the AWB outputs "mag_niet_ontvankelijk_wegens_verzuim" are evaluated
    Then the output "mag_niet_ontvankelijk_wegens_verzuim" is true
