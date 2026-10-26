# POC feature conversion: notes

Log of converting the POC Gherkin feature files (Dutch step dialect) to the
canonical regelrecht BDD grammar (`bdd/grammar.yaml`) with
`corpus/demo/tools/convert_features.mjs`, and of running them with
`just bdd-demo`. Every judgement call lives in the converter or in one of the
two law-rewrite tools next to it; the generated `.feature` files under
`corpus/demo/regulation/nl/**/scenarios/` are never edited by hand.

Regenerate and run:

    node corpus/demo/tools/convert_features.mjs <poc>/features corpus/demo/regulation/nl
    uv run corpus/demo/tools/rewrite_array_property_access.py corpus/demo/regulation/nl
    uv run corpus/demo/tools/round_amount_outputs.py corpus/demo/regulation/nl
    uv run corpus/demo/tools/pass_brp_referentiedatum.py corpus/demo/regulation/nl
    node corpus/demo/tools/apply_absent_semantics.mjs corpus/demo
    uv run corpus/demo/tools/declare_nullable.py corpus/demo
    node corpus/demo/tools/apply_nullable_cells.mjs corpus/demo
    just bdd-demo

The tools are idempotent; running them again reports 0 rewrites. The hand
changes listed under "Made by hand" and in the RFC-036 sections are not
reproduced by a regeneration.

## Counts (last run)

| | |
|---|---|
| POC feature files | 46 |
| skipped as non-engine (`integratie/`, `web/`, `synthesize.feature`) | 5 |
| converted feature files | 41 |
| scenarios converted | 331 |
| `@wip` (skipped by the runner) | 0 |
| run by `just bdd-demo` | 331 |
| passing | 331 |
| failing | 0 |

## Rules applied

### Structure

- One canonical `Background:` with the calculation date and the parameters of the
  POC Background. Register data goes into each scenario, because it differs per
  scenario. A POC Background table is inherited by every scenario.
- Every POC `When de <law> wordt uitgevoerd door <service>` becomes
  `When I evaluate outputs "<asserted outputs>" of "<law $id>"`; the output list is
  exactly what the scenario's Then steps assert (deduplicated, in order). A
  scenario with several Whens keeps them as phases (archiefwet, omgevingswet,
  kinderopvang, huurtoeslag); parameters a later phase adds (a `met` table, a
  claims table) are emitted right before its When.
- The law is looked up by `$id` (the POC law name); when several versions exist
  the latest `valid_from` not after the calculation date decides which outputs
  exist.
- The generated file lives in the directory of the first evaluated law:
  `corpus/demo/regulation/nl/<law dir>/scenarios/<poc file name>`.

### Given steps

| POC | canonical |
|---|---|
| `de datum is "D"` | `the calculation date is "D"` |
| `een persoon met BSN "X"` | `parameter "bsn" is "X"` |
| `een organisatie met KVK-nummer "X"` / `een onderneming met KVK nummer "X"` | `parameter "kvk_nummer" is "X"` |
| `een werkgever met loonheffingennummer "X"` | `parameter "loonheffingennummer" is "X"` |
| `een werknemer met bruto jaarloon "X" euro` | `parameter "bruto_loon" is X` |
| `de aanvraag betreft een "X"` | `parameter "aanvraag_type" is "X"` |
| `een <entity> met ID "X"` | parameter per steps.py (`ICT-project`/`project` -> `project_id`, `organisatie` -> `organisatie_id`, else `<entity>_id`) |
| `een archiefstuk met de volgende eigenschappen` + table | `the following parameters:` (name/value rows) |
| `er is geen Bibob-advies uitgebracht voor deze onderneming` | comment (no bibob_adviezen row applies) |
| `de volgende <SERVICE> <table> gegevens` + table | register rows for the materialiser (below) |
| `When ... met` + table, `When de burger deze gegevens indient:` (claims) | `the following parameters:` before the When; a claim overrides the input of the same name, which is what a parameter does in the engine |

Parameters the evaluated law declares but the scenario never supplies are passed
as `parameter "x" is "null"`: they were None in the POC, and the engine has no
default for an absent parameter (the schema has none either).

A POC phase that only established that required data was still missing
(`ontbreken er verplichte gegevens`) is dropped with a comment; it is an
application-level check the engine has no equivalent for. The scenario continues
with the values the citizen then supplied.

### Then steps

All mappings follow `features/steps/steps.py`; the notable ones:

- `is voldaan aan de voorwaarden` -> `output "voldoet_aan_voorwaarden" is true`;
  when the law has no such output -> `the execution succeeds` (steps.py:
  `requirements_met = bool(outputs)`). The negative form without the output would
  become a comment and tag the scenario `@wip`; it did not occur.
