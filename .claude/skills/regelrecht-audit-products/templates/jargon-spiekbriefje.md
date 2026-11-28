# Jargon-spiekbriefje — {Traject}-workshop

*Houd dit naast je. Alle schema-/engine-termen in mensentaal, met één concreet
casus-voorbeeld per stuk. Generieke definities staan vast (zie method-glossary);
vul de `{voorbeelden}` met casus-inhoud.*

## Kern-metafoor (1 zin voor deelnemers)

> "Stel je een junior-medewerker voor die je wilt leren {de beschikking} nemen. Je
> geeft haar een stappenplan. De YAML is dat stappenplan. De engine is de medewerker
> die het uitvoert. Alles wat we vandaag bespreken: klopt het stappenplan?"

## Snel-overzicht

| Jargon | Mensentaal |
|---|---|
| **YAML** | Het regel-stappenplan als tekstbestand |
| **Engine** | De 'junior-medewerker' die het stappenplan uitvoert |
| **Output** | Het eindantwoord van een wet |
| **Parameter** | Getal dat de caller aanlevert ({voorbeeld}) |
| **Input** | Getal dat de engine zelf ergens ophaalt ({voorbeeld}) |
| **Source** | Mechanisme waarmee input uit een andere wet wordt opgehaald |
| **Caller** | Systeem dat de engine aanroept ({het uitvoerings-systeem}) |
| **Action** | Eén formule die één output berekent |
| **Definition** | Vaste waarde uit de wet ({voorbeeld-drempel}) |
| **Untranslatable** | Wettekst die bewust niet in een formule zit (handmatig oordeel) |
| **Override** | Andere wet vervangt jouw formule/waarde |
| **legal_basis** | Juridische grondslag onder een regel + wettekst-quote per formule |

---

## De drie soorten invoer — hier zit vaak een scope-vraag

- **Parameter** — wat de caller aanlevert. *Voorbeeld*: {casus-parameter}.
- **Input** — wat de engine automatisch uit een andere wet haalt. *Voorbeeld*:
  {casus-input}.
- **Source** — het ophaal-mechanisme. *"Deze input `{x}` heeft een source naar
  {wet}. De engine belt {wet} met onze gegevens en gebruikt dat antwoord."*

**Source vs caller in 2 zinnen voor deelnemers**:
> "Een bedrag kan op twee manieren bij onze formule komen. Óf jullie systeem stopt
> het getal erin (caller-parameter). Óf de engine gaat automatisch naar de
> {bron}-YAML, leest daar de actuele waarde, en gebruikt die (source). Source is
> mechanisch consistent — mits {bron} onderhouden wordt. Caller geeft jullie controle."

---

## Untranslatable — belangrijk concept

> "Sommige stukken kunnen we niet in een formule gieten — '{voorbeeld open norm}',
> 'bijzondere omstandigheden', 'aannemelijk maken' — dat vergt menselijk oordeel.
> Die markeren we als 'untranslatable'. De engine doet die check niet; de behandelaar
> doet 'm handmatig."

## Override — *(indien van toepassing)*

> "{Wet A} zegt: {waarde X}. {Wet B} zegt: nee, {waarde Y}. De engine weet dat —
> zodra de betreffende waarde berekend wordt, pakt de engine automatisch {waarde Y}."

---

## Back-pocket-antwoorden op lastige vragen

**"Wat als jullie formule een getal geeft dat ik in de praktijk niet zou geven?"**
→ "Perfect — dat willen we weten. Geef een concrete case, dan laten we de engine die
doorrekenen en zien we waar de afwijking zit." (gebruik het orakel voor de trace)

**"Wet is niet mechanisch, je kan dit niet zomaar in formules gieten."**
→ "Klopt. De formules dekken niet alle nuance. Daarom is een deel 'untranslatable' —
bewust niet mechanisch. We willen van jullie horen waar die grens ligt."

**"Waarom niet alles in formules?"**
→ "Alleen wat mechanisch toetsbaar is. Alles wat discretie/oordeel vergt blijft
menselijk. Vandaag bepalen we samen welke zin waar valt."

**"Deze wet kan morgen veranderen!"**
→ "Daarom is de YAML gelabeld per datum. Bij een wijziging maken we een nieuwe versie;
de oude blijft beschikbaar voor zaken uit die periode."
