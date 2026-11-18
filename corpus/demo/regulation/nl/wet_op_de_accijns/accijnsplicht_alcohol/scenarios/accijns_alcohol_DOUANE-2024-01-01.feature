Feature: Bepalen accijnsplicht en tarief voor alcoholhoudende dranken
  Als ondernemer die alcoholhoudende dranken uitslaat tot verbruik
  Wil ik weten of ik accijns verschuldigd ben en hoeveel
  Zodat ik weet wat ik moet aangeven en of ik een vergunning nodig heb

  Background:
    Given the calculation date is "2024-06-01"
    And parameter "kvk_nummer" is "85234567"
    And the following "DOUANE" data with key "kvk_nummer" for law "wet_op_de_accijns/accijnsplicht_alcohol":
      | kvk_nummer | heeft_agp_vergunning_register |
      | "85234567" | false                         |

  # Artikel 7 lid 1 rondt het volumeprocent naar beneden af op één decimaal, en
  # pas daarna wordt vermenigvuldigd. Met het ruwe percentage zou 5,28% als
  # 4287 rekenen in plaats van 4222: € 0,65 per hectoliter te veel, altijd ten
  # nadele van de belastingplichtige.
  Scenario: Bier met een gebroken alcoholpercentage rondt naar beneden af
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 5.28
    And parameter "hoeveelheid_hectoliter" is 1
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "is_accijnsgoed, accijnscategorie, tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "is_accijnsgoed" is true
    And output "accijnscategorie" equals "bier"
    And output "tarief_per_hectoliter" equals 4222
    And output "verschuldigde_accijns" equals 4222

  Scenario: Bier tegen het gewone tarief van artikel 7 lid 1
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 5
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 4060
    And output "verschuldigde_accijns" equals 40600

  Scenario: Kleine brouwerij betaalt het verlaagde tarief van artikel 7 lid 2
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 5
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | true |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 3755
    And output "verschuldigde_accijns" equals 37550

  # Artikel 7 lid 1: het minimumbedrag van € 26,13 per hectoliter geldt ook als
  # het percentage een lager bedrag zou opleveren.
  Scenario: Zwak bier valt terug op het minimumbedrag
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 0.6
    And parameter "hoeveelheid_hectoliter" is 1
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 2613

  # Het minimum geldt per hectoliter en niet per partij: bij tien hectoliter
  # blijft het tarief € 26,13 per hectoliter en wordt het totaal tien keer zo
  # hoog. Lid 1 zegt "ten minste € 26,13", lid 2 "€ 26,13 per hectoliter"; deze
  # twee scenario's leggen vast dat beide leden per hectoliter rekenen.
  Scenario: Het minimumbedrag geldt per hectoliter, ook bij een grote partij
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 0.6
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 2613
    And output "verschuldigde_accijns" equals 26130

  Scenario: Ook het verlaagde tarief van lid 2 kent hetzelfde minimum per hectoliter
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 0.6
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | true |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 2613
    And output "verschuldigde_accijns" equals 26130

  # Artikel 6: bier is pas bier boven 0,5%vol, dus daaronder is er geen
  # accijnsgoed en dus niets verschuldigd.
  Scenario: Alcoholvrij bier is geen accijnsgoed
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 0.3
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "voldoet_aan_voorwaarden, is_accijnsgoed, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "voldoet_aan_voorwaarden" is false
    And output "is_accijnsgoed" is false
    And output "verschuldigde_accijns" equals 0

  Scenario: Wijn boven 8,5%vol valt onder het hoge tarief van artikel 10
    Given parameter "type_product" is "wijn_niet_mousserend"
    And parameter "alcoholpercentage" is 12
    And parameter "hoeveelheid_hectoliter" is 5
    And parameter "activiteit" is "handel"
    When I evaluate outputs "accijnscategorie, tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "accijnscategorie" equals "wijn"
    And output "tarief_per_hectoliter" equals 9569
    And output "verschuldigde_accijns" equals 47845

  Scenario: Wijn tot en met 8,5%vol valt onder het lage tarief van artikel 10
    Given parameter "type_product" is "wijn_niet_mousserend"
    And parameter "alcoholpercentage" is 8
    And parameter "hoeveelheid_hectoliter" is 5
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 4795

  # Artikel 10 noemt mousserende en niet-mousserende wijn in één adem en geeft
  # ze hetzelfde tarief; het onderscheid is per 2024-01-01 tariefneutraal.
  Scenario: Mousserende wijn betaalt hetzelfde als niet-mousserende
    Given parameter "type_product" is "wijn_mousserend"
    And parameter "alcoholpercentage" is 12
    And parameter "hoeveelheid_hectoliter" is 5
    And parameter "activiteit" is "handel"
    When I evaluate outputs "tarief_per_hectoliter" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "tarief_per_hectoliter" equals 9569

  Scenario: Tussenproduct boven 15%vol valt onder het hoge tarief van artikel 11d
    Given parameter "type_product" is "tussenproduct_niet_mousserend"
    And parameter "alcoholpercentage" is 18
    And parameter "hoeveelheid_hectoliter" is 2
    And parameter "activiteit" is "handel"
    When I evaluate outputs "accijnscategorie, tarief_per_hectoliter, verschuldigde_accijns" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "accijnscategorie" equals "tussenproducten"
    And output "tarief_per_hectoliter" equals 16180
    And output "verschuldigde_accijns" equals 32360

  # Artikel 13 draagt dezelfde afrondingszin als artikel 7 lid 1: 40,37% rekent
  # als 40,3%. Met het ruwe percentage zou hier 73756 uitkomen, € 1,28 te veel.
  Scenario: Gedistilleerd met een gebroken alcoholpercentage rondt naar beneden af
    Given parameter "type_product" is "overige_alcohol"
    And parameter "alcoholpercentage" is 40.37
    And parameter "hoeveelheid_hectoliter" is 1
    And parameter "activiteit" is "handel"
    When I evaluate outputs "accijnscategorie, tarief_per_hectoliter" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "accijnscategorie" equals "overige_alcoholhoudende_producten"
    And output "tarief_per_hectoliter" equals 73628

  # Artikel 5 jo. 39: produceren buiten een accijnsgoederenplaats mag niet, en
  # dit bedrijf heeft geen vergunning.
  Scenario: Produceren zonder AGP-vergunning voldoet niet aan de vergunningsplicht
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 5
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "productie"
    When I evaluate outputs "agp_vergunning_vereist, heeft_agp_vergunning, voldoet_aan_vergunningsplicht" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "agp_vergunning_vereist" is true
    And output "heeft_agp_vergunning" is false
    And output "voldoet_aan_vergunningsplicht" is false

  Scenario: Handelen vraagt geen AGP-vergunning
    Given parameter "type_product" is "bier"
    And parameter "alcoholpercentage" is 5
    And parameter "hoeveelheid_hectoliter" is 10
    And the following parameters:
      | is_kleine_brouwerij | false |
    And parameter "activiteit" is "handel"
    When I evaluate outputs "agp_vergunning_vereist, voldoet_aan_vergunningsplicht" of "wet_op_de_accijns/accijnsplicht_alcohol"
    Then output "agp_vergunning_vereist" is false
    And output "voldoet_aan_vergunningsplicht" is true
