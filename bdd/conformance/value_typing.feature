@tier:core
Feature: Typing of a parameter value — bdd/grammar.yaml value_typing
  The language writes a value three ways: `is "42"`, `is 42`, and a table
  cell. Digits in double quotes are text and bare digits are a number, in a
  step and in a cell alike. What has no bare form keeps its meaning inside
  quotes: a boolean, an absence, a list.

  The law declares what each parameter holds, and a value of another type is
  refused where it enters (RFC-055). That refusal is what reports which type
  reached the engine: a runner that reads `"42"` as a number fails the first
  scenario and passes the one that expects a refusal.

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Digits in quotes are text
    Given parameter "tekst" is "42"
    When I evaluate outputs "tekst_is_42" of "test_value_typing"
    Then the execution succeeds
    Then output "tekst_is_42" is true

  Scenario: Bare digits are a number
    Given parameter "getal" is 42
    When I evaluate outputs "getal_is_42" of "test_value_typing"
    Then the execution succeeds
    Then output "getal_is_42" is true

  Scenario: Digits in quotes do not pass for a number
    Given parameter "getal" is "42"
    When I evaluate outputs "getal_is_42" of "test_value_typing"
    Then the execution fails with "expected number for parameter 'getal' of test_value_typing, got string"

  Scenario: Bare digits do not pass for text
    Given parameter "tekst" is 42
    When I evaluate outputs "tekst_is_42" of "test_value_typing"
    Then the execution fails with "expected string for parameter 'tekst' of test_value_typing, got number"

  Scenario: A table cell reads the same way
    Given the following parameters:
      | tekst | "42" |
      | getal | 42   |
    When I evaluate outputs "tekst_is_42, getal_is_42" of "test_value_typing"
    Then the execution succeeds
    Then output "tekst_is_42" is true
    Then output "getal_is_42" is true

  Scenario: A quoted cell with digits does not pass for a number
    Given the following parameters:
      | getal | "42" |
    When I evaluate outputs "getal_is_42" of "test_value_typing"
    Then the execution fails with "expected number for parameter 'getal' of test_value_typing, got string"

  Scenario: A boolean keeps its meaning inside quotes
    Given parameter "vlag" is "true"
    When I evaluate outputs "vlag_staat" of "test_value_typing"
    Then the execution succeeds
    Then output "vlag_staat" is true

  Scenario: A list keeps its meaning inside quotes
    Given parameter "lijst" is "[1, 2, 3]"
    When I evaluate outputs "twee_in_lijst" of "test_value_typing"
    Then the execution succeeds
    Then output "twee_in_lijst" is true

  Scenario: Text that is no number literal stays text
    Given parameter "tekst" is "GM0384"
    When I evaluate outputs "tekst_is_42" of "test_value_typing"
    Then the execution succeeds
    Then output "tekst_is_42" is false
