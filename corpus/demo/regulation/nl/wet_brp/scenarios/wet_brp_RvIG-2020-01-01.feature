Feature: Woonsituatie uit de BRP
  Thuiswonend is wie op het adres van een ouder woont: het verblijfsadres is een van de
  ouderadressen. Dezelfde woonplaats is niet genoeg.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bsn" is "999993653"
    And parameter "referentiedatum" is "2025-03-01"

  Scenario: Wie op het adres van een ouder woont, is thuiswonend
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | adres                                                                                                      | ouder_adressen                       |
      | "999993653" | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | ["Kalverstraat 1, 1012NX Amsterdam"] |
    When I evaluate "woonsituatie" of "wet_brp"
    Then output "woonsituatie" equals "THUIS"

  Scenario: Wie in dezelfde plaats als een ouder woont maar op een ander adres, woont uit
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | adres                                                                                                      | ouder_adressen                  |
      | "999993653" | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | ["Damrak 70, 1012LM Amsterdam"] |
    When I evaluate "woonsituatie" of "wet_brp"
    Then output "woonsituatie" equals "UIT"

  Scenario: Wie geen adres heeft, woont niet bij een ouder
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | adres | ouder_adressen                       |
      | "999993653" | null  | ["Kalverstraat 1, 1012NX Amsterdam"] |
    When I evaluate "woonsituatie" of "wet_brp"
    Then output "woonsituatie" equals "UIT"

  Scenario: Wie geen ouders met een adres in de BRP heeft, woont uit
    Given the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | adres                                                                                                      | ouder_adressen |
      | "999993653" | {"straat":"Kalverstraat","huisnummer":"1","postcode":"1012NX","woonplaats":"Amsterdam","type":"WOONADRES"} | []             |
    When I evaluate "woonsituatie" of "wet_brp"
    Then output "woonsituatie" equals "UIT"
