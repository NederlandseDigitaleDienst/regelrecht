Feature: Awir artikel 7, tweede lid, haakt in op een kenmerk van het lid
  "Indien in een inkomensafhankelijke regeling is bepaald dat naast de
  draagkracht van de belanghebbende en diens partner ook de draagkracht van
  medebewoners van belang is [...], wordt mede het toetsingsinkomen van de
  medebewoners in aanmerking genomen." De Wet op de huurtoeslag bepaalt dat
  in artikel 7, eerste lid, en claimt daarmee de smallere soort die Awir
  artikel 7 vestigt. Het kindgebonden budget bepaalt het niet.

  Background:
    Given the calculation date is "2026-01-01"
    Given law "algemene_wet_inkomensafhankelijke_regelingen" is loaded

  Scenario: Bij de huurtoeslag telt het toetsingsinkomen van de medebewoners mee
    When I evaluate "recht_afhankelijk_van_draagkracht" of "wet_op_de_huurtoeslag"
    Then the execution succeeds
    Then output "toetsingsinkomen_medebewoners_in_aanmerking" is true
    Then output "toetsingsinkomen_medebewoners_in_aanmerking" has reactive provenance

  Scenario: Bij het kindgebonden budget vuurt Awir 7, tweede lid, niet
    Given the following parameters:
      | kinderbijslag_betaald_of_zou_worden_betaald | true |
    When I evaluate "heeft_aanspraak_op_kindgebonden_budget" of "wet_op_het_kindgebonden_budget"
    Then the execution succeeds
    Then the result contains exactly the outputs "heeft_aanspraak_op_kindgebonden_budget, wijziging_in_aanmerking_vanaf"
