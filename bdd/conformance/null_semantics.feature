@tier:core
Feature: Absent and unknown values — RFC-036
  Two kinds of "nothing" reach a law from its data, and they mean different
  things. An explicit null is absence: the register is authoritative and says
  there is none (no rent, no partner, no decision). A law may test for it, but
  not calculate with it or decide on it; doing so is an error the author has to
  resolve in the law. An empty cell is unknown: the fact exists and nobody has
  it yet. Unknown propagates through every operation and names the missing
  facts, so a decision process can ask for exactly those. A definite operand
  still decides a logical operation (AND with a false is false, OR with a true
  is true), whatever the unknown turns out to be.

  Driven against test_null_semantics (register inputs that may be absent or
  unknown; article 1 calculates, article 2 tests for absence, article 3 decides
  on a truth value, article 4 passes parameters on), test_null_semantics_bron
  (called cross-law; declares an optional form parameter) and
  test_null_semantics_strikt (forgets a required parameter). Article 5 of
  test_null_semantics calculates and decides on the fields of a record.

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    # The bron knows both persons; the engine resolves every input of an
    # article, so the cross-law calls need a row even when the scenario is
    # about something else.
    Given the following "register" data with key "bsn" for law "test_null_semantics_bron":
      | bsn         | geboortejaar |
      | "999993653" | 1975         |
      | "999993641" | 1980         |

  # ---------------------------------------------------------------------------
  # 1. Everything present computes as before
  # ---------------------------------------------------------------------------

  Scenario: Every value present computes as before
    Given parameter "aanvraag_bedrag" is 250
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_huur | partner_bsn | beschikking          | verzekerd | bedrag |
      | "999993653" | 650  | 700          | "999993641" | {"status": "ACTIEF"} | true      | 250    |
    When I evaluate outputs "huur_hoog, verhoogde_huur, niet_hoog, huurklasse, hoog_en_bekend, hoog_en_onwaar, hoog_of_onbekend, hoog_of_waar, huur_gelijk_aan_partner, lijsten_gelijk, lijst_in_lijsten, verzamelingen_gelijk" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_hoog" is true
    Then output "verhoogde_huur" equals 750
    Then output "niet_hoog" is false
    Then output "huurklasse" equals "hoog"
    Then output "hoog_en_bekend" is true
    Then output "hoog_en_onwaar" is false
    Then output "hoog_of_onbekend" is true
    Then output "hoog_of_waar" is true
    Then output "huur_gelijk_aan_partner" is false
    Then output "lijsten_gelijk" is false
    Then output "lijst_in_lijsten" is false
    Then output "verzamelingen_gelijk" is false
    When I evaluate outputs "huur_ontbreekt, heeft_huur, huur_hoog_veilig, huur_in_lijst, status, partner_geboortejaar" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_ontbreekt" is false
    Then output "heeft_huur" is true
    Then output "huur_hoog_veilig" is true
    Then output "huur_in_lijst" is true
    Then output "status" equals "ACTIEF"
    Then output "partner_geboortejaar" equals 1980
    When I evaluate outputs "verzekerd_en_waar, verzekerd_en_onwaar, verzekerd_of_waar, verzekerd_of_onwaar, niet_verzekerd, verzekeringsklasse" of "test_null_semantics"
    Then the execution succeeds
    Then output "verzekerd_en_waar" is true
    Then output "verzekerd_en_onwaar" is false
    Then output "verzekerd_of_waar" is true
    Then output "verzekerd_of_onwaar" is true
    Then output "niet_verzekerd" is false
    Then output "verzekeringsklasse" equals "ja"
    When I evaluate outputs "eigen_aanvraag_past, aanvraag_past_met_bedrag, aanvraag_past_uit_register" of "test_null_semantics"
    Then the execution succeeds
    Then output "eigen_aanvraag_past" is true
    Then output "aanvraag_past_met_bedrag" is true
    Then output "aanvraag_past_uit_register" is true

  # ---------------------------------------------------------------------------
  # 2. Explicit null is absence: a value the law can test for
  # ---------------------------------------------------------------------------

  Scenario: An explicit null answers the absence test
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" | null | null        | null        |
    When I evaluate outputs "huur_ontbreekt, heeft_huur, huur_hoog_veilig, huur_in_lijst" of "test_null_semantics"
    Then the execution succeeds
    # EQUALS against null is structural: this is the absence test the corpus uses.
    Then output "huur_ontbreekt" is true
    Then output "heeft_huur" is false
    # A law that tests for absence before it compares decides for itself what
    # "geen huur" means for the height of the rent.
    Then output "huur_hoog_veilig" is false
    # Membership is structural too: an absence is not in a list of amounts.
    Then output "huur_in_lijst" is false

  Scenario: A field of an absent record is absent
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" | 650  | null        | null        |
    When I evaluate outputs "status" of "test_null_semantics"
    Then the execution succeeds
    Then output "status" is absent

  Scenario: A null required parameter does not run the other law
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" | 650  | null        | null        |
    # There is no partner to look up: the bron is not executed and the input is
    # null, so the caller's own absence test can decide.
    When I evaluate outputs "partner_geboortejaar" of "test_null_semantics"
    Then the execution succeeds
    Then output "partner_geboortejaar" is absent

  # ---------------------------------------------------------------------------
  # 3. Calculating or deciding on an absence is an error
  # ---------------------------------------------------------------------------

  Scenario: An absent value the law declared never absent is refused before it is calculated with
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" | null | "999993641" | null        |
    # Article 1 orders and adds the rent and did not declare it nullable. The
    # register's null contradicts that declaration, and the contradiction is a
    # data error at the boundary (see nullable.feature), not a calculation
    # that happens to fail.
    When I evaluate "huur_hoog" of "test_null_semantics"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "huur"

  Scenario: An absent truth value the law declared never absent is refused before it is decided on
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | verzekerd |
      | "999993653" | null      |
    When I evaluate "verzekerd_en_waar" of "test_null_semantics"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "verzekerd"

  Scenario: Ordering or calculating with an absent record field fails
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | beschikking                      |
      | "999993653" | {"bedrag": null, "actief": true} |
    # A record is untyped, so no declaration can refuse the null field at the
    # boundary; the operation itself does. "More than nothing" is not a
    # question the law asked: it has to test for absence first (EQUALS … null).
    When I evaluate "bedrag_hoog" of "test_null_semantics"
    Then the execution fails
    Then the execution fails with "operand is null"
    Then the execution fails with "GREATER_THAN"
    Then the execution fails with "EQUALS"

  Scenario: Deciding on an absent record field fails
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | beschikking                     |
      | "999993653" | {"bedrag": 250, "actief": null} |
    # NOT and IF need a verdict; an absence is not one. Both outputs are
    # produced by the same article, so either fails the article.
    When I evaluate "inactief" of "test_null_semantics"
    Then the execution fails
    Then the execution fails with "operand is null"
    Then the execution fails with "NOT"
    When I evaluate "beschikkingsklasse" of "test_null_semantics"
    Then the execution fails

  Scenario: Present record fields compute as before
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | beschikking                     |
      | "999993653" | {"bedrag": 250, "actief": true} |
    When I evaluate outputs "bedrag_hoog, verhoogd_bedrag, inactief, beschikkingsklasse" of "test_null_semantics"
    Then the execution succeeds
    Then output "bedrag_hoog" is true
    Then output "verhoogd_bedrag" equals 350
    Then output "inactief" is false
    Then output "beschikkingsklasse" equals "actief"

  Scenario: A null optional parameter is passed through and the other law's declaration decides
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | bedrag |
      | "999993653" | null   |
    # Article 4 hands the register's `bedrag` (nullable: the register may hold
    # no amount) to the bron as its optional `aanvraag_bedrag`. Null for an
    # optional parameter does not stop the call, but the bron never declared
    # that "geen bedrag" is a value that parameter takes, so the absence is
    # refused at the call instead of failing inside the bron's comparison.
    When I evaluate "aanvraag_past_uit_register" of "test_null_semantics"
    Then the execution fails
    Then the execution fails with "declares as never absent"
    Then the execution fails with "aanvraag_bedrag"

  # ---------------------------------------------------------------------------
  # 4. An empty cell is unknown: the fact exists, nobody has it
  # ---------------------------------------------------------------------------

  Scenario: Comparing, calculating and testing an unknown value is unknown
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" |      | "999993641" |             |
    When I evaluate outputs "huur_hoog, verhoogde_huur, niet_hoog, huurklasse" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_hoog" is unknown
    Then output "huur_hoog" is unknown for lack of "huur"
    Then output "verhoogde_huur" is unknown for lack of "huur"
    Then output "niet_hoog" is unknown for lack of "huur"
    # An IF whose condition is unknown does not fall through to its default:
    # the branch might have applied.
    Then output "huurklasse" is unknown for lack of "huur"
    # An unknown inside a list or a collection is not compared away: the
    # list holding it might be the equal one, or the member of the list.
    When I evaluate outputs "lijsten_gelijk, lijst_in_lijsten, verzamelingen_gelijk" of "test_null_semantics"
    Then the execution succeeds
    Then output "lijsten_gelijk" is unknown for lack of "huur"
    Then output "lijst_in_lijsten" is unknown for lack of "huur"
    Then output "verzamelingen_gelijk" is unknown for lack of "huur"
    # Even the absence test cannot answer: whether the register holds a rent is
    # exactly the fact nobody has.
    When I evaluate outputs "huur_ontbreekt, heeft_huur, huur_hoog_veilig, status" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_ontbreekt" is unknown for lack of "huur"
    Then output "heeft_huur" is unknown for lack of "huur"
    Then output "huur_hoog_veilig" is unknown for lack of "huur"
    # A field of a record nobody has is unknown for that record.
    Then output "status" is unknown for lack of "beschikking"

  Scenario: Logic over an unknown stays unknown unless a definite operand decides
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking | verzekerd |
      | "999993653" |      | "999993641" | null        |           |
    When I evaluate outputs "hoog_en_bekend, hoog_en_onwaar, hoog_of_onbekend, hoog_of_waar" of "test_null_semantics"
    Then the execution succeeds
    # AND with an unknown and a true: unknown. With a false: false, whatever the unknown was.
    Then output "hoog_en_bekend" is unknown for lack of "huur"
    Then output "hoog_en_onwaar" is false
    # OR with an unknown and a false: unknown. With a true: true.
    Then output "hoog_of_onbekend" is unknown for lack of "huur"
    Then output "hoog_of_waar" is true
    # The same over a truth value that is itself unknown.
    When I evaluate outputs "verzekerd_en_waar, verzekerd_en_onwaar, verzekerd_of_waar, verzekerd_of_onwaar, niet_verzekerd, verzekeringsklasse" of "test_null_semantics"
    Then the execution succeeds
    Then output "verzekerd_en_waar" is unknown for lack of "verzekerd"
    Then output "verzekerd_en_onwaar" is false
    Then output "verzekerd_of_waar" is true
    Then output "verzekerd_of_onwaar" is unknown for lack of "verzekerd"
    Then output "niet_verzekerd" is unknown for lack of "verzekerd"
    Then output "verzekeringsklasse" is unknown for lack of "verzekerd"

  Scenario: Membership is true on a definite match and otherwise unknown
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" |      | null        | null        |
    When I evaluate outputs "huur_in_lijst" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_in_lijst" is unknown for lack of "huur"

  # ---------------------------------------------------------------------------
  # 5. Two unknown operands: the union of what is missing
  # ---------------------------------------------------------------------------

  Scenario: Two unknown operands name both missing facts
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_huur |
      | "999993653" |      |              |
    When I evaluate outputs "huur_gelijk_aan_partner" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_gelijk_aan_partner" is unknown for lack of "huur"
    Then output "huur_gelijk_aan_partner" is unknown for lack of "partner_huur"

  Scenario: Two lists that each hold an unknown are not equal but unknown
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn |
      | "999993653" |      |             |
    # Structurally the two lists look alike. That likeness is not a fact about
    # the rent or the partner; the comparison is unknown for both.
    When I evaluate outputs "lijsten_gelijk, lijst_in_lijsten, verzamelingen_gelijk" of "test_null_semantics"
    Then the execution succeeds
    Then output "lijsten_gelijk" is unknown for lack of "huur"
    Then output "lijsten_gelijk" is unknown for lack of "partner_bsn"
    Then output "lijst_in_lijsten" is unknown for lack of "huur"
    Then output "lijst_in_lijsten" is unknown for lack of "partner_bsn"
    Then output "verzamelingen_gelijk" is unknown for lack of "huur"
    Then output "verzamelingen_gelijk" is unknown for lack of "partner_bsn"

  Scenario: A row without the person is unknown for every input, not absent
    # No row at all for this bsn: nobody has any of the facts. That is not the
    # same as a row saying null.
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993641" | 650  | null        | null        |
    When I evaluate outputs "huur_ontbreekt, status" of "test_null_semantics"
    Then the execution succeeds
    Then output "huur_ontbreekt" is unknown for lack of "huur"
    Then output "status" is unknown for lack of "beschikking"

  # ---------------------------------------------------------------------------
  # 6. Parameters across laws
  # ---------------------------------------------------------------------------

  Scenario: An unknown required parameter does not run the other law
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur | partner_bsn | beschikking |
      | "999993653" | 650  |             | null        |
    # Nobody knows yet who the partner is: the bron is not executed and the
    # input is unknown for lack of the fact the parameter was built from.
    When I evaluate outputs "partner_geboortejaar" of "test_null_semantics"
    Then the execution succeeds
    Then output "partner_geboortejaar" is unknown for lack of "partner_bsn"

  Scenario: An optional parameter the caller does not pass is unknown, cross-law
    # Article 4 asks the bron for `past_aanvraag` with only bsn. The bron's
    # `aanvraag_bedrag` (required: false) is not filled in from anywhere: it
    # is unknown for lack of exactly that parameter, so the decision process
    # can ask for it.
    When I evaluate outputs "aanvraag_past" of "test_null_semantics"
    Then the execution succeeds
    Then output "aanvraag_past" is unknown for lack of "aanvraag_bedrag"

  Scenario: An optional parameter the caller does not pass is unknown at top level
    # The same rule for the law that is evaluated directly: its own optional
    # `aanvraag_bedrag` was not passed, and so is unknown where it is used and
    # where it is passed on to the bron.
    When I evaluate outputs "eigen_aanvraag_past, aanvraag_past_met_bedrag, aanvraag_past_uit_register" of "test_null_semantics"
    Then the execution succeeds
    Then output "eigen_aanvraag_past" is unknown for lack of "aanvraag_bedrag"
    Then output "aanvraag_past_met_bedrag" is unknown for lack of "aanvraag_bedrag"
    # An unknown optional parameter is passed through as well: no register row
    # holds `bedrag`, and the bron's comparison says so.
    Then output "aanvraag_past_uit_register" is unknown for lack of "bedrag"

  Scenario: A required parameter the caller does not pass is still an error
    # Only an optional parameter may be left out. A required one that is
    # missing stays the error it always was, so a misspelled key in
    # `parameters:` can never quietly become an unknown outcome.
    When I evaluate outputs "geboortejaar" of "test_null_semantics_strikt"
    Then the execution fails

  Scenario: A required parameter passed as null at the top level is the caller's error
    Given parameter "bsn" is "null"
    Given the following "register" data with key "bsn" for law "test_null_semantics":
      | bsn         | huur |
      | "999993653" | 650  |
    # The register was asked about nobody. That is not "unknown for lack of
    # huur" (nobody has the fact) and not an absence to decide on: the caller
    # forgot whom the law is about. Across laws the same situation is a skip
    # (see "A null required parameter does not run the other law").
    When I evaluate "huur_hoog" of "test_null_semantics"
    Then the execution fails
    Then the execution fails with "cannot be evaluated for nobody"
    Then the execution fails with "bsn"

  # ---------------------------------------------------------------------------
  # 7. The empty cell means one thing in every table
  # ---------------------------------------------------------------------------

  Scenario: An empty cell in a parameter table leaves the parameter out
    Given the following parameters:
      | bsn             | "999993653" |
      | aanvraag_bedrag |             |
    # Not passed, so the optional parameter is unknown for lack of it, exactly
    # as when the row is not there at all.
    When I evaluate "past_aanvraag" of "test_null_semantics_bron"
    Then the execution succeeds
    Then output "past_aanvraag" is unknown for lack of "aanvraag_bedrag"

  Scenario: The word null in a parameter passes an absence
    Given parameter "aanvraag_bedrag" is "null"
    # An absence is a value, and the bron's declaration says its amount is
    # never absent: the caller's null is refused before anything is ordered.
    When I evaluate "past_aanvraag" of "test_null_semantics_bron"
    Then the execution fails
    Then the execution fails with "not declared nullable"
    Then the execution fails with "aanvraag_bedrag"
