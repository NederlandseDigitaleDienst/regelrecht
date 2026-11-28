# Wetgevings-fouten in het {stelsel/traject}

**Datum**: {datum} · **Bron**: {review-synthese / cyclus}
**Onderwerp**: fouten en gaten in de wetgeving die **niet door interpretatie te
repareren** zijn.

> Dit document beschrijft **wetgevings-bugs** — gevallen waarin de wet zelf onjuist,
> achterhaald of onuitvoerbaar is. Voor modellering-fouten die wij kunnen fixen, zie
> `{pad naar modellering-fixes-plan}`.
>
> Geadresseerd aan: {wetgevings-eigenaar} (eigenaar), {uitvoerder/medewetgever}.
> Bedoeld als bron voor een formele wetgevings-notitie.

## Samenvatting

| Categorie | Aantal | Ernst |
|---|---|---|
| Lege delegaties | {n} | KRITIEK |
| Achterhaalde institutionele referenties | {n} | KRITIEK |
| Wettekst-fouten | {n} | KRITIEK |
| Inconsistenties met andere wetten | {n} | ZORG |
| Onuitvoerbare voorwaarden zonder beslisser | {n} | ZORG |
| Onuitgewerkte regelingen | {n} | ZORG |
| {…overige categorieën} | {n} | {…} |

**Conclusie**: {is het stelsel uitvoerbaar zoals het er staat? Waar functioneert de
praktijk op gewoonterecht zonder formele basis? Wat betekent dat voor geautomatiseerde
uitvoering?}

---

## §{n}. {Categorie}

### {n.m} {Titel — wet + artikel/lid}
COMMENT: {ruimte voor menselijke reviewer-nuance — leeg laten bij eerste opstelling}

**Wettekst ({bron-id} {artikel})**:
> "{letterlijk citaat van de geldende tekst}"

**Probleem**: {waarom dit fout/achterhaald/onuitvoerbaar is}.

**Implicatie**: {gevolg voor uitvoering en burger — kan hij zijn recht
kennen/berekenen/betwisten? Maak concreet wat onuitvoerbaar wordt.}

```mermaid
%% optioneel: visualiseer het probleem (circulair criterium / dubbelfunctie / mismatch)
```

**Wat zou moeten gebeuren**: {concrete reparatie — technisch wijzigingsbesluit / AMvB
afkondigen / term vervangen / criterium toevoegen. Geef aan: klein technisch vs
beleidsinhoudelijk.}

*(herhaal per fout; geschrapte fouten doorstrepen met ~~…~~ + reden + telling-correctie)*

---

## §{n}. Wat is precies "uitvoeren" van dit stelsel?

Een burger kan vandaag **geen geautomatiseerd kenbare regel** vinden voor:
1. {…}
2. {…}

{Conclusie: de machine_readable is zo correct als de wet toelaat, maar de wet laat te
veel open om als geautomatiseerd uitvoerbaar stelsel te functioneren — tenzij
wetgevings-onderhoud volgt.}

---

## §{n}. Aanbevolen acties (prioriteits-volgorde)

1. **Reparatie wettekst-fouten** — technisch, onbetwist, zonder beleidsruimte.
2. **Afkondigen ontbrekende MR's/AMvB's** — juridisch noodzakelijk, beleidsinhoudelijk.
3. **Naam-modernisering institutionele referenties** — klein technisch.
4. **Beslisser-aanwijzing voor open normen** — beleidsinhoudelijke keuze.
5. {…}
