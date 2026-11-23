# Input — wat de skill verwacht en hoe je het detecteert

De skill start typisch vanaf een **casus-map met een scope-manifest + verwijzingen
naar de machine-readable YAML's** in een corpus. Casussen verschillen per traject;
de structuur hieronder is het generieke patroon.

## Wat je nodig hebt

1. **Scope-manifest** — welke wetten/regelingen en welke artikelen in scope zijn,
   met hun rol in de keten en het pad naar de YAML. Vaak een `scope.yaml` of een
   tabel in een analyse-document. Bevat per wet minimaal: law-id, laag (A/B/C/D),
   rol, YAML-pad.

2. **Corpus-YAML's** — de machine-readable bestanden zelf (geversioneerd per
   inwerkingtredingsdatum). Hieruit haal je: outputs, actions/formules, parameters,
   inputs, `source:`-koppelingen, `definitions`, `untranslatables`, `overrides`,
   `legal_basis` (incl. per-formule `explanation`-quotes).

3. **Wettekst-bron-URL's** — de officiële publicatie per wet/artikel (voor
   wettekst-excerpts en links in de audit-docs). Vaak afleidbaar uit de YAML-metadata.

## Hoe je detecteert wat er is

- Zoek in de casus-map naar een scope-manifest (`*scope*`, of een analyse-doc met
  een wetten-tabel).
- Zoek de genoemde YAML-paden op in het corpus; verifieer dat ze bestaan en lees de
  relevante artikelen.
- Als er al een formules-afgeleide (bijv. een gegenereerde `formules.md`) bestaat,
  gebruik die als secundaire bron naast de YAML.
- Controleer of de engine een trace kan produceren (voor scenario-uitkomsten en het
  orakel) — zo niet, vraag de analist om uitkomsten of reken op de wettekst.

## Wat als het scope-manifest ontbreekt

Help het eerst opstellen. Loop het corpus af voor de wetten die de casus raken,
classificeer ze in lagen (A/B/C/D — zie `method-glossary.md`), en leg de relaties
vast (source-calls, legal_basis, overrides). Dit ís feitelijk de eerste helft van
het scope-analyse-product; lever het meteen in dat formaat op.

## Minimaal startpunt

Als alleen één wet/artikel + YAML beschikbaar is, kun je nog steeds een
per-artikel audit-doc maken. Scope-analyse en de keten-producten vereisen meer dan
één wet in beeld.
