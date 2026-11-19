Feature: AOW Leeftijdsbepaling - Pensioengerechtigde leeftijd (art. 7a)
  Als burger
  Wil ik weten wat mijn pensioengerechtigde leeftijd is
  Zodat ik weet vanaf welke leeftijd ik AOW kan ontvangen

  Background:
    Given the calculation date is "2026-01-01"

  Scenario: Geboren in 1960 valt onder lid 1 onder n (67 jaar), niet onder de formule van lid 2
    # Audit AUDIT_GETROUWHEID.md: de IF-cascade miste deze case, waardoor
    # 1960-geborenen ten onrechte in de formule-tak van lid 2 vielen.
    Given parameter "geboortedatum" is "1960-05-01"
    And parameter "bsn" is "999400001"
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999400001" | 20.5           |
    When I evaluate outputs "pensioenleeftijd" of "algemene_ouderdomswet/leeftijdsbepaling"
    Then output "pensioenleeftijd" equals 67

  Scenario: Formule lid 2 geeft geen verhoging als V kleiner is dan 0,25
    # V = 2/3 * (L - 20,64) - (P - 67); met L = 21,00 en P = 67 is V = 0,24 (< 0,25),
    # dus geen verhoging (art. 7a lid 2, derde alinea).
    Given parameter "geboortedatum" is "1965-06-15"
    And parameter "bsn" is "999400002"
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999400002" | 21.00          |
    When I evaluate outputs "pensioenleeftijd" of "algemene_ouderdomswet/leeftijdsbepaling"
    Then output "pensioenleeftijd" equals 67

  Scenario: Formule lid 2 geeft een verhoging van drie maanden als V minstens 0,25 is
    # V = 2/3 * (L - 20,64) - (P - 67); met L = 21,02 en P = 67 is V ≈ 0,253 (>= 0,25),
    # dus een verhoging van precies drie maanden (art. 7a lid 2, derde alinea) - geen
    # proportionele verhoging.
    Given parameter "geboortedatum" is "1965-06-15"
    And parameter "bsn" is "999400003"
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999400003" | 21.02          |
    When I evaluate outputs "pensioenleeftijd" of "algemene_ouderdomswet/leeftijdsbepaling"
    Then output "pensioenleeftijd" equals 67.25
