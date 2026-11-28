# {Traject} — testcase-scenario's voor vervolg-sessie

Scenario's om met experts door te lopen. Elk raakt een ander hoofdpad in de digitale
beoordelings-keten. **Kernvraag per scenario**: *"komen jullie op hetzelfde uit als
de engine? En hoe rekenen jullie eigenlijk?"*

> **Vertrouwelijkheid**: scenario's zijn fictief of sterk-gelijkend op echte
> casussen — **nooit echte persoonsgegevens**. De BDD-versie hoort in een
> feature-file in de `scenarios/`-map naast de wet die hij test.

> **Methode**: bouw de scenario's met `regelrecht-scenario-traces` — geef elke casus een
> naam (persona op casus-assen, niet een losse leaf-bundel) zodat experts 'm herkennen, en
> kies personas die samen verschillende keten-paden raken. Bij verschil met de praktijk
> wijzen de keten-checkpoints aan wélke schakel afwijkt, niet alleen dát de uitkomst
> verschilt.

---

## 1. {Scenario-titel — kort beoordelings-pad}

> {Fictieve persoon/situatie in 2-3 zinnen, met de relevante feiten en het bedrag.}

**Wat de engine berekent**: {trace-conclusie in één regel — bijv. volledige
toekenning €X / afwijzing / gedeeltelijk}.

**Discussie**:
- Komen jullie op hetzelfde uit?
- Welke gegevens zien jullie wel die de digitale toets nu niet pakt?
- {scenario-specifieke doorvraag die een open punt op tafel brengt}

---

*(herhaal per scenario; kies scenario's die samen verschillende hoofdpaden raken:
het simpele/volle pad, een afwijzing via een gate, en een grond-met-nuance)*

---

## Wat ze samen aanraken

| Scenario | Hoofdpad | Engine zegt |
|---|---|---|
| 1 | {pad} | {uitkomst} |
| 2 | {pad} | {uitkomst} |
| 3 | {pad} | {uitkomst} |

**Overkoepelend open punt** *(indien van toepassing)*: {het ene punt dat over meerdere
scenario's heen speelt en in het slot wordt teruggebracht}.

---

## Bonus — leeg scenario voor zelf-invullen

> Optioneel. Als experts een eigen casus willen testen. **Niet verplicht alle
> variabelen langs te lopen** — vul alleen wat relevant is; de engine pakt voor de
> rest een standaardwaarde (0 of nee).

### Mini-sjabloon

```
Casus: ____________________________
Wie:       _____________________
Inkomen:   € _____ / mnd  (+ partner € _____ indien van toepassing)
Vermogen:  € _____ ( ____________ )
Aanslag/bedrag: € _____ ( ____________ )
Bijzonder: ________________________________________________
```

---

# Vervolg-sessie — werkvorm en draaiboek

**Vervolg op**: {eerste workshop-doc}
**Doel**: drie testcases langs de experts halen — per case toetsen of de digitale
beoordeling op hetzelfde uitkomt als de praktijk, en bij verschil helder krijgen hoe
zij rekenen.
**Deelnemers**: {2-4 experts} + facilitator. **Duur**: {±90 min}.

## Werkvorm-protocol per scenario (≈20 min)

1. **Lees voor + toon engine-uitkomst** (1 min) — direct de uitkomst noemen. Geen
   discussie nu.
2. **Stilte** (2 min) — ieder schrijft individueel op: *wat zou ik beslissen?* + *welk
   gegeven vond ik doorslaggevend?* (voorkomt dat een dominante stem kleurt).
3. **Pair-share** (3 min) — in tweetallen vergelijken.
4. **Plenair** (9 min) — uitkomsten ophalen, vergelijken met engine; bij **verschil
   doorvragen op de rekenmethode**, niet op de beslissing.
5. **Verdiepingspunten** (5 min) — de 2-3 specifieke vragen onder het scenario.

Time-box strak: bij oploop → parkeren naar het slot.

## Facilitator-tips
- **Stilte-eerst**; **engine-uitkomst vooraf delen** (anders dan blinde toets — hier
  is de cijfer-vergelijking de kern); **bij verschil doorvragen op de rekenstap**
  ("hoe komen jullie van A naar B?"); **niet over beleid argumenteren** — "anders
  rekenen" is informatie, niet onze rol om te bevestigen of dat mag.

## Wat NIET in deze sessie
Code-/YAML-uitleg · diepte van de cross-law-keten · live demo's die nog niet werken ·
brede behandeling van de kleinere open punten (alleen parkeren).

## Afronding
1 zin per persoon. Actiepunten (max 3-4: wat/wie/wanneer).
