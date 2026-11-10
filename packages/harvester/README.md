# regelrecht-harvester

Download Dutch legislation from the BWB repository and convert to schema-compliant YAML.

## Quick Start

```bash
# Build from source
cargo build --release

# Download a law (uses today's date by default)
regelrecht-harvester download BWBR0018451

# Download the version in force on a specific date
regelrecht-harvester download BWBR0018451 --date 2025-01-01 --output ./regulation/nl

# Download one version per in-force date in a period
regelrecht-harvester download BWBR0018472 --from 2024-01-01 --to 2026-12-31
```

## Installation

### From Source

```bash
# Install to ~/.cargo/bin
cargo install --path packages/harvester

# Or build release binary (output: target/release/regelrecht-harvester)
cargo build --release
```

### Requirements

- Rust 1.70+ (stable toolchain)

## CLI Usage

```
regelrecht-harvester <COMMAND>

Commands:
  download  Download a law by BWB ID and convert to YAML
  help      Print help information
```

### Download Command

```
regelrecht-harvester download <BWB_ID> [OPTIONS]

Arguments:
  <BWB_ID>  BWB identifier (e.g., BWBR0018451)

Options:
  -d, --date <DATE>      Harvest the version in force on this date (default: today)
      --from <DATE>      Harvest one version per in-force date from this date (BWB only)
      --to <DATE>        Last in-force date to harvest with --from (inclusive)
  -o, --output <PATH>    Output directory (default: regulation/nl/)
      --max-size <MB>    Maximum response size in MB (default: 100)
  -h, --help             Print help
```

**Examples:**

```bash
# Download Zorgtoeslagwet for today
regelrecht-harvester download BWBR0018451

# Download Participatiewet for a specific date
regelrecht-harvester download BWBR0045530 --date 2022-03-15 --output ./laws

# Download a large law (increase size limit)
regelrecht-harvester download BWBR0020368 --max-size 200
```

### Versions and in-force dates

BWB keeps one consolidation (toestand) per date on which an article changes or comes into force. The manifest lists them. With `--date`, the harvester takes the consolidation that covers that date. It saves the file under the consolidation's start date, so `--date 2025-03-01` can produce `2025-01-01.yaml`. Without `--date` it uses today.

With `--from` and `--to`, the harvester saves every consolidation that starts in the period as `<valid_from>.yaml`. A consolidation that started before `--from` is not included. The period may reach into the future, because BWB publishes a consolidation as soon as its in-force date is fixed. `--date` must not be in the future.

A version contains only what is in force on its `valid_from`. An article that is enacted but not yet in force (status `nogniet`, or an `inwerking` date after `valid_from`) is left out with a warning. If its in-force date is known, the warning names the version it appears in. When BWB has no consolidation yet for that date, the warning says so, and that version cannot be harvested until BWB adds it.

`publication_date` is the latest `publicatie_bron` among the elements in force in that version. References to EU acts by CELEX number link to EUR-Lex. A reference without a target the reader can follow, such as `doc="onbekend"`, stays plain text.

## Library Usage

The harvester can also be used as a Rust library:

```rust
use regelrecht_harvester::{download_law, validate_bwb_id, validate_date};
use regelrecht_harvester::types::{Law, RegulatoryLayer};

fn main() -> regelrecht_harvester::Result<()> {
    // Validate inputs
    validate_bwb_id("BWBR0018451")?;
    validate_date("2025-01-01")?;

    // Download and parse a law
    let law: Law = download_law("BWBR0018451", "2025-01-01")?;

    println!("Title: {}", law.metadata.title);
    println!("Type: {}", law.metadata.regulatory_layer.as_str());
    println!("Articles: {}", law.articles.len());

    for article in &law.articles {
        println!("  Article {}: {} chars", article.number, article.text.len());
    }

    Ok(())
}
```

## Domain: Dutch Law & BWB

### BWB (Basiswettenbestand)

The **Basiswettenbestand** (BWB) is the official Dutch government repository for consolidated legislation. It contains machine-readable versions of all Dutch national laws, with historical versions for any given date.

- **Repository URL:** `https://repository.officiele-overheidspublicaties.nl/bwb`
- **Public viewing:** `https://wetten.overheid.nl`

### BWB ID Format

Each law has a unique **BWB identifier**:

| Pattern | Example | Description |
|---------|---------|-------------|
| `BWBR` + 7 digits | `BWBR0018451` | Regelingen (regulations) |

Examples:
- `BWBR0018451` - Wet op de zorgtoeslag (Healthcare Allowance Act)
- `BWBR0045530` - Participatiewet (Participation Act)
- `BWBR0020368` - Wet op het financieel toezicht (large, ~52 MB)