- Euro amounts (`is het toeslagbedrag "X" euro` etc.) -> eurocent integer
  (`Math.round(X * 100)`); the output name follows steps.py (`hoogte_toeslag`
  else `jaarbedrag`, `pensioen_uitkering_maandelijks` else `pensioenbedrag`,
  `uitkeringsbedrag`, `woonkostentoeslag`, `startkapitaal`, `bedrijfskapitaal_max`,
  `subsidiebedrag`). `is het <veld> "N" eurocent` uses steps.py's field aliases.
- `heeft de output "f" waarde "v"` / `is het veld "f" gelijk aan "v"`: `v` is read
  as the POC did (JSON, then int, then string) and emitted as `is true/false`,
  `is null`, `equals N` or `equals "s"`.
- `is de output "f" leeg` / `is het veld "f" een lege lijst` ->
  `output "f" equals "[]"`. This leans on the JSON-cell extension below: a quoted
  value starting with `[` or `{` is parsed as JSON.
- `bevat de output "f" waarde "v"` -> `output "f" contains "v"` only when the law
  types `f` as a string. Array membership (177 POC assertions, nearly all in the
  delegation-provider laws) is not expressible in the canonical grammar and
  becomes a comment; negative membership likewise. Those scenarios keep their
  other assertions (`heeft_delegaties`, `voldoet_aan_voorwaarden`, ...).
- Approximate POC assertions (WW 1%, kindgebonden budget 2%) are emitted exactly;
  where the engine's value differs it is adopted through the `ADOPTED` table in
  the converter with the POC's text as a comment. One case: kindgebonden budget
  392541 where the POC said "ongeveer €3.925,00".
- The qualitative kindgebonden-budget steps (`lager door hoog inkomen`, ...) would
  be emitted as exact assertions on the observed value via `ADOPTED`; they only
  occur in the skipped `integratie/` files.
- Case-management steps and the whole `integratie/`, `web/` and
  `synthesize.feature` files are skipped: they test the POC application, not the
  engine.

### Register data

The POC tables are fed through the demo materialiser
(`frontend-demo/src/data/materialize.js`, the code the demo app uses) with
`corpus/demo/bindings.yaml`. Cells are parsed the way steps.py did: the
`STRING_FIELDS` columns stay strings (`null`/empty -> null), everything else JSON,
then int, then string. A table given again (in the scenario after the Background,
or twice in a scenario) replaces the earlier rows, as `set_source_dataframe` did.

For every law reachable from the evaluated law(s) through `source.regulation`
one step is emitted per (service, key field):

    Given the following "<service>" data with key "<key>" for law "<law>":

with every materialised input of that law as a column (null included), so the
runner sees the same picture as the demo app. Arrays and objects are written as
JSON in the cell. Key values are every distinct value of a key column (`bsn`,
`kvk_nummer`, `organisatie_id`, ...) across the scenario's tables plus the
parameters; for `bsn`, every `*bsn*` column counts (partners, children). An
object-valued key (`adres` in the LAA law) is written as JSON; the engine keys
every complex value as `"complex"`, which works because a scenario has one.

Four POC readings of a materialised value that the materialiser does not share
are applied in the converter (`materialiseScenario`, `applyPocValueSemantics`):

