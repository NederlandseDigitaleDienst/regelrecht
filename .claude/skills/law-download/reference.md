# Dutch Law Downloader - Technical Reference

## API Documentation

### SRU (Search/Retrieve via URL) API

**Base Endpoint:**
```
https://zoekservice.overheid.nl/sru/Search
```

**Required Parameters:**
- `operation=searchRetrieve`
- `version=1.2`
- `x-connection=BWB` (for Basiswettenbestand)
- `query={CQL_QUERY}` (CQL = Contextual Query Language)

**Optional Parameters:**
- `maximumRecords={N}` (default: 50, max: unlimited)
- `startRecord={N}` (for pagination)

### CQL Query Examples

**By title:**
```
dcterms.title any "zorgtoeslag"
```

**By BWBR ID:**
```
dcterms.identifier==BWBR0018451
```

**By type:**
```
dcterms.type=wet
```

**By effective date:**
```
overheidbwb.geldigheidsdatum=2025-01-01
```

**Combined queries (use AND/OR):**
```
dcterms.type=wet AND overheidbwb.geldigheidsdatum>=2025-01-01
```

### XML Repository Structure

**Base URL:**
```
https://repository.officiele-overheidspublicaties.nl/bwb/
```

**File Patterns:**

1. **WTI (Wetstechnische Informatie):**
   ```
   {BASE_URL}/{BWBR_ID}/{BWBR_ID}.WTI
   ```
   Example: `https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/BWBR0018451.WTI`

2. **Toestand (Consolidated Text):**
   ```
   {BASE_URL}/{BWBR_ID}/{DATE}/xml/{BWBR_ID}_{DATE}.xml
   ```
   Example: `https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/2025-01-01/xml/BWBR0018451_2025-01-01.xml`

3. **Manifest (Available Versions):**
   ```
   {BASE_URL}/{BWBR_ID}/manifest.xml
   ```
   Example: `https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/manifest.xml`

## XML Schemas

### WTI XML Schema

**Namespace:**
```xml
xmlns:bwb-dl="http://www.geonovum.nl/bwb-dl/1.0"
```

**Key Elements:**
```xml
<wetgeving-metadata xmlns:bwb-dl="http://www.geonovum.nl/bwb-dl/1.0">
  <bwb-dl:bwb-id>BWBR0018451</bwb-dl:bwb-id>
  <bwb-dl:soort>wet</bwb-dl:soort>
  <bwb-dl:citeertitel>Wet op de zorgtoeslag</bwb-dl:citeertitel>
  <bwb-dl:officiele-titel>Wet van 21 december 2005...</bwb-dl:officiele-titel>
  <bwb-dl:publicatiedatum>2005-12-30</bwb-dl:publicatiedatum>
  <bwb-dl:inwerkingtreding>
    <bwb-dl:datum-in-werking>2006-01-01</bwb-dl:datum-in-werking>
  </bwb-dl:inwerkingtreding>
</wetgeving-metadata>
```

### Toestand XML Schema

**Namespace:**
```xml
xmlns:bwb="http://www.overheid.nl/2011/BWB"
```

**Document Structure:**
```xml
<wetgeving xmlns:bwb="http://www.overheid.nl/2011/BWB">
  <wet-besluit>
    <wettekst>
      <artikel eId="chp_1__art_1" wId="...">
        <kop>
          <label>Artikel</label>
          <nr status="officieel">1</nr>
        </kop>
        <lid eId="..." status="goed">
          <lidnr status="officieel">1</lidnr>
          <al>Legal text paragraph...</al>
        </lid>
      </artikel>
    </wettekst>
  </wet-besluit>
</wetgeving>
```

**Text Formatting Elements:**

| XML Element | Purpose | Markdown Conversion |
|-------------|---------|-------------------|
| `<al>` | Text paragraph | Normal text |
| `<lid>` | Article paragraph/sub-section | Numbered paragraph |
| `<lijst>` | List | Markdown list |
| `<li>` | List item | `- ` or `1. ` |
| `<nadruk type="cur">` | Cursive/emphasis | `*text*` |
| `<nadruk type="vet">` | Bold | `**text**` |
| `<extref>` | External reference/link | `[text](url)` |
| `<intref>` | Internal reference | `[text](#section)` |
| `<table>` | Table | Markdown table |

## Regulatory Layer Mapping

Map `<bwb-dl:soort>` to `regulatory_layer`:

| Dutch Term | YAML Value |
|------------|------------|
| wet | WET |
| AMvB | AMVB |
| Algemene maatregel van bestuur | AMVB |
| ministeriele regeling | MINISTERIELE_REGELING |
| ministeriële regeling | MINISTERIELE_REGELING |
| beleidsregel | BELEIDSREGEL |
| koninklijk besluit | KONINKLIJK_BESLUIT |
| waterschapsverordening | WATERSCHAPS_VERORDENING |

