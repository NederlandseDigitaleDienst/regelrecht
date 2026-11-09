@tier:core
Feature: Collection operations
  As an author of machine-readable law
  I want to iterate over a group whose size is not known in advance
  So that a legal threshold stays in the law instead of in the data source

  # Exercises RFC-016: FOREACH with filter, combine, dot notation and nesting.
  # Driven against the test law corpus/regulation/nl/wet/test_collections.
  #
  # Every outcome is reduced to a scalar. That is deliberate: the assertion
  # vocabulary has no step for an array, and adding one would oblige every
  # engine that claims the core tier. Reducing also makes each assertion
  # diagnostic about one rule rather than about the array as a whole.

  Scenario: A threshold in the law decides who counts, and what they contribute
    Given the calculation date is "2025-01-01"
    Given parameter "medebewoners" is the collection:
      | leeftijd | bijdrage |
      | 25       | 100      |
      | 19       | 50       |
      | 21       | 200      |
    # Two of the three are 21 or older. The register supplies the ages; the law
    # decides the limit.
    When I evaluate "aantal_medebewoners_21_plus" of "test_collections"
    Then output "aantal_medebewoners_21_plus" equals 2
    When I evaluate "totaal_bijdragen" of "test_collections"
    Then output "totaal_bijdragen" equals 300
    # Without the filter the 19-year-old counts too, so the filter is doing work.
    When I evaluate "totaal_alle_bijdragen" of "test_collections"
    Then output "totaal_alle_bijdragen" equals 350

  Scenario: MIN and MAX reduce the collection to one element's value
    Given the calculation date is "2025-01-01"
    Given parameter "medebewoners" is the collection:
      | leeftijd | bijdrage |
      | 30       | 100      |
      | 40       | 250      |
      | 22       | 75       |
    When I evaluate "hoogste_bijdrage" of "test_collections"
    Then output "hoogste_bijdrage" equals 250
    When I evaluate "laagste_bijdrage" of "test_collections"
    Then output "laagste_bijdrage" equals 75

  Scenario: AND holds only when every element qualifies, OR when any does
    Given the calculation date is "2025-01-01"
    Given parameter "medebewoners" is the collection:
      | leeftijd | bijdrage |
      | 30       | 100      |
      | 40       | 0        |
    When I evaluate "iedereen_draagt_bij" of "test_collections"
    Then output "iedereen_draagt_bij" is false
    When I evaluate "iemand_draagt_bij" of "test_collections"
    Then output "iemand_draagt_bij" is true

  Scenario: A single element is enough for AND and OR to agree
    Given the calculation date is "2025-01-01"
    Given parameter "medebewoners" is the collection:
      | leeftijd | bijdrage |
      | 30       | 100      |
    When I evaluate "iedereen_draagt_bij" of "test_collections"
    Then output "iedereen_draagt_bij" is true
    When I evaluate "iemand_draagt_bij" of "test_collections"
    Then output "iemand_draagt_bij" is true

  Scenario: An empty collection returns each combine's identity
    Given the calculation date is "2025-01-01"
    Given parameter "medebewoners" is the collection:
      | leeftijd | bijdrage |
    # Nothing to count and nothing to add.
    When I evaluate "aantal_medebewoners_21_plus" of "test_collections"
    Then output "aantal_medebewoners_21_plus" equals 0
    When I evaluate "totaal_bijdragen" of "test_collections"
    Then output "totaal_bijdragen" equals 0
    # There is no highest or lowest value of nothing, so the caller has to
    # handle it rather than receive a number that was never computed.
    When I evaluate "hoogste_bijdrage" of "test_collections"
    Then output "hoogste_bijdrage" is absent
    When I evaluate "laagste_bijdrage" of "test_collections"
    Then output "laagste_bijdrage" is absent
    # Vacuous truth: every element of nothing qualifies, and none does.
    When I evaluate "iedereen_draagt_bij" of "test_collections"
    Then output "iedereen_draagt_bij" is true
    When I evaluate "iemand_draagt_bij" of "test_collections"
    Then output "iemand_draagt_bij" is false

  Scenario: A filter that rejects every element behaves like an empty collection
    Given the calculation date is "2025-01-01"
    Given parameter "medebewoners" is the collection:
      | leeftijd | bijdrage |
      | 18       | 100      |
      | 20       | 250      |
    When I evaluate "aantal_medebewoners_21_plus" of "test_collections"
    Then output "aantal_medebewoners_21_plus" equals 0
    When I evaluate "totaal_bijdragen" of "test_collections"
    Then output "totaal_bijdragen" equals 0

  Scenario: A nested FOREACH iterates both levels
    Given the calculation date is "2025-01-01"
    # The households are a definition in the law: a Gherkin table is flat and
    # cannot carry a nested collection. Three households, of 100+200, 50, and
    # none.
    #
    # The inner collection is read through the outer binding.
    When I evaluate "totaal_over_huishoudens" of "test_collections"
    Then output "totaal_over_huishoudens" equals 350
    # The outer level alone counts three, so the 350 above really did descend
    # a second level rather than summing one.
    When I evaluate "aantal_huishoudens" of "test_collections"
    Then output "aantal_huishoudens" equals 3
    # An inner MAX under an outer ADD. The third household is empty, so its MAX
    # is null: there is no highest value of nothing, and that is an absence,
    # not an unknown. Summing an absence is an error the author has to resolve
    # in the law (RFC-036): leaving the household out would report a confident
    # 250 for a collection one member of which was never counted, and counting
    # "geen" as an amount would decide what the text never said.
    When I evaluate "hoogste_per_huishouden_totaal" of "test_collections"
    Then the execution fails
    Then the execution fails with "operand is null"

  Scenario: An inner FOREACH compares its element with the outer one
    Given the calculation date is "2025-01-01"
    # The inner filter and body see the outer binding (a note of November
    # 2026 on RFC-016). Of the first household, with threshold 150, only 200
    # counts; of the second, with threshold 10, both 50 and 20; the third is
    # empty.
    When I evaluate "leden_boven_drempel" of "test_collections"
    Then output "leden_boven_drempel" equals 3
    # An inner binding with the name of the outer one wins inside the inner
    # FOREACH: the highest contribution of the members, not a threshold.
    When I evaluate "hoogste_eigen_bijdrage" of "test_collections"
    Then output "hoogste_eigen_bijdrage" equals 200
