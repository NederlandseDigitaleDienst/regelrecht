# Regelrecht-methode — vaste begrippen

De gedeelde taal van alle audit- en workshop-producten. Generiek: vul de
`{voorbeelden}` met casus-inhoud bij gebruik, maar de definities liggen vast.

## Kern-metafoor (voor niet-technische deelnemers)

> "Stel je een junior-medewerker voor die je wilt leren een beschikking te nemen.
> Je geeft haar een stappenplan. De YAML is dat stappenplan. De engine is de
> medewerker die het uitvoert. Alles wat we vandaag bespreken gaat over: klopt het
> stappenplan?"

## Snel-overzicht

| Begrip | Mensentaal |
|---|---|
| **YAML** | Het regel-stappenplan als tekstbestand (mens + computer leesbaar) |
| **Engine** | Het programma / de 'junior-medewerker' die het stappenplan uitvoert |
| **Output** | Het eindantwoord van een (artikel in een) wet |
| **Parameter** | Waarde die de *caller* aanlevert (bijv. BSN, een bedrag, een type) |
| **Input** | Waarde die de engine zélf ergens ophaalt (uit een andere wet) |
| **Source** | Het mechanisme waarmee een input uit een andere wet wordt opgehaald |
| **Caller** | Het systeem dat de engine aanroept (het uitvoerings-systeem) |
| **Action** | Eén formule die één output berekent |
| **Definition** | Vaste waarde uit de wettekst (een drempel, een percentage) |
| **Untranslatable** | Wettekst die bewust níet in een formule zit (menselijk oordeel) |
| **Override** | Een andere regeling vervangt jouw formule/waarde |
| **legal_basis** | De juridische grondslag onder een regel + per-formule wettekst-quote |
| **Traject** | De werkeenheid: één vertaling in wording, met een eigen branch, corpus, notities, taken en leden |
| **Claim** | Een interpretatiekeuze als notitie bij de tekst: lezing, grond, verworpen alternatief, bevoegde, regelwerkversie |
| **Attestatie** | Het besluit dat een versie productiewaardig is: gedrag vastgelegd in scenario's, toelichting per artikel met interpretatieruimte |

## Definities

### YAML — het stappenplan
Tekstbestand met regels die zowel mens als computer kan lezen. Elke wet/regeling
heeft een eigen YAML-bestand, geversioneerd per inwerkingtredingsdatum.

### Engine — de uitvoerder
Leest de YAML en komt tot een antwoord. Produceert een **trace**: stap-voor-stap
welke parameters welke outputs voeden en in welke volgorde de actions draaien.

### Output — het eindantwoord
Wat een artikel teruggeeft. Een artikel kan meerdere outputs hebben (bijv. een
ja/nee-gate én een bedrag).

### De drie soorten invoer
- **Parameter** — wat de caller aanlevert; verschilt per geval.
- **Input** — wat de engine automatisch uit een andere wet haalt; de caller ziet
  het niet.
- **Source** — de koppeling die een input vult (`source:` → andere wet/output).

Onderscheid parameter vs definition in één zin: *"Definition = een getal uit de
wet zelf. Parameter = een getal dat de caller moet aanleveren."*

### Action — één berekening
Eén regel/formule die één output berekent. Noteer formules in wiskundige/natuurlijke
notatie (AND/OR/IF/MAX/MULTIPLY), **niet** als YAML, in audit-documenten.

### Untranslatable — bewust niet gemodelleerd
Stuk wettekst dat menselijk oordeel vergt ("naar het oordeel van", "bijzondere
omstandigheden", "aannemelijk maken") en daarom niet als formule is vertaald.

Twee subtypes — belangrijk om te benoemen in een audit:
- **Factual** (feitelijk vaststelbaar door een systeem): kán alsnog een
  caller-parameter / gate worden.
- **Judgment** (oordeelsvorming / prognose): blijft untranslatable zonder
  waardeverlies.

### Override — andere regeling wint
Een andere regeling zegt "gebruik mijn waarde/formule voor jouw output". Markeer
de grondslag-vraag expliciet: werkt de override mechanisch correct, maar klopt de
juridische *grondslag-attributie*? (Een klassieke valkuil: de mechanische route is
korter dan juridisch houdbaar.)

### legal_basis — grondslag + quote
Waar berust deze regel op (welke wet, welk artikel), en — per formule — de
letterlijke wettekst-quote die de formule dekt (`legal_basis.explanation`). Dit is
wat je voorleest tijdens een walk-through: *"De YAML zegt: '{quote}'. Klopt dat?"*

## Stelsel-lagen (voor scope-analyse)

Trajecten vallen vaak in lagen. Generiek patroon:
- **Laag A (grondslag)**: de formele wet die delegeert (vaak niet zelf
  machine-leesbaar, alleen `legal_basis`-doel).
- **Laag B (uitwerking)**: uitwerkende regeling(en) waar de kern-berekening leeft:
  gedelegeerd (een ministeriële regeling of AMvB) of, bij medebewind, een
  gemeentelijke verordening die de wet uitvoert en er wel aan gebonden is.
- **Laag C (lokale/uitvoerende laag)**: de orchestrator die scope kiest en
  bovenliggende lagen inpakt via `source:`-calls.
- **Laag D (databronnen)**: wetten die alleen *data* leveren (persoonsgegevens,
  normen), geen formules in de keten.

Drie relatie-types in de wet-graph:
- `==source==>` executie-tijd data-call (formule A haalt waarde uit wet B)
- `-.grondslag.->` legal_basis (juridisch fundament, geen runtime-dependency)
- `--overrides-->` B verandert de betekenis van A's output
- `-.data.->` impliciete parameter-dependency (databron levert data, geen formule)