**Note:** `VERORDENING` and `REGELING` are NOT valid schema enum values.
Valid values: GRONDWET, WET, AMVB, KONINKLIJK_BESLUIT, MINISTERIELE_REGELING,
BELEIDSREGEL, EU_VERORDENING, EU_RICHTLIJN, VERDRAG, UITVOERINGSBELEID,
GEMEENTELIJKE_VERORDENING, PROVINCIALE_VERORDENING, WATERSCHAPS_VERORDENING.

## Target YAML Schema

**Schema URL:**
```
https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.9.0/schema/v0.9.0/schema.json
```

**Required Fields (always):**
- `$schema` (string) — URL to the schema
- `$id` (string) — law slug identifier (e.g., `participatiewet`)
- `regulatory_layer` (enum)
- `publication_date` (string, YYYY-MM-DD)
- `url` (string, top-level)
- `articles` (array)
  - `number` (string)
  - `text` (string, multiline)
  - `url` (string)

**Conditionally Required:**
- `bwb_id` (string, top-level) — required for WET, AMVB, MINISTERIELE_REGELING, GRONDWET

**Optional Fields:**
- `name` (string) — law title
- `valid_from` (string, YYYY-MM-DD) — effective date
- `machine_readable` (object) - NOT included by this skill
- `references` (array)

**Note:** All identifiers (`bwb_id`, `url`) are top-level, not nested.

## File System Structure

**Pattern:**
```
corpus/regulation/nl/{regulatory_layer}/{law_id}/{valid_from}.yaml
```

**Examples:**
```
corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml
corpus/regulation/nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml
corpus/regulation/nl/amvb/besluit_zorgverzekering/2024-01-01.yaml
```

**Law ID Generation:**
- Take `<bwb-dl:citeertitel>` or `<bwb-dl:officiele-titel>`
- Convert to lowercase
- Replace spaces with underscores
- Remove special characters
- Example: "Wet op de zorgtoeslag" → "wet_op_de_zorgtoeslag"

## XML Parsing Reference

The law-download skill (Claude) handles XML parsing directly using WebFetch.
Below are the key XML structures and extraction patterns to follow.

### WTI Extraction

From a WTI file, extract these fields using the `bwb-dl` namespace (`http://www.geonovum.nl/bwb-dl/1.0`):

| XPath | Field |
|-------|-------|
| `//bwb-dl:bwb-id` | BWB identifier (e.g., `BWBR0018451`) |
| `//bwb-dl:soort` | Regulation type (maps to `regulatory_layer`) |
| `//bwb-dl:citeertitel` | Citation title (used for law ID) |
| `//bwb-dl:publicatiedatum` | Publication date |
| `//bwb-dl:datum-in-werking` | Effective date |

### Toestand (Consolidated Text) Extraction

From a toestand XML, extract articles using the `bwb` namespace (`http://www.overheid.nl/2011/BWB`):

| XPath | Field |
|-------|-------|
| `//bwb:artikel` | Article elements |
| `bwb:kop/bwb:nr` | Article number |
| `bwb:lid` | Paragraphs within article |
| `bwb:lid/bwb:al` | Paragraph text |

### SRU Query URL Construction

Construct SRU URLs by URL-encoding the CQL query parameter:
```
https://zoekservice.overheid.nl/sru/Search?operation=searchRetrieve&version=1.2&x-connection=BWB&query={URL_ENCODED_CQL}
```

### Validation

After generating YAML, validate it:
```bash
just validate {FILE_PATH}
```

## Debugging Tips

1. **Check if law exists in BWB:**
   - Go to https://wetten.overheid.nl/ and search manually
   - Verify BWBR ID is correct

2. **Test XML download:**
   - Try URLs in browser first
   - Check manifest.xml for available dates

3. **XML parsing issues:**
   - Save raw XML to file for inspection
   - Check namespaces are correct
   - Use xmllint for validation: `xmllint --noout file.xml`

4. **SRU API not returning results:**
   - Try broader search terms
   - Remove date restrictions
   - Check URL encoding

## External Resources

- **BWB Documentation:** https://www.overheid.nl/help/wet-en-regelgeving
- **SRU API Guide:** https://data.overheid.nl/sites/default/files/dataset/...Handleiding+SRU+BWB.pdf
- **XML Schemas:** https://standaarden.overheid.nl/bwb
- **KOOP (Publisher):** https://www.koopoverheid.nl/
- **Wetten.overheid.nl:** https://wetten.overheid.nl/
