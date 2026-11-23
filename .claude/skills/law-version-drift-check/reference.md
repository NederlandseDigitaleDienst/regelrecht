# Law Version Drift Check — Reference

Operational reference for `law-version-drift-check`. Patterns, algorithms,
prompt templates, output formats. Casus-agnostisch.

## 1. URL patterns

### Primary: wetten.overheid.nl

Per-version base URL:
```
https://wetten.overheid.nl/<BWB-id>/<YYYY-MM-DD>
```

Per-article anchor:
```
https://wetten.overheid.nl/<BWB-id>/<YYYY-MM-DD>#Artikel<N>
```

`<YYYY-MM-DD>` is the YAML's `valid_from`. Always supply the date explicitly —
omitting it returns "current" and silently hides drift in older YAMLs.

Information page (version history, change list):
```
https://wetten.overheid.nl/<BWB-id>/<YYYY-MM-DD>/0/informatie
```

### Secondary: Staatsblad / Staatscourant (officielebekendmakingen.nl)

Publication of record:
```
https://zoek.officielebekendmakingen.nl/stb-<YYYY>-<NR>.html
https://zoek.officielebekendmakingen.nl/stb-<YYYY>-<NR>.pdf
https://zoek.officielebekendmakingen.nl/stcrt-<YYYY>-<NR>.html
```

Use the HTML form first; PDFs may need local extraction (see §4 fallback).

## 2. The single allowed normalization

Pseudocode for the intra-paragraph whitespace normalization. This is the **only**
normalization permitted before diffing.

```
normalize(text):
    paragraphs = split_on_blank_lines(text)
    for each paragraph:
        replace every run of whitespace (spaces, tabs, single line breaks)
        with one space; trim leading/trailing whitespace.
    rejoin paragraphs with a single blank line between them.
```

A paragraph boundary is a blank line in the rendered source. Paragraph
boundaries between leden, between onderdelen, and between citation blocks are
preserved exactly. Everything inside a paragraph — accents, quote glyphs,
punctuation, capitalization — is preserved exactly.

Apply the same normalization to both sides before diffing. Never apply it to
only one side.

## 3. Prompt templates

### 3.1 Structural inventory prompt

Use exactly this form (substitute `<N>` and the URL):

> Visit `<URL>`. Return a structured skeleton of artikel `<N>`:
>
> - aantal leden
> - voor elk lid: de geordende lijst van onderdeel-letters (a, b, c, …) en de
>   laatste letter die in dit lid voorkomt
> - of artikel `<N>` eindigt na een onderdeel of na een doorlopende alinea
>
> Do not paraphrase. Do not summarise. Do not omit the last onderdeel. If you
> are uncertain about the last onderdeel, say so explicitly — do not guess.

Run this prompt **twice with different surface phrasings**. Agreement → accept.
Disagreement → escalate to Staatsblad-fallback for that article.

### 3.2 Verbatim text prompt

Use exactly this form:

> Visit `<URL>`. Return the literal full text of artikel `<N>` at this version,
> lid by lid and onderdeel by onderdeel, verbatim.
>
> - Begin each lid with its number followed by a period.
> - Begin each onderdeel with its letter followed by a period.
> - State the letter of the final onderdeel explicitly at the end.
> - Preserve all punctuation, all accents, all parenthetical reading notes
>   (e.g., bracketed editorial annotations within the text).
> - Do not summarise. Do not paraphrase. Do not "clean up" obvious errors.

### 3.3 Anchor / competent_authority verification prompt

> At `<URL>`, what is the formal name of the orgaan that holds the function
> referenced as `<role>` in artikel `<N>`? Return the name verbatim as it
> appears in the geldende text on the indicated date.

## 4. Fetch reliability ladder

In order of precedence:

1. WebFetch on `wetten.overheid.nl/<bwb>/<date>` with the structural prompt.
2. Second WebFetch with rephrased structural prompt; require agreement with (1).
3. WebFetch on the same URL with the verbatim prompt (only after structural pass).
4. Fall back to Staatsblad (`zoek.officielebekendmakingen.nl/stb-...`) — HTML form.
5. Local PDF extraction (`pdftotext` on the cached Stb. PDF) as final fallback.

Mark each verification with its source mode in the report.

## 5. Verdict assignment

Given the two axes (structural, textual), assign verdict per article:

```
if structural_diff is not empty:
    verdict = DRIFT-structureel
    detail  = enumerate missing/extra elements by name
elif textual_diff is not empty:
    verdict = DRIFT-tekst
    detail  = word-level unified diff per paragraph
elif neither could be fetched after the ladder:
    verdict = NIET-VERIFIEERBAAR
    detail  = failure reason + suggested retry
else:
    verdict = CLEAN
```

