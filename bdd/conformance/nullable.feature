@tier:core
Feature: Absence is declared — RFC-036, nullable
  Whether "geen" is a value a field can take is a property of the field's
  type, not of the data that happens to arrive: a parameter, input or output
  declares it with `nullable: true`, and the default is that it never is. The
  engine holds every party to that declaration at the boundary where the value
  arrives. A register that delivers null for a field the law declares never
  absent has contradicted the law, and that is a data error reported there and
  then, not an `operand is null` three operations later. An output that is
  not nullable and still comes out null is the law saying nothing where it
  promised an answer. Unknown (a fact nobody has) is not governed by the flag
  and propagates as before.

  Driven against test_nullable (article 1 reads a nullable and a non-nullable
  rent, article 2 passes a nullable value to a nullable output and takes the
  highest of a list, articles 3 and 3a fetch the partner's birth year,
  article 4 has a nullable parameter, article 5 passes a nullable amount on
  as an optional parameter, article 6 passes an absent partner to a parameter
  the bron declares nullable) and test_nullable_bron (the law that is called).

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given the following "register" data with key "bsn" for law "test_nullable_bron":
      | bsn         | geboortejaar |
      | "999993653" | 1975         |
      | "999993641" | 1980         |

  Scenario: A null cell for a nullable input is a value the law tests
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | huur | huurgrens |
      | "999993653" | null | 700       |
    When I evaluate outputs "heeft_huur, huurgrens_plus" of "test_nullable"
    Then the execution succeeds
    Then output "heeft_huur" is false
    Then output "huurgrens_plus" equals 800

  Scenario: A null cell for an input the law declares never absent is a data error at the boundary
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | huur | huurgrens |
      | "999993653" | 650  | null      |
    # The register says there is no rent limit; the law said there always is.
    # The contradiction names the source, the field and the law, before any
    # action has run.
    When I evaluate "huurgrens_plus" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "huurgrens"
    Then the execution fails with "source register"

  Scenario: A null parameter for an input the law declares never absent is refused at the boundary
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | huur | huurgrens |
      | "999993653" | 650  | 700       |
    # The caller hands the rent limit in directly, under the input's name,
    # and says there is none. The register is bypassed; the declaration is
    # not, and the caller is named as the origin.
    Given parameter "huurgrens" is "null"
    When I evaluate "huurgrens_plus" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "huurgrens"
    Then the execution fails with "parameter from caller"

  Scenario: An empty cell is unknown whatever the declaration says
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | huur | huurgrens |
      | "999993653" |      |           |
    When I evaluate outputs "heeft_huur, huurgrens_plus" of "test_nullable"
    Then the execution succeeds
    Then output "heeft_huur" is unknown for lack of "huur"
    Then output "huurgrens_plus" is unknown for lack of "huurgrens"

  Scenario: A nullable output may be absent
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | toeslag | toeslagen |
      | "999993653" | 700     | [700]     |
    # No case of the IF matches 700 and there is no default: the class is
    # absent, and the output declared that it may be. The highest of a list
    # with something in it is a value.
    When I evaluate outputs "toeslagklasse, hoogste_toeslag" of "test_nullable"
    Then the execution succeeds
    Then output "toeslagklasse" is absent
    Then output "hoogste_toeslag" equals 700

  Scenario: An output the law declares never absent may not evaluate to null
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | toeslagen |
      | "999993653" | []        |
    # The highest of an empty list is absent, which no static check can
    # foresee. The output promised a value, and breaks that promise.
    When I evaluate "hoogste_toeslag" of "test_nullable"
    Then the execution fails
    Then the execution fails with "evaluated to null"
    Then the execution fails with "not declared nullable"
    Then the execution fails with "hoogste_toeslag"

  Scenario: A skipped call for nobody yields an absence, which a nullable input takes
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | partner_bsn |
      | "999993653" | null        |
    # There is no partner, so the bron is not called and the input is null.
    # Article 3 declared that. A non-nullable input in its place is refused
    # by the static check, so no law reaches this point undeclared.
    When I evaluate "partner_geboortejaar" of "test_nullable"
    Then the execution succeeds
    Then output "partner_geboortejaar" is absent

  Scenario: A null key cell for an input declared never absent is refused at the register
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | partner_bsn |
      | "999993653" | null        |
    # Article 3a declares the partner's number never absent. The register's
    # null contradicts that where the key is read, before the bron is asked
    # about anybody.
    When I evaluate "partner_geboortejaar_strikt" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "partner_bsn"
    Then the execution fails with "source register"

  Scenario: A required parameter the other law declares nullable is passed as null, not skipped
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | partner_bsn |
      | "999993653" | null        |
    # The bron said it can decide on nobody, so the absent partner is handed
    # to it and it decides: there is no one.
    When I evaluate "partner_is_iemand" of "test_nullable"
    Then the execution succeeds
    Then output "partner_is_iemand" is false

  Scenario: A required nullable parameter with a value is run as before
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | partner_bsn |
      | "999993653" | "999993641" |
    When I evaluate "partner_is_iemand" of "test_nullable"
    Then the execution succeeds
    Then output "partner_is_iemand" is true

  Scenario: A partner who is there is looked up as before
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | partner_bsn |
      | "999993653" | "999993641" |
    When I evaluate outputs "partner_geboortejaar" of "test_nullable"
    Then the execution succeeds
    Then output "partner_geboortejaar" equals 1980
    When I evaluate outputs "partner_geboortejaar_strikt" of "test_nullable"
    Then the execution succeeds
    Then output "partner_geboortejaar_strikt" equals 1980

  Scenario: A nullable parameter may be passed as null
    Given parameter "toeslagpartner" is "null"
    When I evaluate "alleenstaand" of "test_nullable"
    Then the execution succeeds
    Then output "alleenstaand" is true

  Scenario: A parameter that is not nullable may not be passed as null
    Given parameter "aanvraag_bedrag" is "null"
    # The bron declared its amount optional (it may be left out) but not
    # nullable: an absence is not a value it takes, required or not.
    When I evaluate "past_aanvraag" of "test_nullable_bron"
    Then the execution fails
    Then the execution fails with "not declared nullable"
    Then the execution fails with "aanvraag_bedrag"

  Scenario: A null optional parameter is refused at the call when the target does not allow it
    Given the following "register" data with key "bsn" for law "test_nullable":
      | bsn         | bedrag |
      | "999993653" | null   |
    # Article 5 hands the register's absent amount to the bron as its optional
    # `aanvraag_bedrag`. The call is not skipped (the parameter is optional),
    # but the bron never declared that "geen bedrag" is a value it takes.
    When I evaluate "aanvraag_past" of "test_nullable"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "aanvraag_bedrag"
    Then the execution fails with "parameter from test_nullable"

  Scenario: An optional parameter that is left out stays unknown, nullable or not
    When I evaluate "past_aanvraag" of "test_nullable_bron"
    Then the execution succeeds
    Then output "past_aanvraag" is unknown for lack of "aanvraag_bedrag"
