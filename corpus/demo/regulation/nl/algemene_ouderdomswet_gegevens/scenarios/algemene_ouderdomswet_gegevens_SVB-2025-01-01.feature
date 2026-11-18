Feature: AOW Gegevens - Pensioengerechtigde leeftijd
  Als burger
  Wil ik weten wat mijn pensioengerechtigde leeftijd is volgens art. 7a lid 1 AOW
  Zodat ik weet vanaf welke leeftijd ik AOW kan ontvangen

  Scenario: Pensioengerechtigde leeftijd in 2020 volgt uit de tabel van lid 1 onder h/i
    Given the calculation date is "2020-06-01"
    And parameter "bsn" is "999100010"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100010" | {"pensioenleeftijd":66} |
    When I evaluate outputs "pensioenleeftijd" of "algemene_ouderdomswet_gegevens"
    Then output "pensioenleeftijd" equals 66

  Scenario: Pensioengerechtigde leeftijd in 2024 volgt uit de tabel van lid 1 onder m
    Given the calculation date is "2024-06-01"
    And parameter "bsn" is "999100011"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100011" | {"pensioenleeftijd":67} |
    When I evaluate outputs "pensioenleeftijd" of "algemene_ouderdomswet_gegevens"
    Then output "pensioenleeftijd" equals 67

  Scenario: Pensioengerechtigde leeftijd in 2025 volgt uit het register (lid 1 onder n, untranslatable)
    Given the calculation date is "2025-06-01"
    And parameter "bsn" is "999100012"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet_gegevens":
      | bsn         | pensioengegevens        |
      | "999100012" | {"pensioenleeftijd":67} |
    When I evaluate outputs "pensioenleeftijd" of "algemene_ouderdomswet_gegevens"
    Then output "pensioenleeftijd" equals 67
