# Definition-of-clean — de leak-discipline

Alles wat deze skill genereert of toevoegt aan een publiek-bestemde laag (deze skill, de
referentie-template) moet **casus-agnostisch** zijn. De gegenereerde PoC en zijn ingevulde
service-spec zijn casus-specifiek en horen **uitsluitend** in privé-ruimte (privé-repo,
fictieve data).

## De vier categorieën die nooit in een publieke laag mogen

1. **Casus-onderbouwingen** — domein-vocabulaire of voorbeelden die naar één casus wijzen,
   ook geparafraseerd.
2. **Casus-namen** — namen van wetten/regelingen/trajecten die een specifieke casus aanduiden.
3. **Persoonsnamen** — inclusief persona-aanduidingen.
4. **Organisatie-/functienamen** — organisaties, bestuursorganen, functietitels.

Generieke methode-/rollentermen mogen wél (bv. *behandelaar, jurist, ontwerper,
uitvoeringsexpert, corpus, engine, untranslatable, PoC als concept*) — mits ze geen
specifieke casus verraden.

## Gelaagde borging (van sterk naar aanvullend)

1. **Structurele scheiding (primair)** — casus-specifieke waarden (law-ids, param-/output-
   namen, bedragen, onderbouwingen) bestaan alleen in het privé-corpus + de gegenereerde
   (privé) PoC. De publieke lagen hebben er **geen slot** voor: alleen placeholders/config-
   sleutels, at-runtime gevuld. Dan is er categorisch niets te lekken.
2. **Vorm-heuristiek** — scan op de *vorm* van casus-inhoud, ongeacht welke casus:
   identifier-achtige nummerreeksen, code-patronen, law-id-vormen, bedragen in
   voorbeeldposities, "aanhef + Hoofdletternaam" voor personen.
3. **Allowlist van placeholders** — definieer wat in voorbeeld-/configposities wél mag
   (`<law-id>`, `<param>`, een dummy-corpus, de generieke rollen); vlag de rest.
4. **Semantisch vangnet** — een adversariële review (mens of LLM) die geparafraseerde/
   impliciete verwijzingen vangt die regex mist.

## Enforcement

- Plaats publieke artefacten zo dat de repo-leak-guard ze scant (`.claude/skills/…`).
- De guard is een **denylist** (snelle eerste filter) en draait in **pre-commit én CI** —
  dus de PR wordt server-side gecontroleerd, niet alleen lokaal.
- Een denylist is reactief; combineer 'm daarom met laag 1–4 hierboven. Voeg per nieuw
  traject de concrete markers toe aan de denylist.

## Checklist vóór publiceren

- [ ] Geen treffers op de vier categorieën (handmatig + guard).
- [ ] Voorbeelden gebruiken placeholders / een dummy-corpus, geen echte waarden.
- [ ] Geen verwijzing naar een specifieke build, sessie of traject.
- [ ] Semantische review uitgevoerd (laag 4) bij twijfel.
