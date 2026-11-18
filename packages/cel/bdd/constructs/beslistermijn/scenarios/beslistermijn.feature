Feature: The decision term of Awb 4:13 on an application for a permit
  As someone who models the general administrative law
  I want the decision term of the Awb to follow every application
  So that a decision given after the last day of the term shows as late

  # The construct "beslistermijn". The Algemene wet bestuursrecht comes from
  # the corpus (../corpus.yaml names it), the permit acts in ../regulation are
  # fictional, and nothing else is loaded. Article 4:13 hooks onto every
  # application (AANVRAAG): it takes the day of receipt as the moment that
  # counts. It gives the last day of the term set by statutory provision
  # (lid 1) as `uiterste_beslisdatum`, eight weeks after receipt when the
  # host act sets none, and the eight weeks of lid 2 as
  # `redelijke_beslistermijn_einddatum`.
  #
  # The cell records an application on the calculation date, and that day is
  # the receipt. The decision is recorded on the calculation date of its own
  # step. Article 1 of a permit act both establishes the application and
  # decides on it, so the decision runs the hooks on the application again
  # and gives the last day with it.
  #
  # Awb 4:13 in the corpus leaves out the Algemene termijnenwet, which moves
  # a last day on a weekend or public holiday to the next working day. The
  # dates here keep every last day on a working day, so the scenarios hold in
  # law as well as in the model.
  #
  # Names that come from the corpus stay Dutch: `datum_ontvangst`,
  # `dagtekening` and the other fields of Awb 4:2, and the outputs
  # `uiterste_beslisdatum` and `redelijke_beslistermijn_einddatum` of Awb
  # 4:13.

  Scenario: The application holds the fields of the permit act and of Awb 4:2, and takes the receipt as its moment
    # The hooks on the application decide what it holds. Awb 4:13 adds no
    # field: its only parameter, datum_ontvangst, is the moment the cell
    # fills. No field of the decision (such as the day of the decision or
    # the last day of the term) appears on the application.
    Given the calculation date is "2026-03-03"
    Then the submission under article "1" of "test_permit_act" holds exactly the fields:
      | field                 |
      | meets_conditions      |
      | aanvraag_ondertekend  |
      | naam_aanvrager        |
      | adres_aanvrager       |
      | dagtekening           |
      | gevraagde_beschikking |
    And the submission under article "1" of "test_permit_act" takes its moment as "datum_ontvangst"

  Scenario: A decision well within eight weeks is on time
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-20"
    When the cell decides "permit_decision" on submission "permit_application"
    Then the execution succeeds
    And output "permit_granted" is true
    # From the corpus: the output of Awb 4:13.
    And output "redelijke_beslistermijn_einddatum" equals "2026-04-28"
    And output "redelijke_beslistermijn_einddatum" has reactive provenance
    And decision "permit_decision" is given on or before "redelijke_beslistermijn_einddatum"
    # test_permit_act_implements is loaded too and fills the term of 4:13
    # with one week, but only for its own application (RFC-058).
    And output "uiterste_beslisdatum" equals "2026-04-28"

  Scenario: A decision on the last day of the term is on time
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true |
    Given the calculation date is "2026-04-28"
    When the cell decides "permit_decision" on submission "permit_application"
    # From the corpus: the output of Awb 4:13.
    Then output "redelijke_beslistermijn_einddatum" equals "2026-04-28"
    And decision "permit_decision" is given on or before "redelijke_beslistermijn_einddatum"

  Scenario: A decision one day after the last day of the term is late
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true |
    Given the calculation date is "2026-04-29"
    When the cell decides "permit_decision" on submission "permit_application"
    # From the corpus: the output of Awb 4:13.
    Then decision "permit_decision" is given after "redelijke_beslistermijn_einddatum"
    # Late is not refused: the term has passed, the decision still stands.
    And output "permit_granted" is true

  Scenario: The term runs from the day of receipt, not from the date on the application
    # The application is dated 20 February and arrives on 3 March. Eight
    # weeks from its date would end on 17 April; the term ends on 28 April.
    Given the calculation date is "2026-03-03"
    # dagtekening: the date of the application, a field of Awb 4:2 in the corpus.
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true       |
      | dagtekening      | 2026-02-20 |
    Given the calculation date is "2026-04-20"
    When the cell decides "permit_decision" on submission "permit_application"
    # From the corpus: the output of Awb 4:13.
    Then output "redelijke_beslistermijn_einddatum" equals "2026-04-28"
    And decision "permit_decision" is given on or before "redelijke_beslistermijn_einddatum"

  Scenario: A permit act with a term of its own sets two weeks with an override
    # Awb 4:13 lid 1: a term set by statutory provision goes first. This
    # host act sets the last day, `uiterste_beslisdatum`, with an override on
    # that output of 4:13 (article 2 of test_permit_act_short_term). The
    # last scenario sets `uiterste_beslisdatum` through implements instead
    # (RFC-058). The fallback of lid 2 is not replaced and stays eight weeks.
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act_short_term" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-17"
    When the cell decides "permit_decision" on submission "permit_application"
    # From the corpus: the outputs of Awb 4:13, the first replaced by the host act.
    Then output "uiterste_beslisdatum" equals "2026-03-17"
    And output "uiterste_beslisdatum" has override provenance
    And output "redelijke_beslistermijn_einddatum" equals "2026-04-28"
    And decision "permit_decision" is given on or before "uiterste_beslisdatum"

  Scenario: Under a term of two weeks, three weeks is late
    # Within the eight weeks of lid 2, and still late: the term of the host
    # act goes first.
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act_short_term" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-24"
    When the cell decides "permit_decision" on submission "permit_application"
    # From the corpus: the output of Awb 4:13, replaced by the host act.
    Then decision "permit_decision" is given after "uiterste_beslisdatum"

  # Awb 4:13 lid 1 as an open term: the host act fills
  # `wettelijke_beslistermijn` with implements (article 2 of
  # test_permit_act_implements), and under the hook on its application only
  # that act fills it, with the receipt from its own scope (RFC-058). The last
  # day comes out as `uiterste_beslisdatum`; `redelijke_beslistermijn_einddatum`
  # keeps the eight weeks of lid 2.
  Scenario: A permit act fills the statutory term of Awb 4:13 through implements
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act_implements" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-11"
    When the cell decides "permit_decision" on submission "permit_application"
    # From the corpus: the outputs of Awb 4:13.
    Then output "uiterste_beslisdatum" equals "2026-03-10"
    And output "redelijke_beslistermijn_einddatum" equals "2026-04-28"
    And decision "permit_decision" is given after "uiterste_beslisdatum"
