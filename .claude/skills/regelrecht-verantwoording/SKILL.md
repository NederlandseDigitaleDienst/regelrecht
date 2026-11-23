---
name: regelrecht-verantwoording
description: >
  Legt de interpretatie van machine-uitvoerbare regels vast zoals een cel in de
  chronolexografie een vaststelling vastlegt: per artikel een claim (lezing, grond,
  verworpen alternatief, bevoegde, regelwerkversie), de uitspraak van de bevoegde, een
  scenario dat de lezing in gedrag vastlegt, en ten slotte een attestatie van de hele
  versie. Samen vormen ze een moderne, artikelsgewijze memorie van toelichting. Gebruik
  dit wanneer een open term wordt ingevuld, een bepaling meerdere lezingen toelaat, een
  untranslatable toch een invulling krijgt, je als bevoegde uitspraak doet, of een
  versie wordt geattesteerd. Casus-agnostisch. Het ritme staat in regelrecht-traject.
allowed-tools: Read, Glob, Grep, Bash, Edit, Write
user-invocable: true
---

# Verantwoording — de toelichting bij machine-uitvoerbare regels

Een klassieke memorie van toelichting legt per artikel uit hoe het bedoeld is, met
rekenvoorbeelden. Ze is een document: zodra de wet wijzigt, veroudert ze ongemerkt. Deze
skill doet hetzelfde voor machine-uitvoerbare regels, met de principes van de
chronolexografie (`docs/…/concepts/chronolexography`):

- **Een interpretatie is een vaststelling, geen waarheid**: "op moment T heeft actor A, op
  deze grond, deze lezing gekozen boven dat alternatief, voor regelwerkversie H."
- **De kroniek groeit alleen.** Er wordt niets gewijzigd. Een correctie, een uitspraak of een
  overname is een nieuw record dat verwijst naar het vorige.
- **Een record kopieert de wet niet; het verwijst naar de versie** (`law_id`, `valid_from` en
  hash), zoals `law_versions` op een chronolexogram.
- **De toelichting per artikel is een reductie**: een antwoord dat je uit de kroniek
  berekent, geen document dat iemand bijhoudt.
- **Gedrag bewijs je door opnieuw te draaien.** De toelichting zegt waarom, het scenario zegt
  wat, en alleen het scenario kan dat laatste bewijzen.

