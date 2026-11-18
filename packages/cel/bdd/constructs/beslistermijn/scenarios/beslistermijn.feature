Feature: The decision term as a hook on the decision of a host law
  As someone who models the general administrative law
  I want a decision term that fires on every decision of a host law
  So that a decision taken after the last day of the term is late

  # The construct "beslistermijn". The regelwerken in ../regulation are
  # fictional and are the only ones these scenarios load; the corpus is not
  # loaded. Article 1 and 2 of test_algemene_bestuursregels follow article
  # 4:13 of the Awb: the term a statutory provision sets (lid 1), else eight
  # weeks after the application was received (lid 2). Both fire as a hook on
  # every BESCHIKKING. test_wet_tafeldekken sets no term of its own;
  # test_wet_feestmaaltijd sets two weeks with an override.
  #
  # The application is recorded on the day it was received, and the decision
  # on the calculation date of the step that takes it: that day is the
  # peildatum of the hook, and so the day the decision is given.

  Scenario: A decision well within eight weeks is on time
    Given the calculation date is "2026-03-02"
    When the cell records submission "aanvraag" under article "1" of "test_wet_tafeldekken" with:
      | aantal_gasten   | 8          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-03-20"
    When the cell decides "besluit" on submission "aanvraag"
    Then the execution succeeds
    And output "tafel_wordt_gedekt" is true
    And output "beslistermijn_weken" equals 8
    And output "uiterste_beslisdatum" equals "2026-04-27"
    And output "uiterste_beslisdatum" has reactive provenance
    And output "besluit_te_laat" is false

  Scenario: A decision on the last day of the term is on time
    Given the calculation date is "2026-03-02"
    When the cell records submission "aanvraag" under article "1" of "test_wet_tafeldekken" with:
      | aantal_gasten   | 8          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-04-27"
    When the cell decides "besluit" on submission "aanvraag"
    Then output "uiterste_beslisdatum" equals "2026-04-27"
    And output "besluit_te_laat" is false

  Scenario: A decision one day after the last day of the term is late
    Given the calculation date is "2026-03-02"
    When the cell records submission "aanvraag" under article "1" of "test_wet_tafeldekken" with:
      | aantal_gasten   | 8          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-04-28"
    When the cell decides "besluit" on submission "aanvraag"
    Then output "besluit_te_laat" is true
    # Late is not refused: the hook says the term passed, the decision of
    # the host still stands.
    And output "tafel_wordt_gedekt" is true

  Scenario: The term runs from the day of receipt, not from the day the cell records it
    # The application arrived on 2 March and was recorded a week later. Eight
    # weeks after recording would end on 4 May; the term ends on 27 April.
    Given the calculation date is "2026-03-09"
    When the cell records submission "aanvraag" under article "1" of "test_wet_tafeldekken" with:
      | aantal_gasten   | 8          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-04-30"
    When the cell decides "besluit" on submission "aanvraag"
    Then output "uiterste_beslisdatum" equals "2026-04-27"
    And output "besluit_te_laat" is true

  Scenario: A host law with a term of its own sets two weeks
    Given the calculation date is "2026-03-02"
    When the cell records submission "aanvraag" under article "1" of "test_wet_feestmaaltijd" with:
      | aantal_gangen   | 4          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-03-16"
    When the cell decides "besluit" on submission "aanvraag"
    Then output "feestmaaltijd_wordt_bereid" is true
    And output "beslistermijn_weken" equals 2
    And output "beslistermijn_weken" has override provenance
    And output "uiterste_beslisdatum" equals "2026-03-16"
    And output "besluit_te_laat" is false

  Scenario: Under a host law with a term of two weeks, three weeks is late
    # Within the eight weeks of lid 2, and still late: the term of the host
    # law goes first.
    Given the calculation date is "2026-03-02"
    When the cell records submission "aanvraag" under article "1" of "test_wet_feestmaaltijd" with:
      | aantal_gangen   | 4          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-03-23"
    When the cell decides "besluit" on submission "aanvraag"
    Then output "besluit_te_laat" is true

  # @wip: the engine does not fill the open term of a hook article from the
  # host law yet. An implementation today gets only the parameters of the
  # hook article and fills the term for every host law at once. Branch
  # feat-hook-host-implements builds it; that pull request adds the host law
  # test_wet_ontbijt (its article 3 implements wettelijke_beslistermijn_weken
  # of test_algemene_bestuursregels article 1 with one week) and removes this
  # tag.
  @wip
  Scenario: A host law fills the statutory term through implements
    Given the calculation date is "2026-03-02"
    When the cell records submission "aanvraag" under article "1" of "test_wet_ontbijt" with:
      | aantal_broodjes | 6          |
      | datum_ontvangst | 2026-03-02 |
    Given the calculation date is "2026-03-10"
    When the cell decides "besluit" on submission "aanvraag"
    Then output "beslistermijn_weken" equals 1
    And output "uiterste_beslisdatum" equals "2026-03-09"
    And output "besluit_te_laat" is true