Diacritics differences contribute to `textual_diff` and therefore to
DRIFT-tekst. They are not exempt.

A DRIFT-tekst entry whose **only** differences are diacritics is additionally
counted in the diacritics-aggregate sub-table; its verdict remains DRIFT-tekst.

## 6. Calibration ijkpunten

The skill requires a corpus-local calibration file:
```
<corpus-root>/docs/drift-ijkpunten.md
```

Format (per ijkpunt):
```
- file: <relative path to YAML>
  article: <number>
  axis: structural | textual
  expected: <one-line description of the known drift>
```

Before producing a drift-report, the skill independently re-finds each ijkpunt.
Recall < 80% → run is invalid; produce no drift-report; report the calibration
failure.

The ijkpunten file lives in the corpus, not in this skill, so the skill stays
casus-agnostisch. Curate ijkpunten that exercise different failure modes:
at least one DRIFT-structureel, one DRIFT-tekst, one diacritics-only.

## 7. Output format

### 7.1 Drift-rapport (one file)

Markdown, written to `<corpus-root>/docs/drift-reports/<file-stem>-<run-date>.md`.

```markdown
# Drift-rapport — <relative-path-to-yaml>

**Run-datum:** <YYYY-MM-DD HH:MM>
**bwb_id:** <id>
**valid_from (YAML):** <YYYY-MM-DD>
**Calibratie:** ✅ (<recalled>/<total> ijkpunten) | ❌ <reason>

## Verdict per artikel

| Artikel | Verdict | Bron-modus | Detail |
|---|---|---|---|
| 1 | CLEAN | primair | — |
| ... | ... | ... | ... |

## Aggregaat
- CLEAN: <n>
- DRIFT-tekst: <n>
- DRIFT-structureel: <n>
- NIET-VERIFIEERBAAR (bevroren): <n>

## Diakritiek-aggregaat (planning)
Aantal artikelen waar diakritiek de enige driftbron is: <n>

## Bevroren artikelen (scope-restrictie)
- artikel <N> — <reason> — retry <suggested>

## Adjacent fields
- competent_authority: <verdict per name>
- url anchors: <verdict per article>
```

### 7.2 Per-finding tracker entry

Append to `<corpus-root>/docs/issues/drift-resolutie-tracker.md` (or equivalent
location per stelselanalyse convention):

```markdown
| ID | File | Artikel | Verdict | Bron | Detail | Status |
|---|---|---|---|---|---|---|
| D-<seq> | <path> | <N> | DRIFT-tekst | primair | lid 2: "<a>" → "<b>" | drift-gerapporteerd |
```

Status vocabulary: `drift-gerapporteerd`, `drift-in-fix`, `drift-opgelost`,
`drift-bevroren` (scope-restricted).

### 7.3 Sub-report: diacritics aggregate

A separate table in the same drift-rapport, listing the articles whose only
deviation is diacritics. Used for planning a single corpus-wide reconciliation
PR, not for excusing individual findings.

## 8. Frozen-article semantics (scope-restrictie)

A NIET-VERIFIEERBAAR article is frozen for the duration of the current
micro-cycle. Concretely, the cycle's worker (Edit/Write tooling) must refuse:

- any modification of the frozen article's `text:` block,
- any modification of its `url:` anchor,
- any modification of `competent_authority` values within the article's scope.

The cycle may still touch other articles in the same file.

The freeze lifts when the next drift-pass either gives the article a definitive
verdict (CLEAN / DRIFT-*) or the cycle is closed (a new cycle starts with its
own drift-pass).

## 9. Pitfalls and forbidden shortcuts

- **Open prompts.** "Summarise artikel N" produces text that *looks* complete
  while losing the final onderdeel. Always force enumeration.
- **Trusting a single fetch.** Two independent fetches with different phrasings;
  disagreement → Staatsblad-fallback.
- **One-sided normalization.** Never normalise only the geldende text and not
  the YAML, or vice versa.
- **Interpreting errata.** A bracketed editorial note in the geldende text
  (e.g., a reading correction) belongs in the YAML verbatim. It is a
  wetgevings-fout if reported separately to the wetgever; the YAML still
  mirrors it.
- **Skipping calibration.** A drift-report without passed calibration is not
  a drift-report.
- **Bypassing on load.** Flaky network → scope-restrictie. There is no
  "skip" mode.

## 10. Hand-off conventions

- The skill writes its output to the corpus, never into itself.
- The skill never edits regulation YAMLs. It produces reports and tracker
  entries; the fix lands via the corpus's modellering-fixes-plan workflow
  (see `regelrecht-stelselanalyse`).
- The skill never pushes. Commits and PRs follow the corpus repo's protocol;
  pushes require explicit user permission.
