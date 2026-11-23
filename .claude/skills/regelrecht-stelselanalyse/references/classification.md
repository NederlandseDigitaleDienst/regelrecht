# Vier-weg-classificatie — het conceptuele hart

Elke bevinding in een desk-review krijgt **precies één** van vier labels. Het label
bepaalt waar de bevinding landt en welke actie volgt. Verkeerd labelen leidt tot de
verkeerde actie (een wet "fixen" die we niet kunnen wijzigen, of onze YAML niet
corrigeren omdat we de fout bij de wet legden).

> Het label zegt wat voor *soort* bevinding het is en welke actie volgt, niet wie haar mag
> sluiten. Dat is een aparte vraag, die je afleidt uit waar de bevinding aan hangt (zie
> `regelrecht-verantwoording`, stap 2). Kiest een bevinding een lezing, leg haar dan vast als
> claim.

| Label | Wat | Waar het landt | Actie |
|---|---|---|---|
| **Modellering-fout** | Onze YAML/feature wijkt af van de (correcte) wettekst | `modellering-fixes-plan` | Wij fixen de YAML |
| **Wetgevings-fout** | De wet zelf is onjuist/achterhaald/onuitvoerbaar; niet door interpretatie te repareren | `wetgevingsfouten-analyse` | Aanbevolen wetgevings-actie |
| **Engine-limitatie** | Wet + modellering kloppen, maar de engine kan het (nog) niet uitvoeren | `engine-limitaties` | Engine-issue / engine-PR |
| **Acceptabele untranslatable** | Open norm die bewust niet gemodelleerd wordt en dat ook niet hoeft | gemarkeerd in de YAML | Geen actie |

## Beslisboom

1. Wijkt onze YAML/feature af van wat de wettekst feitelijk zegt? → **modellering-fout**.
2. Klopt onze modellering met de wet, maar is de *wet zelf* fout/achterhaald/onuitvoerbaar?
   → **wetgevings-fout**.
3. Kloppen wet én modellering, maar faalt de *engine* op de uitvoering? → **engine-limitatie**.
4. Is het een open norm die we bewust niet modelleren? → ga naar het onderscheid hieronder.

## Het subtielste onderscheid: open norm — untranslatable of wetgevings-fout?

Dezelfde clausule (*"naar het oordeel van"*, *"redelijkerwijs"*, *"onbillijkheid van
overwegende aard"*) kan beide zijn. De stance bepaalt het label:

- **Acceptabele untranslatable** wanneer er een **kenbare beslisser** is én een
  **toetsbaar kader** (wie beslist, wanneer, op welke grond, met welk rechtsmiddel).
  De norm vergt menselijk oordeel, maar is wel uitvoerbaar en toetsbaar.
- **Wetgevings-fout** ("open norm zonder beslisser") wanneer beslisser, kader, of
  grond **ontbreekt**: niet duidelijk wie toetst, geen criterium, geen rechtsmiddel,
  of een circulair criterium. Dan is de norm niet kenbaar uitvoerbaar.

Toets daarom bij elke open norm: *wie* beslist, *wanneer*, op welk *dossier/grond*, en
met welk *rechtsmiddel*. Ontbreekt een van deze → kandidaat wetgevings-fout.

## Onderscheid met de audit-doc (workshop-skill)

| | Audit-doc (`regelrecht-audit-products`) | Desk-classificatie (deze skill) |
|---|---|---|
| Vraag | "Klopt ons stappenplan?" | "Klopt de wet zelf, en is hij uitvoerbaar?" |
| Aanname | De wet is correct; fout zit in onze modellering | Fout kan in modellering, wet, of engine zitten |
| Open norm | Geaccepteerde untranslatable | Untranslatable **of** wetgevings-fout (zie boven) |
| Uitkomst | Bevestigde/gecorrigeerde YAML | Wetgevings-notitie + fixes + engine-issues + status |
| Publiek | Domein-experts in een sessie | Wetgevingsjuristen / ministerie / eigen team |

Een audit-doc kent maar één richting (de fout zit bij ons). Deze skill voegt de andere
drie richtingen toe. Dat is het hele verschil — en de reden dat de producten op elkaar
lijken maar niet hetzelfde zijn.

## De features-vs-YAML meta-check

Voer bij elke validatie uit: *maken de features en de YAML dezelfde fout?* Zo ja, dan
valideert de BDD-suite de YAML in plaats van de wet — groene tests bewijzen dan niets
over juridische correctheid. Dit is een veelvoorkomende valkuil en hoort als expliciete
meta-bevinding in de synthese.
