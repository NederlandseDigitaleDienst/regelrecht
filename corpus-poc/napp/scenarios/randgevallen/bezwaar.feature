Feature: Bezwaar tegen een beschikking (AWB hoofdstuk 6 en 7)
  De vereisten aan het bezwaarschrift (6:5), het verzuimherstel (6:6), de
  tijdigheid met verzendtheorie (6:9), het afzien van horen (7:3) en de
  beslistermijn (7:10) komen uit de wet; de orchestratie levert feiten.

  Scenario: Een compleet bezwaarschrift voldoet aan de vereisten
    Given the calculation date is "2026-06-01"
    And the following facts:
      | naam_en_adres_vermeld | true |
      | dagtekening_vermeld   | true |
      | besluit_omschreven    | true |
      | gronden_vermeld       | true |
      | ondertekend           | true |
    When the AWB outputs "voldoet_aan_bezwaarschriftvereisten" are evaluated
    Then the output "voldoet_aan_bezwaarschriftvereisten" is true

  Scenario: Zonder gronden ontbreekt een vereiste en is herstel nodig
    Given the calculation date is "2026-06-01"
    And the following facts:
      | naam_en_adres_vermeld           | true  |
      | dagtekening_vermeld             | true  |
      | besluit_omschreven              | true  |
      | gronden_vermeld                 | false |
      | ondertekend                     | true  |
      | herstelgelegenheid_geboden      | false |
      | binnen_hersteltermijn_aangevuld | false |
    When the AWB outputs "ontbreken_gronden, herstel_vereist, mag_niet_ontvankelijk_wegens_verzuim" are evaluated
    Then the output "ontbreken_gronden" is true
    And the output "herstel_vereist" is true
    And the output "mag_niet_ontvankelijk_wegens_verzuim" is false

  Scenario: De verzendtheorie redt een per post verzonden bezwaarschrift
    Given the calculation date is "2026-06-01"
    And the following facts:
      | ontvangen_voor_einde_termijn        | false |
      | ter_post_bezorgd_voor_einde_termijn | true  |
      | ontvangen_binnen_week_na_termijn    | true  |
    When the AWB outputs "bezwaar_tijdig" are evaluated
    Then the output "bezwaar_tijdig" is true

  Scenario: Te laat ontvangen en niet tijdig ter post bezorgd is niet tijdig
    Given the calculation date is "2026-06-01"
    And the following facts:
      | ontvangen_voor_einde_termijn        | false |
      | ter_post_bezorgd_voor_einde_termijn | false |
      | ontvangen_binnen_week_na_termijn    | true  |
    When the AWB outputs "bezwaar_tijdig" are evaluated
    Then the output "bezwaar_tijdig" is false

  Scenario: Afzien van horen kan alleen op een grond uit artikel 7:3
    Given the calculation date is "2026-06-01"
    And the following facts:
      | kennelijk_niet_ontvankelijk | false |
      | kennelijk_ongegrond         | false |
      | indiener_ziet_af_van_horen  | false |
      | volledig_tegemoetgekomen    | false |
    When the AWB outputs "mag_afzien_van_horen" are evaluated
    Then the output "mag_afzien_van_horen" is false