1. A binding that selects on a cross-law input (`adres: $vestigingsadres` in the
   APV laws and precariobelasting, `bsn: $partner_bsn` in wet_inkomstenbelasting
   and wet_studiefinanciering, 31 bindings) is pre-resolved by following the
   reference to the law that produces the output; when that output is a plain
   projection of a table-bound input, that input is materialised for the same
   key (a record keyed on another field, such as `terras_locatie`, uses the
   scenario's single kvk_nummer). The materialiser itself resolves `$x` only
   against parameters and table-bound inputs, so the demo app does not get these
   values today.
2. (Withdrawn with RFC-036.) A numeric input whose cell was null used to become
   0, the way the POC's ADD skipped None operands. What a missing register row
   means is now the binding's `absent:` in `bindings.yaml`, and the materialiser
   writes exactly that; see the RFC-036 section below.
3. An object input (a register row) carries every property the law reads on it,
   null when the row lacks the column: `row.get(kolom)` was None in the POC, the
   engine treats a missing key as an author error. (The 0 for a property passed
   straight through to a number/amount output is withdrawn with rule 2.)
4. An array that consists of nulls only (a column the table never had) becomes
   null: FOREACH treats null as an empty collection, which is what the POC's None
   amounted to.

Tables the bindings never reference are dropped silently. In the POC features
these are `BELASTINGDIENST inkomen`, the LAA `terugmelding`/`post_onbestelbaar`
tables, `DJI detentie`, `GEMEENTE_ROTTERDAM bibob_adviezen`, `IND vreemdelingenwet`,
`KVK bedrijfsgegevens`, `RECHTSPRAAK curatele`, `RvIG kinderen`, `UWV WIA` and
`UWV wet_structuur_uitvoeringsorganisatie_werk_en_inkomen`: mostly POC tables
named after a law, which the POC engine read as output overrides. The migrated
laws compute those outputs from the register tables the bindings do know. The
three LAA "meldt twijfel" scenarios pass because a missing register adres counts
as a deviation, not because their `terugmelding` rows reach the engine.

### Engine test-helper change

`packages/engine/tests/bdd/helpers/value_conversion.rs`: a table cell (and, through
the same function, a quoted capture) that starts with `[` or `{` and parses as JSON
becomes an Array/Object value. Needed for array- and object-typed register inputs
(`kinderen_leeftijden`, `adres`). A cell that does not parse stays a string. Unit
test `test_convert_json_array_and_object` in the same file (runs in the
`bdd_table` test target). Mirrored in `packages/frontend-shared/src/gherkin/actions.ts`
(`parseValue`).

## Law and binding changes

All in `corpus/demo/`; each carries a YAML comment at the site. The demo laws
all validate against schema v0.5.7 after the changes (`script/validate.sh`).

### Made by tools (re-runnable, idempotent)

`corpus/demo/tools/rewrite_array_property_access.py`:

- POC implicit list mapping `$lijst.veld` on an array-valued variable -> `FOREACH`
  without `combine` (returns the body values as an array). 40 action values in
  12 laws (the delegation providers, handelsregisterwet, machtigingenwet, wgbo).
- POC `EXISTS $lijst` on an array (migrated as `NOT (EQUALS $x null)`, true for an
  empty list) -> `GREATER_THAN (FOREACH count) 0`, false for null and for the
  empty list, as EXISTS was. 27 sites in 13 laws. Scalars keep the null test.
- Reported, not rewritten: `handelsregisterwet` compares `$inschrijvingen.rechtsvorm`
  (a list) with `IN`; the POC semantics there have no single equivalent. No
  scenario reaches it with data.

`corpus/demo/tools/round_amount_outputs.py`: explicit rounding (RFC-024) on the
outputs whose POC scenarios assert whole eurocents:
`algemene_ouderdomswet.pensioenbedrag`, `pensioenwet.pensioen_uitkering_maandelijks`,
`werkloosheidswet.ww_uitkering_per_maand`, `wet_inkomstenbelasting.totale_belastingschuld`
and `.totale_heffingskortingen`, `wet_op_het_kindgebonden_budget.kindgebonden_budget_jaar`,
`zorgtoeslagwet.hoogte_toeslag` (both versions), `zvw/werkgeversbijdrage.zvw_werkgeversbijdrage`,
`participatiewet/bijstand/amsterdam.uitkeringsbedrag`, and
`wet_structuur_uitvoeringsorganisatie_werk_en_inkomen.verzekerde_jaren` (precision 2,
which makes the AOW opbouw 48/50 exactly as in the POC). ROUND (half-up) is used:
the POC expectations 126307.84 -> 126308, 209691.79 -> 209692, 210820.80 -> 210821
and 470595.96 -> 470596 rule out truncation. The one exception is
`werkloosheidswet.ww_duur_maanden`, where 12.5 is asserted as 12: FLOOR. The
rounding is null-guarded (`IF value null -> null`), because the engine's
arithmetic lets a null operand through but ROUND/FLOOR insist on a number; WW
evaluated for a Bbz applicant without dienstverbanden produced None in the POC
and would otherwise fail here.

### Made by hand

- Input order (the engine resolves inputs in declaration order; the POC resolved
  lazily): the register inputs that cross-law inputs pass as `$parameter` are
  declared first in `alcoholwet/vergunning/rotterdam` (bsn, vloeroppervlakte,
  type_bedrijf), `algemene_plaatselijke_verordening/exploitatievergunning`
  (bsn_eigenaar), `kieswet` (verkiezingsdatum) and
  `omgevingswet/energiebesparing/informatieplicht` (adres before bag_gebruiksdoel).
- `wet_brp`: `leeftijd` (AGE), `heeft_vast_adres`, `postadres` and `woonplaats`
  read a date or `$adres.*` that was None in the POC for a person without
  register data; the engine raises on a path on null or a null date, so the
  actions test for a value first (null / false otherwise).
- `zorgtoeslagwet` (2024 and 2025): the age test read `wet_brp#leeftijd` on the
  calculation date. Wzt art. 1 lid 1 onder c makes a person a verzekerde from the
  month after the month of the eighteenth birthday, so the law now reads
  `wet_brp#geboortedatum` and tests the age on the last day of the month before
  the calculation month. The edge cases are in
  `zorgtoeslagwet_verzekerde_vanaf_18-2025-01-01.feature`.
- `zorgtoeslagwet` (2024 and 2025): `hoogte_toeslag` is computed only when
  `voldoet_aan_voorwaarden` holds, and is absent otherwise. The POC returned 0
  for a person without aanspraak; the law (art. 2 lid 1, art. 3 lid 1) gives no
  aanspraak at all, which is not an aanspraak on nothing. The wealth test of
  art. 3 lid 1 moved into `voldoet_aan_voorwaarden`. The under-18 and wealth
  scenarios assert `hoogte_toeslag` is absent, and
  `zorgtoeslagwet_geen_aanspraak-2025-01-01.feature` covers wealth in 2025.
- `burgerlijk_wetboek_handelingsonbekwaamheid.curator_bsn`: the one SWITCH-shaped
  site the tool reports; rewritten by hand to the same count + FOREACH form.
- `wet_structuur_uitvoeringsorganisatie_werk_en_inkomen`: a dienstverband without
  end date (empty or null) counted to the reference date in the POC; the engine's
  DATE_DIFF needs a date, so `to:` falls back to `$referencedate`.
- `wet_brp/terugmelding/belastingdienst` and `.../toeslagen`: the POC compared the
  BRP adres with the register adres as dicts that only held the register's
  columns (postcode, huisnummer). The materialised object also carries straat and
  woonplaats (null), so structural equality always failed; the comparison is now
  field-wise on postcode and huisnummer, with a missing register adres still
  counting as a deviation (as in the POC). Toeslagen also guards
  `$toeslagen_onderzoek.adres_geverifieerd` against a missing onderzoek.
- `alcoholwet/vergunning/rotterdam` passes the two optional parameters of
  `alcoholwet/vergunning` explicitly: `is_ingeschreven_svh_register` from its own
  `svh_registratie_geldig` input and `is_van_slecht_levensgedrag` from a new
  definition `geen_slecht_levensgedrag: false`. They were None in the POC when
  absent. The engine has no default for an absent parameter, and a null parameter
  makes it skip the referenced law altogether (RFC-007), so a constant is the
  only way to say "no such registration" here.
- `wet_kinderopvang.is_gerechtigd` was the constant `true`; the POC only computed
  outputs when the requirements held, so it read as false for someone who did not
  qualify. Now `$voldoet_aan_voorwaarden`. The other 25 constant-true boolean
  outputs in the corpus are left alone: for most of them the constant is the
  intended meaning (`heeft_delegaties: true`, "everyone has a SELF delegation"),
  and no scenario depends on the gating.
- By the coordinator, kept: `algemene_ouderdomswet/leeftijdsbepaling` declares an
  optional `bsn` parameter and its eight callers pass `bsn: $bsn`. (The
  materialiser's default of 0 for a number/amount input without a register row
  that this bullet used to mention is gone; see the RFC-036 section below.)

## RFC-036: afwezig en onbekend

RFC-036 gives the engine two values for "nothing": `null` is an absence the
data states (the register is authoritative: no partner, no permit), an
*unknown* is a fact nobody supplied, and it names what is missing. Arithmetic
and ordered comparison on `null` are errors ("a law does not calculate with
'geen' without saying so"); an optional parameter the caller leaves out is
unknown; an input no data source has a value for is unknown. The first run of
`just bdd-demo` under that engine failed 61 of 330 scenarios: 34 `AbsentOperand`
errors and 27 unknown outcomes for lack of `wet_brp.referentiedatum`. What was
changed, and why, in the order it was done:

### `wet_brp.referentiedatum` is required and every caller passes it

The age depends on a reference date, and the calling law decides which date.
`wet_brp` used to fall back to the calculation date when the parameter was
left out; under RFC-036 a left-out optional parameter is unknown, so the age
was unknown for 20 of 21 callers. The parameter is now `required: true` and
`corpus/demo/tools/pass_brp_referentiedatum.py` (text-based, idempotent) adds
`referentiedatum: $referencedate` to every `regulation: wet_brp` call that does
not name a date: 58 calls in 20 laws. The kieswet keeps passing its
`$verkiezingsdatum`, now for both of its `wet_brp` calls (age and nationality
on the election date).

### `bindings.yaml` says what a missing register row means

Every scalar `kind: table` binding carries `absent:` (see the header of that
file): `unknown` (36 bindings: the register cannot say "none"), `null` (78: the
register is authoritative for non-existence) or `0` (31: an income or count
register where no row means no such income). The first pass reproduced the
materialiser's old type heuristic (amount/number -> 0, else null), generated by
`migrate_poc_laws.py`, which now emits the key; 83 bindings were then reviewed
and carry a comment. The deviations from the heuristic: the BRP `personen`
columns, the Rotterdam `inrichtingen`/`leidinggevenden`, the ANW
`ao_percentage`, the BGT/terrassenbeleid/precario tables, the RVO energy and
mobility tables, the ACICT projects, the CBS life expectancy and the four
register-wrapper objects (`pensioengegevens`, `werkgegevens`,
`inkomensgegevens`, `vermogensgegevens`) went from null/0 to unknown; the six
`pensioen_deelnemers` amounts went from 0 to null (no participation, and the
pensioenwet decides on that); the AWB `organisaties` table was reviewed and
kept its heuristic (an exhaustive description per organisation). The
materialiser (`frontend-demo/src/data/materialize.js`) follows `absent`, never
the type; a `kind: claim` input is left out until the citizen supplies it; a
lookup on a null selector (the partner's income without a partner) is null; a
lookup on a selector nobody has is unknown; a matched row that lacks the
`field:` column follows `absent` like a missing row.

### The scenario tables

`corpus/demo/tools/apply_absent_semantics.mjs` rewrote the generated data
tables (the POC repository has since moved on, in law layout, BSN ranges and
parameter emission, so regenerating with `convert_features.mjs` no longer
reproduces the committed files; the committed features are the source now and
carry a second header line naming the tool). Per column it follows the
binding: 481 `null` cells in `absent: unknown` columns became empty cells
(the runner omits the key: unknown); 24 claim columns (33 cells) that the old
materialiser had filled with null/0 were dropped, 31 rows and 28 steps that
then stated nothing were dropped; 23 fabricated `0` cells for the CBS life
expectancy were replaced by the figure `profiles.yaml` holds globally for the
scenario's year (20.4 to 20.7). `null` cells in `absent: null` columns and `0`
cells in `absent: 0` columns are unchanged. The 67 `parameter "x" is "null"`
steps are unchanged too: a scenario that passes null states an absence the
law decides on. `convert_features.mjs` was brought in line (no numeric fill,
an omitted key is an empty cell) for a future regeneration from a POC checkout
that matches. Its two sites that wrote a quoted Gherkin capture used to escape
`"` but not `\`; neither runner unescapes anything (the step patterns capture
`"([^"]*)"` and pass the text on as is), so an escape would be read literally.
They now share `quoted()`, which refuses a value containing a quote or a
backslash instead; no POC value has needed one.

### Laws changed (each with a YAML comment at the site)

- `algemene_ouderdomswet/leeftijdsbepaling`: the increase for those born from
  1960 had no `default`; when the life expectancy did not exceed the reference
  value the increase was an absence and the `ADD` failed. Art. 7a lid 2 AOW
  sets V to 0 in that case: `default: 0`. This one gap caused every
  `AbsentOperand` failure (zorgtoeslag, kinderopvang, huurtoeslag,
  inkomstenbelasting, Bbz, bijstand Amsterdam) through `is_aow_leeftijd`.
- `wet_studiefinanciering`: `studiefinanciering` and `partner_studiefinanciering`
  are 0 when there is no `onderwijstype` (art. 2.1 WSF 2000: only an enrolled
  student has a right); the per-type `IF` had no case for a non-student and the
  `MULTIPLY` failed.
- `pensioenwet`: `voldoet_aan_voorwaarden` is false and `pensioen_uitkering_bruto`
  is 0 without a participant record (`type_regeling` null; art. 1 Pensioenwet,
  deelnemer), because the six fund inputs are now absent instead of 0.
- `vreemdelingenwet`: the "TIJDELIJK" case compares the permit's dates only
  when both are recorded (art. 14 Vw 2000); a permit row without dates made
  the comparison fail.
- `wet_brp`: `referentiedatum` required, the fallback in `leeftijd` removed.
- `archiefwet/*` (9 parameters), `kernenergiewet` (`technologie_beschrijving`),
  `besluit_basisveiligheidsnormen_stralingsbescherming` (2): application-form
  parameters that scenarios pass as null are `required: false`; the engine
  rejects a null for a required top-level parameter, and these laws test for
  the absence themselves.

### The demo application

The portal derives the next question from the outcome's missing facts
(`askedInputs.ts`: `no_data` for a `kind: claim` input, `not_passed` for a form
parameter, required parameters first), passes no `null` for an unanswered
form field, shows `null` as "geen" and an unknown as "onbekend" with
"ontbreekt: ...", never takes an unknown verdict for a yes (tile: "Nog niet te
bepalen"; application: to the caseworker; simulation: counted as "onbekend"
and the disposable income of that subject unknown rather than 0). Records
handed to the engine never contain `undefined`.

## Absence is declared: `nullable` (schema v0.5.8)

RFC-036's follow-up makes absence a property of the type: a parameter, input
or output carries `nullable: true` when `null` is a legitimate value of it,
and the engine refuses a `null` at every boundary where the declaration says
none can occur (a register delivering null for a non-nullable input, a
non-nullable output evaluating to null, a non-nullable parameter passed null).
The validator runs a static type check (RFC-037) on top: `EQUALS … null` only
on a nullable field (N1), a literal null only as a nullable output's value
(N2), an `IF` without `default` only as a nullable output's value (N3), a
nullable variable only in arithmetic, ordering, logic, dates and `FOREACH`
after an absence test on that same variable (N4), a cross-law input nullable
when the output it takes is (N5), and the general typing rules T1 to T4. With
the declarations in place the first run of `just bdd-demo` under that engine
failed 184 steps (118 "declares as never absent", 45 "cannot be evaluated for
nobody"); the run after the work below passes 330 of 330.

### `declare_nullable.py` derives the declarations

`corpus/demo/tools/declare_nullable.py` (run with `uv run`, `--report` for a
dry run) reads the laws, `bindings.yaml` and the scenario files and marks a
field nullable under one or more criteria, iterating to a fixpoint over the
corpus because a nullable output makes the consuming input nullable:

| criterion | fields | meaning |
|---|---|---|
| `binding` | 77 | the input's binding says `absent: null` |
| `null-test` | 59 | the law compares the field with a literal null |
| `cross-law` | 36 | the input takes a cross-law output that is nullable |
| `through` | 34 | the output passes a nullable variable, or a property of a nullable record, straight through |
| `selector` | 30 | the input is a table lookup keyed on a nullable variable (the partner's data without a partner) |
| `skip` | 19 | the input is a cross-law call that passes a nullable variable for a required parameter (RFC-007 skip rule) |
| `scenario` | 16 | a scenario passes the parameter as `"null"` |
| `no-default` | 14 | the output's value is an `IF` without `default` |
| `literal` | 7 | the output's value can be a literal null |

225 fields in 49 of the 80 laws (151 inputs, 55 outputs, 19 parameters; a
field may satisfy several criteria). Those 49 laws moved their `$schema` to
v0.5.8, the version that defines the attribute; the other 31 declare nothing
and stay on v0.5.7. The tool is idempotent and reports a declaration no
criterion derives any more as `STALE`; it never removes one. The `argument`
criterion (a nullable variable passed to an optional parameter) fired nowhere.

The demo's `bindings.yaml` and the laws are held together by
`frontend-demo/src/data/bindings.test.ts` (part of `npm test -w frontend-demo`):
`absent: null`, or a `select_on` on a nullable variable, requires
`nullable: true` on the input; every other scalar table input must not be
nullable; an array input carries neither. Two mismatches came out of it, both
on the law side (below: `wet_brp.geboortedatum`,
`burgerlijk_wetboek_minderjarigheid.persoonsgegevens`). One binding changed:
`kieswet.verkiezingsdatum` went from `absent: null` to `absent: unknown`, since
a register without a scheduled election cannot name a date; an absent date
would have made `wet_brp` skip the age on the election day and the right to
vote null, deciding nothing.

`corpus/demo/tools/apply_nullable_cells.mjs` rewrites a `null` cell in a
data-table column whose input is not nullable (to `0` for an `absent: 0`
binding, to an empty cell otherwise). After `apply_absent_semantics.mjs` every
remaining `null` cell already sat in a nullable column, so it changed 0 cells;
it stays as the check for a future regeneration. The 67 `parameter "x" is
"null"` steps are covered by the `scenario` criterion.

### Dead absence tests removed (mismatches and N1)

A test on a field that can never be null is dead under the type system, and
it was pulling nullability into every consumer. Each site carries a comment:

- `wet_brp`: the BRP has a birth date for every registered person (art. 2.7
  lid 1 onder a Wet BRP); a person the BRP does not know is unknown, not
  absent (`absent: unknown`). `leeftijd` is `AGE` without the POC's null
  guard. Before this, `leeftijd` was nullable and with it the `leeftijd` input
  of 15 laws and, through `geboortedatum`, the `pensioenleeftijd` of 8.
- `algemene_ouderdomswet/leeftijdsbepaling`: `geboortedatum` is a required,
  never absent parameter.
- `burgerlijk_wetboek_minderjarigheid`: `persoonsgegevens` is the BRP row,
  unknown for a person the BRP does not know. The property tests
  (`$persoonsgegevens.heeft_handlichting`) stay.
- `wet_brp/laa`, `wet_brp/terugmelding/*` (3), `wet_bag`: the `adres`
  parameter is required and never null (a caller without a BRP address skips
  the call, a top-level null is refused at the boundary).

In all seven of these laws the POC's `voldoet_aan_voorwaarden` was that dead
presence test and nothing else; none of them takes a decision about the person
(a register answers a question, LAA and terugmelding decide `genereer_signaal`
and `heeft_gerede_twijfel_adres`). A constant `true` would have said "voldoet"
for a person the register does not know, whose every other output is unknown.
The output is gone; nothing read it (no cross-law source, no dashboard output,
the demo treats a law without it as "computed for all"), and the nine
`voldoet_aan_voorwaarden is true` assertions in the LAA scenarios went with it.

### The RFC-024 rounding guard is gone

`round_amount_outputs.py` used to wrap `IF EQUALS(expr, null) THEN null DEFAULT
ROUND(expr)`. Under RFC-036 arithmetic on an absent operand is an error, never
a null, so the guard was dead, and the checker reads `EQUALS <arithmetic> null`
as N1 and the `then: null` as a nullable output, which then made
`algemene_ouderdomswet.pensioenbedrag` and its consumers nullable. The tool now
writes the bare `ROUND` and unwraps a guard it finds (12 outputs in 11 laws).

### Guards added (N3, N4)

Where the checker found a real gap, the law got the guard or default the legal
text supports, with a comment citing the article. The type checker establishes
presence only for the variable an absence test names, so a guard on a
correlated variable (`heeft_partner`, `partner_bsn`) does not cover the
partner's income, which is a separate nullable input (null exactly when there
is no partner: the null-key rule of the bindings). A rule is narrowed, never a
law bent (RFC-037): the guard stays on the fact the article names, and a field
that can be absent gets, where the text says an absent component counts as
nothing (an income nobody has, an asset nobody owns), a per-field fallback
`IF EQUALS $x null THEN 0 ELSE $x` on that field. An `AND` of presence tests
that zeroes the whole box or denies the whole verdict when one field is absent
encodes a rule the article does not contain, and is not used. Where the article
itself requires the field (the partner's age for "jonger dan", art. 8 AOW; the
exploitant's age, art. 2:28 lid 3 APV; the fund's pension age, art. 18 lid 4 Pw)
its presence is tested and the comment says so.

- `algemene_kinderbijslagwet`: `ontvangt_kinderbijslag`, `aantal_kinderen`,
  `kinderen_leeftijden` are false, 0 and `[]` without an SVB record (art. 7
  AKW); they were properties of an absent record and the kindgebonden budget
  could not calculate with them.
- `wet_studiefinanciering`: the per-onderwijstype choices and the aanvullende
  beurs have `default: 0` (art. 2.1 jo. 3.1 and 3.9 WSF 2000); nested in ADD
  and MULTIPLY an `IF` without default is N3. In the partner's aanvullende
  beurs a parental income that is not there (one parent) and an absent count
  of studying siblings count 0 (art. 3.9 WSF 2000), per field.
- `wet_op_het_kindgebonden_budget`, `zorgtoeslagwet` (both versions): the
  partner/no-partner `IF` under `ROUND` had two boolean cases and no default;
  it is now `heeft_partner true -> B, default A`. In KGB an absent
  `partner_vermogen` or `partner_toetsingsinkomen` counts 0 inside the sum
  (art. 7 Awir), under the `heeft_partner` guard.
- `besluit_bijstandverlening_zelfstandigen`: `NOT(EQUALS $bbz_aanvraag null)`
  opens the conditions (art. 2 lid 1 jo. 35 Bbz 2004); they read properties of
  the application record, absent for anyone who did not apply.
- `wet_inkomstenbelasting`: the partner boxes and `gezamenlijk_vermogen` keep
  the single guard `NOT(EQUALS $partner_bsn null)` (art. 2.17 Wet IB 2001);
  inside it every partner amount counts 0 when absent, per field, and so does
  `partner_buitenlands_inkomen` in both `partner_inkomen` outputs.
- `wet_kinderopvang`: the partner's income counts 0 when absent (art. 1.7 Wko
  jo. art. 7 Awir); the partner's worked hours are tested before the comparison
  (art. 1.6 lid 3 Wko requires them).
- `participatiewet/bijstand`: under `heeft_partner`, `partner_bezittingen` and
  `partner_inkomen` count 0 when absent (art. 32 and 34 Pw).
- `algemene_ouderdomswet`: the partner's age and AOW age are tested before the
  toeslag comparison; art. 8 AOW ("jonger dan de pensioengerechtigde
  leeftijd") requires both.
- `pensioenwet`: `type_regeling` null is the deelnemer test (art. 1 Pw) for
  both the verdict and the uitkering; the fund's pension age is required by
  the verdict itself (art. 18 lid 4 Pw) and tested; kapitaal, jaren, loon and
  franchise count 0 when absent, each only in the regeling that uses it, so a
  participant in an uitkeringsovereenkomst without kapitaal stays a
  participant.
- `algemene_plaatselijke_verordening/exploitatievergunning`:
  `leeftijd_exploitant` is tested before the age conditions (art. 2:28 lid 3
  APV Rotterdam requires the exploitant's age); without a registered owner
  there is no age.
- `algemene_plaatselijke_verordening/terrassen`: `beschikbare_oppervlakte`,
  `max_sluitingstijd_doordeweeks` and `max_sluitingstijd_weekend` are tested
  before use (art. 2:28 jo. 2:30b APV Rotterdam: without a KVK registration
  there is no vestigingsadres and no BGT location or beleidsgebied to look
  up); `vergunde_oppervlakte` is 0 and the vergunde sluitingstijden are
  absent without them; `precariobelasting_per_jaar` tests the surface and the
  tariff (art. 2 jo. 5 Verordening precariobelasting Rotterdam).

`IN` on a nullable subject is not flagged (`IN(null, list)` is a definite
false, RFC-036), so the register statuses tested with `IN`
(`penitentiaire_beginselenwet.status`, `wet_forensische_zorg.zorgtype`, …)
carry only the declaration.

### Type errors the checker found (T1, T3)

- `algemene_plaatselijke_verordening/terrassen`: `gewenste_sluitingstijd_*`
  were declared `string` and compared with the policy's number; the scenarios
  pass 23 and 24. Now `number`, on one scale with the policy's
  `max_sluitingstijd_*`: hours from the start of the exploitatiedag, 24 is
  midnight, 25 is 01:00, 26 is 02:00 (the POC's `1` for 01:00 sorted below
  every evening hour, so 01:00 passed a 23:00 policy). The persona data
  already used 25 and 26; a scenario requests 01:00 against a 23:00 policy and
  is refused.
- `awb/bezwaar`, `awb/beroep`: the count of "Objected" events summed the
  booleans of an `EQUALS` in `FOREACH combine ADD`; now `filter:` plus
  `body: 1` (RFC-016).
- `wet_brp.woonsituatie` compared the residence address (object) with the
  parents' addresses (array) with `EQUALS`, never true; now `IN`. Nothing in
  the demo data has parent addresses, so no outcome changes.
- `zvw.registratie` was declared `object` while the binding reads one status
  column compared with `ACTIEF`; now `string`.

Not changed after discussion with the engine side: a literal null inside a
`LIST` or an uncombined `FOREACH` body (`valid_from_dates: [null]` in the
delegation laws) is a value, and N2 was narrowed to leave container literals
alone.

## `@wip`

None. The one scenario that was here, `zorgtoeslagwet_TOESLAGEN-2025-01-01 ::
Persoon onder 18 heeft geen recht op zorgtoeslag`, had POC data that contradicted
its name: born 2007-01-01, the person is 18 on the calculation date 2025-02-01 and
a verzekerde from that date (Wzt art. 1 lid 1 onder c). `CORRECTED` in
`convert_features.mjs` moves the birth date to 2008-01-01. The POC tested the age
on `$prev_january_first`, which is not the date the law names either; the demo law
now tests the month rule of art. 1 lid 1 onder c itself, and
`zorgtoeslagwet_verzekerde_vanaf_18-2025-01-01.feature` holds the edge cases.

## Worth knowing

- The engine skips a referenced law when a *required* parameter passed to it is
  null or unknown (`resolve_external_input_internal`, RFC-007, RFC-036) and
  errors when a declared required parameter is absent. An optional parameter
  the caller leaves out is unknown in the target (`not_passed`); a null passed
  to it is an absence the target decides on.
- `$name.prop` on an array errors (`expected object, got array`); on null it is
  null (a field of no record is no record); on an unknown it is unknown; a
  missing key errors. The POC gave None in all cases. Rule 3 above and the
  wet_brp guards cover the cases the scenarios hit; other persona data may hit
  more.
- The engine evaluates only the actions a requested output depends on
  (RFC-043), but it has no `requirements` gate: the POC computed outputs only
  when its `requirements` held. Constant-true outputs therefore read differently
  in negative cases (see wet_kinderopvang).

## Getrouwheid aan de wettekst

`AUDIT_GETROUWHEID.md` in deze map is de neerslag van een audit over elke wet
die deze migratie aanraakt: per wet is de `machine_readable` naast de
artikeltekst gelegd om te vinden waar de modellering om een technische reden
afwijkt (schema, engine, typechecker, scenario, datacontract, POC-erfenis) in
plaats van om een juridische. Wat zonder nieuwe taalconstructen recht te
trekken was, is rechtgetrokken; de rest staat er met reden, inclusief de
constructen die de taal nog mist. Lees dat bestand voordat je een wet uit dit
corpus als bron gebruikt: het zegt per wet wat de tekst dekt en wat niet.
