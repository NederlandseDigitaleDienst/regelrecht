# AGENTS.md

Instructions for coding agents working in this repository (Codex, Copilot,
Cursor, Claude Code and others). This file is the source; `CLAUDE.md` only
imports it, because Claude Code reads that name. Edit this file, not that one.

## Skills

Task-specific instructions live in `.claude/skills/<name>/SKILL.md`. When this
file names a skill (`roadmap`, `poc-add`, `code-reviewer`, ...), read that
`SKILL.md` before starting the task. Claude Code loads them on its own; other
agents open them as ordinary files.

They stay under `.claude/skills/` rather than the cross-tool `.agents/skills/`,
because Claude Code only discovers skills in the former and a symlink between
the two does not survive a Windows checkout.

## Where facts live

This file holds instructions for working in the repository: rules,
conventions, and what to do or avoid. Facts about the system (which components
exist, their URLs, what CI checks, how deployment and cleanup work) live in the
docs site, and this file links to the page instead of repeating it. Open the
page by its repo path; `docs/src/content/docs/operations/ci-cd.md` renders at
`https://docs.regelrecht.rijks.app/operations/ci-cd`.

When you change how the system works, update the docs page in the same pull
request. When you find a fact here that the docs also state, move it there and
leave a link: two copies drift.

## Project Overview

**regelrecht** is a platform for machine-readable Dutch law execution, as a
monorepo. The component tour and the full directory map are in
`docs/src/content/docs/guide/architecture.md`, with a page per component under
`docs/src/content/docs/components/`. Orientation:

- `packages/` - the Rust crates (engine, law-model, pipeline, harvester, admin,
  editor-api, corpus, shared, tui, poc-portal and a few more) and the JS package
  `frontend-shared`, the code the Vue frontends share
  (`docs/src/content/docs/components/frontend-shared.md`)
- `frontend/`, `frontend-lawmaking/`, `frontend-demo/` - the editor, the
  law-making visualization, and the demo (engine as WASM, no backend; see
  `docs/src/content/docs/components/demo.md`)
- `corpus/regulation/` - the law corpus in YAML; `corpus/demo/` - the demo corpus
- `bdd/` - the canonical Gherkin language (`bdd/grammar.yaml`) and the engine
  conformance suite; the two scenario buckets are explained in
  `docs/src/content/docs/guide/testing.md`
- `docs/` - the Astro site for the landing page, the docs, the RFCs and the roadmap

BDD step bindings for every engine are generated from `bdd/grammar.yaml`.
**Never hand-edit a generated file** (`grammar.generated.js`, the Rust bindings
from `packages/engine/build.rs`): change `grammar.yaml` and run
`just bdd-codegen`. A failing law-validation scenario (bucket A, next to a law)
means a law changed or the scenario is stale, and a human decides which.

## Vocabulary: regelwerk

The definition is in the glossary (`docs/src/content/docs/reference/glossary.md`,
under "RegelRecht-Specific Terms") and the reasoning in RFC-044. What follows is
how to apply it when you write.

- **A regelwerk** (English: rulework, Frisian: regelwurk) is one regulation as
  recorded in the law format, identified by its `$id`. Each dated YAML file is a
  **version** of it. Count versions when you mean files, and regelwerken when
  you mean `$id`s.

Which word a sentence needs:

- **The thing someone adds, opens, enriches, validates, saves or establishes**
  is a regelwerk. Do not write *wetsbestand*, *law file* or, for this meaning,
  *specificatie*.
- **What the legislature laid down** stays *wet*, *regeling* or *wettekst*:
  "dit komt letterlijk uit de wet", wetten.overheid.nl, "Soort regeling". A
  screen a citizen reads keeps those words too; regelwerk is a word for the
  people who make and review them.
- **The schema and the language are not a regelwerk**, and the word never
  becomes a prefix for them. A regelwerk is written in the law format and
  conforms to the schema. The prose linter in `.claude/skills/docs-writing/` rejects such
  compounds.

Replacing an older word is a judgement per occurrence, never a search and
replace: "law YAML" and "specification" mean a regelwerk in one sentence and the
language in the next. When in doubt, leave the old word. Change a document as a
whole or not at all, and leave other authors' RFCs in the words they were
written in. Where a text quotes or paraphrases the paper, keep the paper's term
in parentheses beside the new word.

Code identifiers keep `law` (`law_id`, `ArticleBasedLaw`, the API routes, the
WASM API, the Gherkin steps); werkpakket slugs keep their old words.

## Code is English

