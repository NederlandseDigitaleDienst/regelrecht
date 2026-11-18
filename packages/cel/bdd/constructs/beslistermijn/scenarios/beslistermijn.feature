Feature: The decision term of Awb 4:13 on an application for a permit
  As someone who models the general administrative law
  I want the decision term to follow every application
  So that a decision given after the last day of the term shows as late

  # The construct "beslistermijn". Everything it loads is in ../regulation and
  # fictional: test_general_administrative_act, after articles 4:2 and 4:13
  # of the Algemene wet bestuursrecht, and three permit acts. Article 4:13
  # hooks onto every application (AANVRAAG): it takes the day of receipt as
  # the moment that counts. It gives the last day of the term set by
  # statutory provision (lid 1) as `decision_deadline`, eight weeks after
  # receipt when the permit act sets none, and the eight weeks of lid 2 as
  # `reasonable_decision_term_end`.
  #
  # The cell records an application on the calculation date, and that day is
  # the receipt. The decision is recorded on the calculation date of its own
  # step. Article 1 of a permit act both establishes the application and
  # decides on it, so the decision runs the hooks on the application again
  # and gives the last day with it.
  #
  # Like the Awb in the corpus, 4:13 here leaves out the Algemene
  # termijnenwet, which moves a last day on a weekend or public holiday to
  # the next working day. The dates here keep every last day on a working
  # day, so the scenarios would hold under the Algemene termijnenwet too.

  Scenario: The application holds the fields of the permit act and its date, and takes the receipt as its moment
    # The hooks on the application decide what it holds. Article 4:2 adds
    # the date of the application. Article 4:13 adds no field: its only
    # parameter, date_of_receipt, is the moment the cell fills. No field of
    # the decision (such as the day of the decision or the last day of the
    # term) appears on the application.
    Given the calculation date is "2026-03-03"
    Then the submission under article "1" of "test_permit_act" holds exactly the fields:
      | field            |
      | meets_conditions |
      | application_date |
    And the submission under article "1" of "test_permit_act" takes its moment as "date_of_receipt"

  Scenario: A decision well within eight weeks is on time
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-20"
    When the cell decides "permit_decision" on submission "permit_application"
    Then the execution succeeds
    And output "permit_granted" is true
    # test_permit_act_implements is loaded too and fills the term of 4:13
    # with one week, but names its own application as the host (RFC-058).
    And output "decision_deadline" equals "2026-04-28"
    And output "decision_deadline" has reactive provenance
    And output "reasonable_decision_term_end" equals "2026-04-28"
    And decision "permit_decision" is given on or before "decision_deadline"

  Scenario: A decision on the last day of the term is on time
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true |
    Given the calculation date is "2026-04-28"
    When the cell decides "permit_decision" on submission "permit_application"
    Then output "decision_deadline" equals "2026-04-28"
    And decision "permit_decision" is given on or before "decision_deadline"

  Scenario: A decision one day after the last day of the term is late
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true |
    Given the calculation date is "2026-04-29"
    When the cell decides "permit_decision" on submission "permit_application"
    Then decision "permit_decision" is given after "decision_deadline"
    # Late is not refused: the term has passed, the decision still stands.
    And output "permit_granted" is true

  Scenario: The term runs from the day of receipt, not from the date on the application
    # The application is dated 20 February and arrives on 3 March. Eight
    # weeks from its date would end on 17 April; the term ends on 28 April.
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act" with:
      | meets_conditions | true       |
      | application_date | 2026-02-20 |
    Given the calculation date is "2026-04-20"
    When the cell decides "permit_decision" on submission "permit_application"
    Then output "decision_deadline" equals "2026-04-28"
    And decision "permit_decision" is given on or before "decision_deadline"

  Scenario: A permit act with a term of its own sets two weeks with an override
    # Lid 1: a term set by statutory provision goes first. This permit act
    # sets the last day, `decision_deadline`, with an override on that
    # output of 4:13 (article 2 of test_permit_act_short_term). The last
    # scenario sets it through implements instead (RFC-058). The fallback of
    # lid 2 is not replaced and stays eight weeks.
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act_short_term" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-17"
    When the cell decides "permit_decision" on submission "permit_application"
    Then output "decision_deadline" equals "2026-03-17"
    And output "decision_deadline" has override provenance
    And output "reasonable_decision_term_end" equals "2026-04-28"
    And decision "permit_decision" is given on or before "decision_deadline"

  Scenario: Under a term of two weeks, three weeks is late
    # Within the eight weeks of lid 2, and still late: the term of the
    # permit act goes first.
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act_short_term" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-24"
    When the cell decides "permit_decision" on submission "permit_application"
    Then decision "permit_decision" is given after "decision_deadline"

  # Lid 1 as an open term: the permit act fills `statutory_decision_term`
  # with implements (article 2 of test_permit_act_implements). The filling
  # names the application of article 1 as its host, so it applies under the
  # hook on that application only, with the receipt from its scope (RFC-058). The last day comes out as `decision_deadline`;
  # `reasonable_decision_term_end` keeps the eight weeks of lid 2.
  Scenario: A permit act fills the statutory term of Awb 4:13 through implements
    Given the calculation date is "2026-03-03"
    When the cell records submission "permit_application" under article "1" of "test_permit_act_implements" with:
      | meets_conditions | true |
    Given the calculation date is "2026-03-11"
    When the cell decides "permit_decision" on submission "permit_application"
    Then output "decision_deadline" equals "2026-03-10"
    And output "reasonable_decision_term_end" equals "2026-04-28"
    And decision "permit_decision" is given after "decision_deadline"
