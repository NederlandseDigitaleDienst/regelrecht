---
title: "Harvester"
description: "Downloads Dutch legislation from BWB and CVDR and converts it into the corpus YAML format."
---

The harvester downloads Dutch legislation and converts it to the RegelRecht YAML format. It handles two sources: national law from the BWB (Basiswettenbestand / wetten.nl) and decentralized regulations from the CVDR (Centrale Voorziening Decentrale Regelgeving). The CLI picks the right source from the identifier, so `BWBR…` IDs are fetched from BWB and `CVDR…` IDs from the CVDR.

## Overview

- **Language**: Rust
- **Location**: `packages/harvester/`
- **Sources**: BWB / wetten.nl (national law) and CVDR (local/decentralized regulations)
- **Output**: ruleworks that hold only the legal text (no `machine_readable` yet)

## How it works

```mermaid
flowchart LR
    A[BWB Repository] -->|1. WTI metadata| B[Harvester]
    A -->|2. Manifest XML| B
    A -->|3. Content XML| B
    B -->|4. Parse & Split| C[Articles]
    C -->|5. Generate| D[Rulework]
    D --> E[Corpus Juris]
```

### Pipeline Steps

1. **Validate input** - BWB ID format (`BWBR` + 7 digits) and date
2. **Download WTI metadata** - title, regulatory layer, publication date
3. **Resolve consolidation date** - from manifest.xml, find version valid for target date
4. **Download content XML** - the consolidated law text (with size limit check)
5. **Parse elements** - via extensible registry of element handlers
6. **Split articles** - hierarchical splitting into artikel → lid → lijst → li with dot-notation numbering (e.g., `1`, `1.1`, `1.1.a`)
   and record each article's `placement`: the containers around it
7. **Normalize text** - fix spacing, wrap at 115 chars (the slug in the output path is ASCII-folded through Unicode NFKD; article text keeps its accents)
8. **Generate YAML** - schema-compliant output with yamllint compliance
9. **Atomic write** - temp file → sync → rename

### Element Processing

The harvester uses an extensible **registry system** for XML element handling:

| Handler Type | Elements | Behavior |
|-------------|----------|----------|
| **Inline** | `nadruk`, `extref`, `intref`, `al` | Pass through with text |
| **Structural** | `lid`, `lidnr`, `lijst`, `li` | Manage numbering, recurse |
| **Skip** | `meta-data`, `jci`, `kop`, `brondata` | Excluded from output |
| **Passthrough** | `sup`, `sub` | Extract without special handling |
| **Marker** | `plaatje`, `illustratie`, `formule` | Text where there is any, otherwise `[formule niet in tekst beschikbaar]` |

The marker matters more than it looks. The BWB XML states some norms as a
picture: artikel 22a Participatiewet puts the whole kostendelersnorm formula in
an `<illustratie>`. Those elements used to be skipped, so the sentence that
introduces the formula ended on its colon and the norm vanished with nothing to
show it had. An incomplete article then read as a complete one. The harvester
still cannot render an image, but it no longer drops it in silence, and a
`<formule>` carrying a readable fallback keeps that text.

### Dutch Law Hierarchy

Articles are split following the legal text structure:

```
artikel (number from kop/nr)
├── lid (number from lidnr)
│   ├── al (text container)
│   └── lijst
│       └── li (number from li.nr, e.g., "a", "1°")
└── lijst (can appear directly)
    └── li
```

Produces dot-notation: `1`, `1.1`, `1.1.a`, `1.1.a.1°`

### Placement

Above the article sit the containers that organize the law: `boek`, `deel`,
`hoofdstuk`, `titeldeel`, `afdeling` and `paragraaf`. The harvester walks up
from each `artikel` and writes the ones it finds to the article's `placement`,
with the number from `kop/nr` and the opschrift from `kop/titel` as `heading`.
Inline markup in an opschrift keeps its text; a footnote (`noot`) is left out.
Article 3 of the Archiefwet 1995 comes out as:

```yaml
- number: '3'
  text: ...
  url: https://wetten.overheid.nl/BWBR0007376/2024-06-19#Artikel3
  placement:
    hoofdstuk:
      number: II
      heading: Archiefbescheiden in het algemeen
```

An article that no container encloses has no `placement`, which is normal for
a short law. When the same kind is nested, the innermost one is recorded.
Elements the schema has no slot for (`sub-paragraaf`, `divisie`, the `bijlage`
itself) are not recorded, so the articles in an Awb bijlage, whose chapters are
`divisie` elements, come out without a placement.

## Usage

### CLI

```bash
# Download today's version of a law
regelrecht-harvester download BWBR0018451

# Download for a specific date
regelrecht-harvester download BWBR0018451 --date 2022-03-15 --output ./laws

# Large law with increased size limit
regelrecht-harvester download BWBR0020368 --max-size 200

# Download a decentralized regulation by CVDR ID (source detected automatically)
regelrecht-harvester download CVDR681386
```

### As Library

```rust
use regelrecht_harvester::{download_law, http::create_client, validate_bwb_id, validate_date};

validate_bwb_id("BWBR0018451")?;
validate_date("2025-01-01")?;

let client = create_client()?;
let law = download_law(&client, "BWBR0018451", "2025-01-01").await?;
println!("Title: {}", law.metadata.title);
println!("Articles: {}", law.articles.len());
```

For CVDR regulations use `download_cvdr_law(&client, cvdr_id, date)`, where the date is optional; `detect_source` returns the right source for either kind of identifier.

## Output path convention

```
{output}/{regulatory_layer}/{slug}/{date}.yaml
```

Example: `regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`

The regulatory layer is determined from the WTI metadata (`soort-regeling` field).

## HTTP Resilience

- **Timeout**: 30 seconds (accommodates large XML files)
- **Max response size**: 100 MB (configurable via `--max-size`)
- **Retries**: 3 attempts with exponential backoff (500ms, 1s, 2s)
- **Retry triggers**: Connection errors, timeouts, 5xx responses
- **No retry on**: 4xx client errors

## Current limitations

- **Text-only extraction** - tables and complex formatting simplified to text
- **No machine_readable** - output contains text only; executable logic added separately
- **Reference extraction incomplete** - cross-references detected but not fully resolved
- **Large laws** - a large law such as the Wet op het financieel toezicht (52.6 MB) fits in the default 100 MB limit; anything larger needs `--max-size`

## Testing

```bash
just harvester-test
```

Integration tests use fixtures from `tests/fixtures/zorgtoeslag/` (real WTI and content XML) to validate the complete pipeline from XML to valid YAML.

## Further reading

- [Law Format](/concepts/law-format) - the YAML format the harvester produces
- [Pipeline](./pipeline) - job orchestration for harvesting tasks