Identifiers, comments, test names and messages meant for developers are in
English. Dutch belongs in text a user reads and where this file or a skill asks
for it, such as commit subjects and PR descriptions. Dutch values in the data
(schema values, law ids, YAML keys) stay as they are, and so do Dutch legal
terms and names of concepts (`toeslagpartner`, `regelwerk`) unless a clear,
unambiguous English equivalent exists.

## Development Setup

Prerequisites are in `docs/src/content/docs/guide/getting-started.md`; the local
stack, `just dev-setup` and the pre-commit hooks are in
`docs/src/content/docs/guide/dev-environment.md`.

- Run `just dev-setup` once per machine. It shares one cargo target dir across
  all worktrees, which saves a cold build per worktree. mold is needed only on
  x86_64 Linux, where `packages/.cargo/config.toml` links with it
  (`dev_needs_mold` in `script/dev-lib.sh` decides); elsewhere cargo uses the
  default linker.
- Use the `just` recipes rather than calling `cargo` directly: they are what the
  pre-commit hooks and CI run, so a green `just` run locally is the same check.
  `just` lists them; `docs/src/content/docs/guide/testing.md` says which one to
  run when. Before pushing, run `just check` (or `just test-no-docker` for the
  tests on a machine without Docker).

### Pre-commit Hooks

Install them with `pre-commit install --hook-type pre-commit --hook-type
commit-msg`; without the `commit-msg` type the commit-message check never runs
locally. What the hooks cover is in `docs/src/content/docs/guide/dev-environment.md`.

**NEVER use `--no-verify` when committing.** Fix the underlying problem instead of bypassing hooks.

**No tool branding in commits.** Do not add "Generated with <tool>" or
"Co-Authored-By: <AI agent>" lines to commit messages.

### Commit & PR Title Conventions

PR titles are linted by `.github/workflows/pr-title.yml` (a failing title blocks
merge). The format is **Conventional Commits**: `type(scope): subject`, where
`(scope)` is optional. Commit messages should follow the same shape.

- **Allowed types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`,
  `test`, `chore`, `build`, `ci`.
- **Allowed scopes** (optional): `engine`, `admin`, `pipeline`, `harvester`,
  `editor`, `corpus`, `github`, `frontend`, `lawmaking`, `demo`, `docs`, `grafana`,
  `ci`, `schema`, `deps`, `dev`. An unlisted scope fails the lint.
- **The subject MUST start with a lowercase letter** (`subjectPattern:
  ^[a-z].*$`). This is the easiest rule to trip on: `docs: RFC-…` fails because
  "RFC" is uppercase — write `docs: verwijzingen naar RFC's …` instead. Editing
  the PR title re-runs the check; no new commit needed.

Per the global convention these subjects are written in **Dutch** (PR
descriptions too), while code identifiers stay English.

### Every pull request names its werkpakket

**Every PR body ends with a `Werkpakket:` line naming the werkpakket from the
roadmap that the work contributes to.** The check **`Werkpakket genoemd`**
(`.github/workflows/werkpakket-gate.yml`) turns red without it. It is not a
required status check, but treat it as one: write the line whenever you open a
PR; it is not optional and not something to ask about.

```
Werkpakket: referentie-casus-i
Werkpakket: referentie-casus-i, effect-over-tijd
Werkpakket: geen — losse typefout in de docs
```

- **The slug is the werkpakket's `id`**, which is also its filename and its URL.
  The full list is `ls docs/src/content/roadmap/werkpakketten/`, rendered at
  `/roadmap`. Never invent one: an unknown slug fails the check, which then
  suggests the nearest matches.
- **Several werkpakketten** on one line, comma-separated.
- **`geen` needs a reason.** `Werkpakket: geen` on its own fails. Work that
  genuinely belongs to no werkpakket says why: `geen — losse typefout in de
  docs`. Without the reason it is a box that fills itself, and then the check
  measures whether someone can paste a line rather than whether they asked the
  question.
- **Exempt**, decided from the API and not from the workflow: Dependabot PRs and
  fork PRs.

Put it on its own line at the end of the body, in trailer form. That is what
makes it greppable, survives being copied into a merge commit, and lets a later
script total up commits and PRs per werkpakket without this gate changing.

**Write the bare slug.** After the gate passes, `script/linkify-werkpakket.sh`
rewrites the line in the PR body into a markdown link to the roadmap, so the
reference is clickable where people actually read it:

```
Werkpakket: [referentie-casus-i](https://regelrecht.rijks.app/roadmap/werkpakket/referentie-casus-i)
```

