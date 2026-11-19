@tier:core
Feature: Typing of a parameter value — bdd/grammar.yaml value_typing
  The language writes the same value three ways: `is "42"`, `is 42`, and a
  table cell. All three name the same value — the content decides what it is,
  and the quotes are only there because the language has no bare form for a
  boolean or a date. An engine that reads the quoted form as text hands the
  law a String where it compares an Int, and since EQUALS is structural that
  comparison goes quietly false instead of failing.

  EQUALS is exactly what reports which type actually reached the engine.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: A quoted number is a number
    Given parameter "waarde" is "42"
    When I evaluate outputs "waarde_is_getal, waarde_is_tekst" of "test_value_typing"
    Then the execution succeeds
    Then output "waarde_is_getal" is true
    Then output "waarde_is_tekst" is false

  Scenario: A bare number is the same number
    Given parameter "waarde" is 42
    When I evaluate outputs "waarde_is_getal, waarde_is_tekst" of "test_value_typing"
    Then the execution succeeds
    Then output "waarde_is_getal" is true
    Then output "waarde_is_tekst" is false

  Scenario: A table cell reads the same way
    Given the following parameters:
      | waarde | 42 |
    When I evaluate outputs "waarde_is_getal, waarde_is_tekst" of "test_value_typing"
    Then the execution succeeds
    Then output "waarde_is_getal" is true
    Then output "waarde_is_tekst" is false

  Scenario: A table cell in double quotes is text, whatever it looks like
    Given the following parameters:
      | waarde | "42" |
    When I evaluate outputs "waarde_is_getal, waarde_is_tekst" of "test_value_typing"
    Then the execution succeeds
    Then output "waarde_is_getal" is false
    Then output "waarde_is_tekst" is true

  Scenario: A value that is not a number literal stays text
    Given parameter "waarde" is "GM0384"
    When I evaluate outputs "waarde_is_getal, waarde_is_tekst" of "test_value_typing"
    Then the execution succeeds
    Then output "waarde_is_getal" is false
    Then output "waarde_is_tekst" is false