### Regulatory Layers

Dutch law has several regulatory layers, indicated in the output YAML:

| Layer | Dutch Name | Description |
|-------|------------|-------------|
| `WET` | Wet | Formal law (passed by parliament) |
| `AMVB` | Algemene Maatregel van Bestuur | General administrative measure |
| `MINISTERIELE_REGELING` | Ministeriële regeling | Ministerial regulation |
| `KONINKLIJK_BESLUIT` | Koninklijk Besluit | Royal decree |
| `BELEIDSREGEL` | Beleidsregel | Policy rule |

### Key Dutch Legal Terms

| Dutch | English | Description |
|-------|---------|-------------|
| Artikel | Article | Main structural unit of a law |
| Lid | Paragraph | Numbered paragraph within an article |
| Onderdeel | Subsection | Lettered subsection (a, b, c...) |
| Hoofdstuk | Chapter | Chapter grouping articles |
| Afdeling | Division | Division within a chapter |
| Paragraaf | Section | Section within a division |
| Aanhef | Preamble | Opening text before articles |
| Bijlage | Appendix | Attachment to the law |

## Output Format

The harvester produces YAML files conforming to the [regelrecht schema](https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.3.1/schema.json).

### Example Output

```yaml
---
$schema: https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.3.1/schema.json
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2024-12-20'
valid_from: '2025-01-01'
bwb_id: BWBR0018451
url: https://wetten.overheid.nl/BWBR0018451/2025-01-01
name: '#wet_naam'
competent_authority: '#bevoegd_gezag'
articles:
  - number: '1'
    text: |-
      In deze wet en de daarop berustende bepalingen wordt verstaan onder:

      a. verzekerde: degene die ingevolge de Zorgverzekeringswet verzekerd is;
      b. toetsingsinkomen: het verzamelinkomen, bedoeld in artikel 2.18 van
      de Wet inkomstenbelasting 2001;
      ...
    url: https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1
  - number: '2'
    text: |-
      1. Indien de normpremie voor een verzekerde in het berekeningsjaar
      minder bedraagt dan de standaardpremie...
    url: https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2
```

### Output Directory Structure

Files are saved to: `{output_dir}/{regulatory_layer}/{slug}/{date}.yaml`

```
regulation/nl/
├── wet/
│   ├── wet_op_de_zorgtoeslag/
│   │   └── 2025-01-01.yaml
│   └── participatiewet/
│       └── 2022-03-15.yaml
├── ministeriele_regeling/
│   └── regeling_standaardpremie/
│       └── 2025-01-01.yaml
└── amvb/
    └── ...
```

## Architecture

The harvester is organized into several modules:

| Module | Description |
|--------|-------------|
| `cli` | Command-line interface (clap) |
| `config` | Configuration constants and validation |
| `harvester` | Main download pipeline orchestration |
| `http` | HTTP client for BWB repository |
| `wti` | WTI metadata file parsing |
| `content` | Content XML downloading |
| `xml` | XML parsing utilities |
| `registry` | Extensible element handler system |
| `splitting` | Article splitting logic |
| `yaml` | YAML output generation |
| `types` | Core data types (Law, Article, Reference) |
| `error` | Error types and Result alias |

### Processing Pipeline

```
BWB ID + Date
     │
     ▼
┌─────────────┐     ┌─────────────┐
│  Download   │────▶│ Parse WTI   │  (metadata: title, type, dates)
│  WTI file   │     │   metadata  │
└─────────────┘     └─────────────┘
                           │
                           ▼
                    ┌─────────────┐     ┌─────────────┐
                    │  Download   │────▶│ Split into  │  (article splitting)
                    │ content XML │     │  articles   │
                    └─────────────┘     └─────────────┘
                                               │
                                               ▼
                                        ┌─────────────┐
                                        │ Generate    │
                                        │ YAML output │
                                        └─────────────┘
```

## Limitations & Known Issues

This harvester is under active development. Current limitations:

- **Text-only extraction**: Extracts article text only; tables and complex formatting may be simplified
- **No machine_readable generation**: Outputs text only; `machine_readable` sections must be added manually
- **Reference extraction**: Cross-references are detected but not fully resolved
- **Large laws**: Some laws (like Wet op het financieel toezicht) require increased `--max-size`

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- download BWBR0018451

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

## License

EUPL-1.2 - See [LICENSE](../../LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/NederlandseDigitaleDienst/regelrecht)
- [Schema Documentation](https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.3.1/schema.json)
- [wetten.overheid.nl](https://wetten.overheid.nl) - Official Dutch law viewer
- [BWB Repository](https://repository.officiele-overheidspublicaties.nl/bwb) - Raw BWB data