Een productiewaardige versie is een **geattesteerde** versie. Die rust op beide benen: het
gedrag (scenario's en tests) en de toelichting per belangrijk artikel. Zie *Attestatie*,
onderaan.

## Wat er is

| | wat | wie |
|---|---|---|
| **anker** | een letterlijk citaat uit de wettekst | — |
| **claim** | de lezing, grond, verworpen alternatief, bevoegde en regelwerkversie | model of analist |
| **uitspraak** | het oordeel over één claim | de bevoegde |
| **scenario** | legt de lezing vast in gedrag: rood onder het alternatief | analist |
| **attestatie** | het besluit dat versie H productiewaardig is | de bevoegde |

De stand van een claim en de toelichting per artikel worden **afgeleid**; ze worden niet
opgeslagen.

**Wie vaststelt.** Elke organisatie stelt haar eigen lezingen vast: de wetgevingsdirectie de
lezing van de wetgever, de uitvoeringsorganisatie de lezing in de uitvoering. Een uitvoerder
stelt de bedoeling van de wetgever niet zelf vast. Die citeert hij, met versie. Een eigen
lezing van een open norm is uitvoeringsbeleid en komt via `implements`. **De rechter is een
bron, geen vaststeller in deze kroniek.** Een uitspraak kan een grond zijn, of aanleiding
voor een nieuwe claim. Hoe je jurisprudentie binnen een cyclus overneemt, is nog niet
uitgewerkt.

## Wanneer wel, wanneer niet

Wel: een `open_term` wordt ingevuld, je kiest tussen lezingen, een `untranslatable` krijgt
alsnog een invulling, een analist neemt een modelvoorstel over of verwerpt het, of een
versie moet naar productie.

Niet: een opmerking zonder gevolg voor de uitvoering (een gewone `commenting`-notitie volstaat
dan), of een keuze over het model zelf zonder citaat (een dode parameter). Die laatste hoort
in de wet-YAML.

## Het bestand

`annotations/{law_id}/annotations.yaml` naast `regulation/` (in de monorepo:
`corpus/annotations/`). Werk op de trajectbranch of een feature-branch, nooit op main.

Conceptueel is dit de **interpretatiekroniek**. De annotaties-sidecar is voorlopig de opslag:
hij is append-only, de editor leest hem, en omdat elk record zijn regelwerkversie noemt, kan
hij later mechanisch naar een kroniekformaat met `refers_to` worden overgezet.

**Append-only.** Zet nieuwe notities achteraan, met dezelfde inspringing. Parse en herschrijf
het bestand nooit. De editor werkt net zo en weigert een herbouwd bestand.

## Stap 1 — het anker

Neem de passage letterlijk over uit het `text`-veld van het artikel in de geldende toestand.
Kies een citaat dat één keer voorkomt, of geef een prefix en suffix mee van 10 tot 40 tekens.

- Controleer het tegen de geparste tekst, niet tegen het bestand. `text` is omgebroken; houd
  prefix en suffix binnen één regel.
- `regelrecht:hint` helpt niet: de resolver negeert hem bewust.

## Stap 2 — de bevoegde

Bij een open term staat de bevoegde in `open_terms[].delegated_to`. Anders volgt hij uit het
soort vraag:

| vraag over | bevoegd |
|---|---|
| een feit, de uitvoeringspraktijk | wie de regeling uitvoert |
| de lezing van de tekst | de wetgevingsjurist (de bedoeling) of de jurist van de uitvoerder (de toepassing) |
| bevoegdheid, normconflict | jurist of bestuur |
| beleid dat ontbreekt | de normsteller |

Wijk je hiervan af, zet de reden dan in de claim. Zo'n afwijking is zelf een bevinding.

## Stap 3 — de claim

```yaml
  - type: Annotation
    motivation: questioning
    creator: <naam, of de tool bij een modelvoorstel>
    created: '<ISO-datum>'
    workflow: open
    target:
      source: regelrecht://<law_id>
      selector:
        type: TextQuoteSelector
        exact: '<citaat>'
        prefix: '<10-40 tekens>'
        suffix: '<10-40 tekens>'
    body:
      - {type: TextualBody, purpose: identifying, value: 'claim:<id>'}
      - {type: TextualBody, purpose: questioning, value: '<de kwestie, in de woorden van de tekst>'}
      - {type: TextualBody, purpose: assessing,   value: '<de gekozen lezing, in één zin>'}
      - {type: TextualBody, purpose: commenting,  value: 'grond: <wettekst · wetsgeschiedenis · systematiek · praktijk · jurisprudentie>'}
      - {type: TextualBody, purpose: commenting,  value: 'alternatief: <verworpen lezing> — gevolg: <wie merkt het, welke kant op>'}
      - {type: TextualBody, purpose: tagging,     value: 'regelwerk:<law_id>@<valid_from>#sha256:<16 hex>'}
      - {type: TextualBody, purpose: tagging,     value: 'bevoegd:<uit stap 2>'}
      - {type: TextualBody, purpose: tagging,     value: 'voorstel_van:<model|mens>'}
```

- **`claim:<id>`.** Het notitieschema kent geen id. Neem de eerste 8 tekens van
  `printf '%s|%s|%s|%s' "$law_id" "$exact" "$creator" "$created" | shasum | cut -c1-8`.
- **`regelwerk:`** is de versie waarvoor de lezing is vastgesteld. De hash wordt berekend
  over de inhoud van het YAML-bestand, net als in de engine (`content_hash`):
  `shasum -a 256 regulation/…/{law_id}/{valid_from}.yaml | cut -c1-16`. Het bestand, niet het
  artikel: zo rekent de cel ook. Of er daarna inhoudelijk iets aan dít artikel veranderde,
  stel je vast als de hash verschilt (zie *Als het regelwerk verandert*).
- **Het alternatief is verplicht.** Zonder verworpen lezing is er niets om te bekrachtigen,
  en geen scenario dat de keuze kan vastleggen.
- **Een modelvoorstel** heeft wel `bevoegd`, maar wordt pas vastgesteld als een analist het
  overneemt. Die schrijft dan een nieuwe claim met de eigen `creator`, `voorstel_van:model` en
  `over:<id van het voorstel>`. `voorstel_van` maakt meetbaar hoe vaak een voorstel
  ongewijzigd wordt overgenomen.
- Een tag uit `annotations/_vocabulary/ambiguity.yaml` mag erbij. Tags buiten dat vocabulaire
  geven een waarschuwing, geen fout.

Vult de claim een open term in, zet dan `# claim:<id>` als YAML-commentaar boven het
`implements`-blok. Het schema heeft hier nog geen veld voor.

## Stap 4 — de lezing vastleggen in gedrag

Een scenario legt een lezing pas vast als het **onder het verworpen alternatief rood wordt**.
Een scenario dat onder beide lezingen groen is, bewijst over deze keuze niets. Het alternatief
uit de claim is de mutant.

```gherkin
  @claim:<id>
  Scenario: <de casus waarin de twee lezingen uiteenlopen>
    # onder het alternatief: <de uitkomst die dan zou volgen>
    Given …
```

- Zet het scenario bij de wet (`…/scenarios/*.feature`, bucket A).
- Controleer één keer dat het discrimineert: pas het alternatief tijdelijk toe in de YAML,
  draai het scenario (`just bdd`, met `BDD_BUCKET=corpus`), zie het rood worden, en draai de
  wijziging terug. Noem in de commit dat je dit hebt gedaan.
- Een rekenvoorbeeld uit de wetsgeschiedenis (`law-mvt-research`) is vaak precies zo'n
  scenario: de wetgever die zelf laat zien welke lezing hij bedoelde.

## Stap 5 — de uitspraak (als bevoegde)

Een nieuwe notitie op hetzelfde citaat:

```yaml
  - type: Annotation
    motivation: replying
    creator: <de bevoegde>
    created: '<ISO-datum>'
    target: <identiek aan de claim>
    body:
      - {type: TextualBody, purpose: tagging,    value: 'over:<claim-id>'}
      - {type: TextualBody, purpose: tagging,    value: 'verdict:<bekrachtigd|gecorrigeerd|weerlegd|onbeslist>'}
      - {type: TextualBody, purpose: tagging,    value: 'regelwerk:<law_id>@<valid_from>#sha256:<16 hex>'}
      - {type: TextualBody, purpose: commenting, value: 'reden: <verplicht bij gecorrigeerd en onbeslist>'}
```

De `creator` moet een ander zijn dan die van de claim. Er is één uitspraak per claim. Wie
later anders oordeelt, schrijft een nieuwe claim met `over:`; de oude uitspraak blijft staan.

| verdict | gevolg |
|---|---|
| `bekrachtigd` | de lezing staat; stap 4 als dat nog niet gebeurd is |
| `gecorrigeerd` | de analist schrijft een nieuwe claim met `over:<id>` en past het corpus aan |
| `weerlegd` | de invulling gaat eruit of wordt vervangen; het verdict trekt de claim in vanaf het begin |
| `onbeslist` | blijft open, met reden; telt bij de attestatie als bewuste openheid |

## Stap 6 — valideren

```
just validate-annotations <pad/naar/annotations.yaml>
```

Faalt het schema, dan zou de editor het bestand ook weigeren. Krijg je `ambiguous`, maak
prefix of suffix langer; bij `orphaned` klopt het citaat niet met de geldende tekst.
Waarschuwingen over onbekende tags horen erbij. Bij wetten met meer dan 1.000 artikelen laadt
de engine de wet niet (`MAX_ARRAY_SIZE`) en meldt de resolver *selector not checked*. Dat
zegt niets over de claim; controleer het citaat dan met de hand.

## Wat je afleidt

**De stand van een claim:**

| stand | wanneer |
|---|---|
| voorgesteld | alleen een claim met `voorstel_van:model` en de tool als `creator` |
| vastgesteld | een claim van een mens. **Rekent mee:** de keten loopt door op deze lezing |
| uitgesproken | een `replying` met `over:<id>` van een andere `creator` |
| vastgelegd in gedrag | er is een `@claim:<id>`-scenario dat discrimineert |
| verouderd | de `regelwerk:`-hash verschilt van het bestand nu |

**De toelichting bij een artikel** is per artikel de laatste claim die geldt (niet weerlegd,
niet overschreven door een nieuwere claim met `over:`), met haar uitspraak en scenario's. Dat
is wat de editor per artikel hoort te tonen: de moderne memorie van toelichting, berekend uit
de kroniek.

## Als het regelwerk verandert

Verschilt de hash in `regelwerk:` van het bestand nu, dan zijn er twee bevindingen, net als
bij `law_since` in de cel, en die hou je uit elkaar:

| | scenario's van de claim | betekenis |
|---|---|---|
| **regelwerk veranderd** | nog groen | de lezing staat waarschijnlijk nog. Eén nieuwe claim met `over:` en de nieuwe hash volstaat; geen nieuwe uitspraak nodig als het artikel inhoudelijk gelijk bleef |
| **uitkomst veranderd** | rood | de lezing is niet meer wat het model doet. Een nieuwe claim, en een nieuwe uitspraak |

Bestaat er geen discriminerend scenario, dan valt het verschil niet te meten en is elke
hashwijziging een nieuwe vraag aan de bevoegde. Ook daarom is stap 4 de moeite waard.

## Attestatie

Een versie is productiewaardig als de bevoegde haar attesteert. De attestatie rust op twee
benen: **gedrag** (scenario's en tests) en **toelichting** (claims met uitspraak).

**Welke artikelen een toelichting nodig hebben, leid je af.** Dat zijn artikelen waarin het
model iets doet wat niet letterlijk in de tekst staat:
- een ingevulde `open_term`;
- een aanname uit `law-reverse-validate`;
- een untranslatable of marking;
- een `override`;
- een open bevinding uit de stelselanalyse.

Een artikel dat volledig in de letter terug te vinden is, heeft geen toelichting nodig; daar
is de tekst de toelichting.

Per artikel dat toelichting nodig heeft, één van deze vier:

| toelichting | discriminerend scenario | geldt als |
|---|---|---|
| ✓ uitgesproken | ✓ | sterkst: uitgelegd en vastgelegd |
| — | ✓ | alleen bij een artikel zonder interpretatieruimte; anders een gat |
| ✓ uitgesproken | — | alleen bij een oordeel per geval (discretie, untranslatable): wat de machine niet beslist, valt niet vast te leggen in gedrag. Anders een gat |
| ✓ `onbeslist` met reden | — | bewuste openheid; telt mee, en de attestatie noemt het |

Voorwaarden voor de attestatie van versie H:
1. alle scenario's en tests van de wet groen op H;
2. elk artikel dat toelichting nodig heeft valt in een geldige rij hierboven, met claims op
   hash H (of "regelwerk veranderd" met groene scenario's);
3. geen claim in stand *voorgesteld* of *verouderd*;
4. de attesterende is een ander dan wie de claims vaststelde.

**Vastleggen.** Het notitieschema vereist een citaat, en een attestatie gaat over de hele
versie. Leg haar daarom vast als **annotated git-tag** op de commit die H bevat:
`attestatie/<law_id>/<valid_from>`. De tag heeft een auteur en een datum, verandert niet, en
de commit legt de inhoud vast. In de tagboodschap: de hash, per toelichtingsartikel de rij uit
de tabel met claim-id's, en de bewuste openheden. Intrekken (ex tunc, bij een fout) is een
nieuwe tag `attestatie-ingetrokken/<law_id>/<valid_from>` met reden. Een nieuwe versie van de
wet krijgt een eigen attestatie (ex nunc). Tags maken of pushen doe je alleen op verzoek.

Een script dat de voorwaarden controleert bestaat nog niet. Tot het er is, loopt de analist ze
na en zet het resultaat in de PR van het traject.

## Open

- **Welke tijdas.** Een toelichting noemt nu zowel `valid_from` (de wet die in werking is) als
  de hash (de inhoud). Of zij over het een of het ander gaat, is dezelfde open vraag als de
  tweede tijdas in de chronolexografie (`concepts/chronolexography/open-questions`). Tot die
  is beslist, leg je beide vast.
- **Jurisprudentie overnemen.** De rechter is een bron. Hoe een uitspraak binnen een cyclus
  wordt overgenomen, is nog niet uitgewerkt.

## Rapporteren

```
Verantwoording — {law_id} @ {valid_from} #{hash}
  toelichting nodig:   {N} artikelen
  claims:              {N} (voorgesteld {a} · vastgesteld {b} · uitgesproken {c} · verouderd {d})
  in gedrag vastgelegd:{N} claims met een discriminerend scenario
  wacht op:            {bevoegde}: {n}, …
  attestatie:          {mogelijk | geblokkeerd door: …}
```