Do not write that link yourself and do not paste a URL into the trailer; the bot
builds it. The gate reads both forms (it strips the markdown before matching), so
the rewrite cannot turn the next run red, and a line that is already a link is
left alone. The werkpakket page links back to the PRs carrying its slug, so the
reference works in both directions.

Commit messages are not checked and carry no trailer requirement. Note that this
repo squash-merges and the squash body is assembled from the individual commit
messages, not the PR body, so a `Werkpakket:` line only reaches `git log` if you
put it in a commit message. Do that when it is useful, not as a rule.

**When the PR touches a law from the corpus, add a `Wet:` line under it**, with
the law's `$id` (the directory name under `corpus/regulation/`):

```
Werkpakket: referentie-casus-i
Wet: wet_op_de_zorgtoeslag
```

This line is optional, because most PRs touch no law and requiring it would
produce the same empty box as a reasonless `geen`. Present, it has to resolve:
the gate rejects an id that is not in the corpus, and renders each one as a link
to the law on wetten.overheid.nl in the check's summary. The URL comes from the
rulework's own `url` (falling back to `bwb_id`), so it cannot drift from the
corpus. Do not write the link yourself, and never invent a BWB number: name the
`$id` and let the gate resolve it.

Which werkpakket a change belongs to is a judgement, so make it deliberately:
match the work to the roadmap rather than reaching for the nearest-sounding
slug. If nothing fits, `geen` with an honest reason is the correct answer, not a
failure. The logic lives in `script/require-werkpakket.sh`, with
`script/require-werkpakket.test.sh` next to it (a `gh` stub) covering every path
that decides green or red; both run as a pre-commit hook. Content changes to the
roadmap itself go through the `roadmap` skill.

### Test Data

**Never use real secret or private information in tests.** This is a public
repository — anything in a test fixture is published. Do not put names of
private repositories, internal hostnames, credentials, tokens, real BSNs/personal
data, or any reference to a private working environment into test fixtures,
assertions, comments, or sample data. Use clearly-fictional placeholders instead
(e.g. `example-org/regelrecht-corpus-example`). When a test needs to model a
private/traject-owned source, anonymize the identifiers — the test should prove
the behavior, not leak where the real data lives.

### Git Worktrees

When using git worktrees, create them **inside the project folder** (e.g., `.worktrees/`).

```bash
git worktree add .worktrees/feature-branch feature-branch
```

## Architecture Notes

The law format is in `docs/src/content/docs/concepts/law-format.md`, cross-law
references in `docs/src/content/docs/concepts/cross-law-references.md`, and
delegation (`open_terms` / `implements`) in
`docs/src/content/docs/concepts/inversion-of-control.md`. A working example of
both: `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`.

- The hand-authored `schema/latest/schema.json` is the **canonical contract**.
  The Rust `law-model` (`packages/law-model/`, described in
  `docs/src/content/docs/components/law-model.md`) must conform to it; neither
  is generated from the other. After changing either one, run `just conformance`
  (see `docs/src/content/docs/reference/conformance.md`). Do not "fix" a
  divergence listed in `KNOWN_GAPS` in passing; reconciling those is tracked
  separately.
- A mistake in a released schema version, even in a `description`, is fixed
  with a patch release: a new `schema/vX.Y.Z` directory, wired up like the
  previous release. Never add an overlay, correction table or build-time
  substitution that makes the docs or any other consumer say something
  different from the released `schema.json`. Validators and other
  implementations download the schema itself, so a correction layer hides the
  error instead of fixing it (PR #1579 replaced such an overlay with v0.7.1).
  The same holds more broadly: when a source of truth is wrong, fix the
  source, not a layer on top of it.

## De demo is meertalig

`frontend-demo/` draait in het Nederlands, het Engels en het Fries.
**Nederlands is de bron**: elke tekst staat eerst in `src/i18n/nl.js`, en de
andere woordenboeken zijn er de vertaling van. Bewerk ze in dezelfde wijziging;
een sleutel die maar in één bestand landt is een bug, geen halve klus.

- **De talentabel in `src/i18n/index.js` is de enige bron.** Daar staat per taal
  zijn prefix, zijn `Intl`-tag, zijn naam in de eigen taal en zijn woordenboek.
  Een taal toevoegen is een regel in die tabel plus een pad per pagina in
  `router.ts`; zet nooit een taalcode los in een `===` of een objectsleutel,
  want dat is precies wat een vierde taal stil Nederlands laat worden.
- **Het Fries is vertaald maar nog niet nagekeken.** De vertaling komt van
  taalmodellen, niet van een vertaler. Wat een Friestalige revisor moet
  nakijken staat in `corpus/demo/i18n/REVIEW-fy.md`: de plekken waar de
  vertalers zelf zeiden dat ze het niet zeker wisten, met per geval wat er vast
  staat en wat niet. Vul die lijst aan wanneer je op zo'n plek stuit, in plaats
  van de twijfel in een commit-bericht te laten zitten.
  `MAX_IDENTICAL_SHARE` in `i18n.test.ts` bewaakt dat het aandeel sleutels dat
  nog letterlijk het Nederlands is daalt en niet stijgt; dat getal gaat met de
  hand omlaag, met de reden in de commit.
- **Een Fries woord houdt zijn diakriet, ook vooraan een zin.** `Ôfwiisd`, niet
  `Ofwiisd`. Zo'n fout haalt élke andere controle, want de tekst verschilt nog
  steeds van het Nederlands, en hij is toch fout. Alle vier de vertalers
  maakten hem. `frisian.test.ts` vangt hem nu; breid de stammenlijst daar uit
  als er een woord bijkomt.
- **Paden bevatten geen teken dat gecodeerd moet worden.** Slugs worden vertaald
  (`/en/ruleworks`, `/fy/senarios`), maar een apostrof wordt `%27` en dat is het
  adres dat tijdens een presentatie op het scherm komt. Het Nederlands doet het
  al zo: het tabblad heet "Scenario's" en het pad is `/scenarios`.
  `router.test.ts` weigert een pad met zo'n teken.
- **Gherkin-stappen zijn de uitzondering op "een taal is een tabelregel".** De
  sjablonen in `gherkinNl.ts` zijn geschreven zinnen, geen tabelwaarde. Een taal
  zonder sjablonen toont de canonieke Engelse stap, mét Engelse sleutelwoorden:
  `Functionaliteit` boven een Engelse stap zou een taal suggereren die er niet
  is.

- **Elke zichtbare tekst gaat door `t()`.** Geen `locale === 'en' ? … : …` in
  een component: dat haalt de tekst buiten bereik van elke controle, en het is
  precies hoe de landingspagina zijn strings over drie plekken verspreid kreeg.
  `scripts/check-i18n.mjs` weigert zo'n vertakking, tenzij er een
  `i18n-ok:`-commentaar boven staat dat uitlegt waarom (het enige geldige geval
  nu: terugvallen op corpusinhoud die nog geen vertaling heeft).
- **Na het wijzigen van een Nederlandse tekst: hervertaal en draai
  `node scripts/i18n-bless.mjs`.** `en.sources.js` houdt per sleutel de
  vingerafdruk bij van het Nederlands waaruit vertaald is, zodat een vertaling
  die is blijven staan terwijl het origineel veranderde, opvalt. Zonder die
  stap faalt `i18n.test.ts`, mét de sleutelnamen en het commando erbij.
- **Adressen**: Nederlandse paden blijven zoals ze zijn, Engels staat onder
  `/en/` met vertaalde slugs (`/regelwerken` ↔ `/en/ruleworks`). De tabel staat in
  `src/router.ts`; een pagina spreek je aan op naam (`localeRouteName`), nooit
  op een letterlijk pad, anders belandt een Engelse bezoeker op een Nederlands
  tabblad.
- **Wetteksten worden niet vertaald.** `article.text` is de geldende wettekst;
  een vertaling daarvan heeft geen rechtskracht en is de wet niet. Die blijft
  Nederlands, in elke taal, met `lang="nl"` op het paneel eromheen. Dat is geen
  opmaakdetail, want een schermlezer kiest daarop zijn stem. In elke andere taal
  dan het Nederlands staat er een banner boven (`wet.dutch_only.*`) die uitlegt
  waaróm, met een link naar de bekendgemaakte tekst op wetten.overheid.nl. Dat
  die uitleg er staat is het punt: een onvertaalde tekst zonder verklaring leest
  als werk dat niet af is, en dit is juist een keuze.

  Die banner zegt dat de wetten in deze demo in het Nederlands zijn
  bekendgemaakt, en niet dat een Nederlandse wet alleen in het Nederlands
  bestaat. Dat laatste stond er eerst en is onwaar: Fries is op grond van de Wet
  gebruik Friese taal een officiële taal in Fryslân, en er zijn regelingen met
  een authentieke Friese tekst. Die staan alleen niet in dit corpus. Zeg wat van
  déze teksten waar is.
- **Eigennamen blijven staan.** `Belastingdienst` is de naam van een orgaan,
  geen omschrijving; wie op "Tax Administration" zoekt vindt niets. Een
  Engelse toelichting tussen haakjes mag, vertalen niet.
- **Opmaak loopt via `intlLocale()` in `src/data/format.ts`**, nooit via een
  hardgecodeerde taalcode. Een `Intl`-formatter legt zijn taal vast op het
  moment dat je hem maakt, dus eentje op moduleniveau blijft na een taalwissel
  in de oude taal formatteren.
- **Engels is `en-GB`, niet `en-US`.** Dat geeft "22 September 2026" en een
  24-uurs klok, zoals een Nederlands overheidsscherm een datum en een tijd
  noteert, en het houdt de leesvolgorde van het Nederlandse origineel aan.
  `en-US` zou er "September 22, 2026" en "12:00 AM" van maken. De
  `docs-writing`-skill vraagt Amerikaans Engels voor *proza*; datumvolgorde is
  een andere vraag en dit is het antwoord daarop. Het bedrag blijft in beide
  talen in euro's; alleen de scheidingstekens en de plaats van het symbool
  verschillen.

## Frontend code

Frontends are Vue applications that follow Vue's own conventions: Vite to build,
Vitest to test, single-file components, and composables for shared state. The
aim is TypeScript for code that runs in the browser (`.ts`,
`<script setup lang="ts">`). The editor, lawmaking, the demo and
`frontend-shared` are there; the PoC frontends are still JavaScript and will be
migrated later, so keep new code in those in JavaScript until then. Scripts
that run in plain Node, the modules they load, and generated files stay
JavaScript. The setup is in `docs/src/content/docs/guide/frontend-code.md`.

- Import a TypeScript module without its extension (`'./values'`). Plain Node
  cannot resolve that, so a module that Node loads keeps its `.js`.
- Prefer `unknown` and narrowing over `any`. An `any` or a non-null `!` gets a
  comment saying why it is safe.
- After moving or renaming a module, check the `vi.mock` paths that name it. A
  stale path stops mocking without an error.

## Frontend / UI Components

**All user interface MUST be built with components from the NLDD Designsysteem: https://github.com/NederlandseDigitaleDienst/design-system** (the NLDD `nldd-*` web components, from `@nldd/design-system`). Do not hand-roll custom UI elements when a design-system component exists. For the required component hierarchy, nesting rules, and layout patterns, use the `storybook-component-hierarchy` skill.

The element prefix is `nldd-`, with two l's. Older prose may still write
`ndd-`; that is stale.
Check an attribute against the package's own `.d.ts` before using it — a web
component with an attribute it does not know renders nothing and reports
nothing, so a guessed attribute fails silently and only in the browser.

### Icon names

When an icon has an alias, prefer the **alias name** over the canonical name — e.g. `icon="harvest"` not `wheat`, `icon="info"` not `info-circle`, `icon="new-account"` not `person-circle-badge-plus`. Aliases are more meaningful and stable; the alias list lives in the design system at `src/components/content/icon/icon-aliases.js`.

### When you can't build what you want with these components

Do **not** silently improvise. Follow these steps in order:

1. **Reconsider the design choice.** Investigate whether the design should be different. Try to conform to existing choices already made elsewhere in regelrecht (look at how other views/components in `frontend/` and `frontend-lawmaking/` solved similar problems) before introducing anything new.
2. **If you still can't proceed: stop and ask.** Tell the user explicitly that you need to take a shortcut, describe what's missing, and ask for permission. The user can then say whether it should be done differently or whether they will request a new feature/component from the design system. Do not take the shortcut without approval.

### Reporting additional CSS

If you needed **any additional CSS styling** on top of the design-system components (overrides, custom spacing, layout hacks, etc.), you **must report this explicitly** to the user — list exactly what custom CSS you added and why. Custom styling on top of the design system is a signal that may need a design-system change, so it must never be hidden.

## Proof-of-concepts

`poc.regelrecht.rijks.app` is één ZAD-component (`poc`) met de pocs erachter,
elk achter een eigen wachtwoord, gerouteerd binnen `packages/poc-portal`. Hoe
het portaal werkt en waarom het één component is:
`docs/src/content/docs/components/poc-portal.md`.

```
pocs/registry.yaml          het register — de enige bron
corpus-poc/<slug>/          casus-regelgeving, data, varianten
frontend-poc-<slug>/        de app
```

Een poc waarvan de kern een Rust-crate is die als WASM in de browser draait,
staat in één map: `packages/poc-<slug>/` met de app in `frontend/` ernaast
(zo chronolexografie). Dat is een toegestane indeling naast de gewone, geen
vervanging ervan.

Een poc toevoegen of zijn status wijzigen: gebruik de **`poc-add`**-skill. Die
kent de subpad-aanpassingen die een losse app nodig heeft en de plekken in de
deploy die met de hand bij moeten.

Drie dingen om te weten voordat je hier iets aanraakt:

- **`corpus-poc/` is geen geldend recht.** Het zijn casus-corpora en
  reconstructies van wetsvoorstellen, met een eigen schrijfstijl uit de losse
  PoC-repo's. Ze gaan niet door `just validate`, staan buiten yamllint, en de
  harvester heeft er niets mee te maken. Behandel ze niet als het corpus.
- **Elke poc draagt een `status` en een `voorbehoud`**, allebei verplicht en
  zonder default. Ze staan op drie plekken (kaart, inlogscherm, en een strook
  binnen de poc zelf), omdat die drie verschillende mensen bereiken: een
  doorgestuurde diepe link slaat het overzicht over, en een screenshot uit een
  poc reist zonder omringende tekst. Een uitgerekend bedrag ziet er even
  stellig uit, of het model nu doorgelopen is met juristen of niet.
- **Wachtwoorden staan alleen in ZAD**, nooit in de repo. Anne zet
  `POC_PW_<SLUG>` als user env var op component `poc`, in de ZAD Operations
  Manager; zad-cli heeft geen `env`-commando. Het portaal weigert te starten
  als er een wachtwoord ontbreekt; een poc in het register zonder wachtwoord
  zou anders voor iedereen open staan. Een preview kopieert de env vars van
  productie bij het aanmaken, dus een preview van vóór het wachtwoord faalt
  met `NoPassword`.

De beleidsassistent in de OCW-pocs draait op de Claude Code CLI met Anne's
eigen abonnementstoken (`CLAUDE_CODE_OAUTH_TOKEN`, uit `claude setup-token`).
De tool-sandbox is smal (alleen de regelrecht-MCP-tools, geen bestandssysteem),
maar wie het wachtwoord heeft kan het abonnement laten werken. Dat is een
bewuste afweging, geen omissie.

## Published papers are frozen

`docs/src/research/` holds published work: one generated `.html` per paper
with its companions, each produced by a converter next to the paper's LaTeX
source (the page under `docs/src/pages/research/` names which). **Never edit these to match the current state of the
code.** A paper is a claim someone made at a moment in time, and the record of
what was true then is the whole point of citing it. A paper that silently tracks
the codebase cannot be cited at all.

This comes up because the code moves past the paper. RFC-016 added collection
operations to an engine the paper describes as having none ("There is no
aggregation over collections: no `SUM`, no `COUNT`, no iteration"). That
sentence stays. It was accurate when written, and the reader who follows a
citation to it needs to find what the author wrote, not a retrofit.

If the divergence matters, say so somewhere that is not the paper: the RFC that
supersedes it, a docs page, or release notes. If a paper genuinely needs to
change, that is an erratum or a new version, and it is the author's call, not a
side effect of a code change.

## RFC Process

This project uses an RFC process for design decisions.

- **Location**: `docs/src/content/rfcs/`
- **Process document**: See `docs/src/content/rfcs/rfc-000.md`
- **Template**: Use `docs/src/content/rfcs/template.md`

### When to Write an RFC

Write an RFC for:
- Law representation format changes
- Execution engine architecture changes
- Cross-cutting design patterns
- Integration patterns between components

### RFC Metadata (frontmatter)

RFC metadata lives in YAML **frontmatter**, not a bold-labelled body preamble.
The fields are `title`, `status`, `implementation`, `topic`, `date`, `authors`,
optional `depends_on`, and optional `short_title`. `topic` files the RFC under a
group on the RFC index; the allowed values live in `docs/src/lib/rfc-topics.ts`.
The docs site
(`docs/src/pages/rfcs/`, parsed by `docs/src/lib/rfcs.ts`) renders `status` and
`implementation` as NDD tags and the rest as a header line — there is no rehype
preamble plugin.

Two orthogonal fields, both required on every RFC so an absent tag never reads
as "unknown":

- **`status`** — lifecycle only: `Draft | Proposed | Accepted | Rejected | Superseded`.
  A built-and-merged RFC is `Accepted`, not `Draft`; "Draft" means the design
  itself is unsettled.
  `Reserved` is the one status outside that lifecycle: a placeholder holding a
  number an open pull request has claimed. It needs `reserved_by` (the PR URL) and
  carries no `implementation`; see the comment in `docs/src/content.config.ts`.
- **`implementation`** — build state: `Implemented | Partially implemented | Not implemented`.
  Independent of `status` (code can land ahead of acceptance). Ground the value
  in the actual codebase, not the RFC's aspirations.

### An accepted RFC is not rewritten

Once an RFC is accepted it records what was decided then, so a change of design
gets a **new RFC** and the old one goes to `status: Superseded` with a line
pointing at its replacement. The text underneath stays as it was written. This
is the same reason the published papers are frozen: a document that quietly
tracks the code cannot be cited, and a reader who follows a reference has to
find what the author wrote.

The frontmatter is not the design. `status` moving to `Superseded` or
`Rejected`, and `implementation` tracking what is built, are records *about* the
document and are expected to change; that is what those fields are for. What
stays put is the body: the claim the author made, apart from the factual
corrections described below.

What else needs no supersede: updating a reference when another document is
renamed, and *adding* a note about what a later RFC did with the old decision.
An RFC that is amended on one point, rather than replaced, keeps its status and
gains a pointer; its own text stays as it was.

What does need one: replacing the title, the central concept, or the field
definitions. The case that produced this rule: schema v0.7.0 renames the channel
RFC-012 describes from `untranslatables` to `markings`, and the first attempt
rewrote RFC-012 to match. That would have made every existing citation to it
point at a document about a different field, while a version on schema v0.5.x
still carries `untranslatables` and the engine still reads it. The RFC keeps its
text and goes to `Superseded` instead, and the RFC introducing the new channel
carries the new design.

A factual error in the body may be corrected in place: a wrong article or lid,
a wrong body or organization, an example that relies on a regulation that had
already lapsed when the RFC was accepted, a citation that points at the wrong
source, an example filed under the wrong category because the facts about it
were wrong. The rule protects the decision, and a wrong article number is not a
decision the author made. A fact that was true when the RFC was accepted and
changed later (a regulation that lapsed since, a deadline that moved) is not an
error; it gets a note, not a correction. Two conditions. The correction names
its source: the statute, or the review that found it, as a link to the pull
request or issue. And every corrected RFC ends with a `## Corrections` section
that lists each one: the date, what changed, and where the correction came
from, so a reader who cited the old text can see what moved. A correction that
changes what a concept means, what a category contains, or what the design does
is not a factual correction; it gets an issue or a new RFC. When anyone disputes
that a change is factual, treat it as a design change.

Questions from a linked review (a pull request or issue) may be appended to an
accepted RFC as a clearly labeled section of open questions: like a note, they
change no decision. A reviewer whose text substantially lands in the body may
be added to `authors`. Open questions and small corrections credit the reviewer
in their own section instead, because `authors` reads as endorsement of the
design and a reviewer who questions it should not be listed as its author.

## Code Reviews

After completing significant code changes, review them with the `code-reviewer`
skill (`.claude/skills/code-reviewer/SKILL.md`) before committing.

Run that review in a fresh subagent when your tool has one (in Claude Code: the
Agent tool with `subagent_type: "general-purpose"`), so the review does not
inherit the reasoning that produced the change.

## CI/CD and Deployment

What CI checks, which checks are required, and how the merge gates and the
merge queue work: `docs/src/content/docs/operations/ci-cd.md`. Which components
are deployed where, previews, cleanup, ZAD CLI and secrets:
`docs/src/content/docs/operations/deployment.md`. The instructions below are
what changes how you act.

### The Claude review merge gate

**Claude review completed** is a required check. It passes only once the
automated review has run to completion for this commit and left no finding
marked `🔴 **Critical**`. How it establishes that, and what it cannot protect
against, is in `docs/src/content/docs/operations/ci-cd.md#the-claude-review-gate`.

- **There is no override.** Fix the Critical finding and push again. Do not add
  an escape hatch (a label, a magic comment, an environment variable): an agent
  operates one as easily as a person, while it reads as a human judgement.
- **Wait on the job's `status`, never on the number of review comments.** Zero
  comments means "still running" as often as "nothing to report".
- **No review on the head commit? Start one by hand** with
  `gh workflow run "Claude Code Review" -f pr=<n>`, then rerun the failed gate
  (`gh run rerun <run id> --failed`); the gate takes that review over. Rerunning
  an old `pull_request` run does not help, because it keeps the commit it was
  started for. Toggling the PR to draft and back to ready also works.
- **A PR that edits `.github/workflows/claude-code-review.yml` gets no automatic
  review**: the action skips itself when the file differs from the default
  branch, and the gate then blocks. Such a PR needs a human review and an admin
  merge.
- **Do not rename the step "Record that the review ran"** without changing
  `PROOF_STEP` in `script/await-claude-review.sh`, and do not change
  `CRITICAL_MARKER` without changing the review prompt. The test suites bind
  both pairs; a mismatch makes the gate read past every finding.
- Every comment the review writes ends with `<!-- claude-review -->`; snapshot
  and cleanup select on that line, not on the author.
- A red outcome must state what was established and name no cause it has not
  tested ([issue #1178](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/1178)).
- The logic lives in `script/await-claude-review.sh` and
  `script/claude-review-comments.sh`; change a path that decides green or red
  only together with its test suite next to it.

### Security updates

A Dependabot security update skips the five-day cooldown and turns
**Security update approved** green only once an engineer with write access has
approved it, on the commit being merged; after a rebase or push the approval no
longer counts. That check is not a required status check, so nothing technically
stops a merge: do not merge a security update without that approval.
`claude-dependabot.yml` does not merge security updates either. Details:
`docs/src/content/docs/operations/ci-cd.md#the-security-update-gate`.

The Mattermost notification in that workflow reads its webhook from the
**Dependabot** secret store, not the Actions one. The logic is in
`script/require-security-approval.sh` with its test suite.

### The merge queue

`main` merges through a merge queue. Put a PR in it with:

```bash
gh pr merge <nr> -R NederlandseDigitaleDienst/regelrecht --squash
```

"The merge strategy for main is set by the merge queue" is a notice, not an
error. Do not pass `--delete-branch`; the repository deletes the branch itself.
The GraphQL queries and the `gh run list` command below are in
`docs/src/content/docs/operations/ci-cd.md#the-merge-queue`.

- **A successful command does not mean the PR is queued.** Check
  `mergeQueueEntry`.
- **A PR that drops out of the queue stays `OPEN`.** Never wait for the state to
  leave `OPEN`; follow `mergeQueueEntry` (`MERGED` is done, `OPEN` with
  `mergeQueueEntry: null` is a drop) and read the reason from the
  `REMOVED_FROM_MERGE_QUEUE_EVENT` timeline item.
- **A failure in the queue does not show on the PR.** Find the red run with
  `gh run list --event merge_group`.
- **Flaky test in the queue**: when a test the PR does not touch fails in the
  queue while the PR's run and the latest `main` run are green, queue it once
  more. If it drops a second time on the same failure, find the cause instead of
  queueing it again, and open an issue for the flaky test.
- **When the required checks in the branch protection change, update
  `REQUIRED_CHECKS` in `script/merge-queue-checks.test.mjs` in the same go.**
  Nothing reminds you: the protection changes with a button, not a commit.
- **A required job may skip in the queue (`if:`), but its workflow must run on
  `merge_group`.** A workflow that does not run leaves the check on "Expected"
  and hangs every entry.
- Keep "Require branches to be up to date before merging" off.
- `enforce_admins` is on, so a stuck entry cannot be bypassed: take it out of
  the queue, merge, switch the queue back on.

### Previews

A PR builds and deploys nothing unless it carries the **`deploy:preview`**
label. Add it only when someone needs a running preview; removing it (or
closing the PR) deletes the preview. `deploy:<component>` labels force a
component to build and do nothing without `deploy:preview`. Fork PRs never
build.

### Cleanup

All image cleanup beyond a PR's own `pr-N` tags runs through
`script/ghcr-cleanup.mjs` from `scheduled-cleanup.yml`. Extend that script; do
not add a second cleaner next to it, because two cleaners do not know each
other's exceptions and production runs on a `sha-` tag. What it protects and
why: `docs/src/content/docs/operations/deployment.md#cleanup`.

### Debugging deploy-preview failures

A timeout on the wait ("Timed out after 900s waiting for the task") only means
ZAD was slower than the window; the deployment carries on. An error with a
status or an exception is what needs diagnosing: `zad logs <deployment>` (e.g.
`zad logs pr429`) and look for `ERROR` lines. `Could not extract URL from
result` with `"status": "superseded"` is harmless; re-run only that job. The
full procedure, including resetting a broken preview database, is in
`docs/src/content/docs/operations/deployment.md#debugging-a-failed-preview-deploy`.

### ZAD CLI

Use [`zad-cli`](https://github.com/RijksICTGilde/zad-cli) with `ZAD_API_KEY`
and `ZAD_PROJECT_ID` in `.env`. Install and common commands:
`docs/src/content/docs/operations/deployment.md#zad-cli`.
