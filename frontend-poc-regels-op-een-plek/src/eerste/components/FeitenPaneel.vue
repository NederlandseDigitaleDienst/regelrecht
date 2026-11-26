<script setup>
// Toont de stapel op een punt (uit stapel.js): wat geldt, wat erbovenop komt, wat opzij
// gaat en waarom, wat beoogd is. Geen oordeel over de 5 lagen: dat is de toets.
// Opmaak met componenten uit het NLDD Designsysteem 0.9.0. Geen in- en uitklappen
// (ontwerprichtlijn "Vermijd expand/collapse"): alles staat in beeld, het paneel scrollt.
import { computed } from 'vue';

const props = defineProps({
  stapel: { type: Object, required: true },
  rd: { type: Array, required: true },
  // Opmaak van de lijsten. Over de kaart heen 'simple': zonder eigen vlak, zodat de kaart erdoor blijft schijnen.
  lijst: { type: String, default: 'box-base' },
});

const ROL = {
  basisplan: 'Basisplan', wijzigingsplan: 'Wijzigingsplan', uitwerkingsplan: 'Uitwerkingsplan',
  paraplu: 'Paraplu', facet: 'Facetplan', tam: 'TAM-plan', 'tam-ontwerp': 'Ontwerp-TAM',
};

const MELDING = {
  'bron-ontbreekt': { variant: 'critical', titel: 'Het geldende plan zit niet in de bronnen' },
  'geen-plan': { variant: 'critical', titel: 'Geen plan gevonden' },
  'alleen-plangrens': { variant: 'warning', titel: 'Alleen de plangrens is digitaal' },
  'gelijke-datum': { variant: 'warning', titel: 'Twee plannen met dezelfde datum' },
  'archeologie-open': { variant: 'warning', titel: 'Archeologie: open vraag' },
};

const geldend = computed(() => props.stapel.tam ?? props.stapel.basisplan);

function planRegel(plan) {
  return `${ROL[plan.rol]} · ${plan.planstatus} ${plan.datum}`;
}

function regelLink(plan, obj) {
  return obj?.verwijzing ? plan.regels + obj.verwijzing : plan.regels;
}

function waarden(m) {
  return (m.waarden ?? []).map((w) => `${(w.type ?? m.naam).replace(/^other: /, '')}: ${w.waarde}`).join('; ') || m.naam;
}

function namen(lijst) {
  return [...new Set(lijst.map((o) => o.naam))].join(', ');
}

const feitenRijen = computed(() => {
  const g = geldend.value;
  if (!g) return [];
  const f = g.feiten;
  const rijen = [];
  if (g.plan.alleen_plangrens) {
    // Oud plan (IMROPCP2008): geen bestemmingsvlakken in de GML, alleen pdf's.
    rijen.push({ label: 'Bestemming, bouwvlak, maatvoering', tekst: 'staan alleen op de plankaart (pdf)', link: g.plan.plankaart, linkTekst: 'Plankaart' });
    rijen.push({ label: 'Regels', tekst: 'pdf', link: g.plan.regels });
    return rijen;
  }
  for (const e of f.enkelbestemming) {
    rijen.push({
      label: props.stapel.tam ? 'Gebied of activiteit' : 'Bestemming',
      tekst: e.naam, extra: artikelTekst(e), link: regelLink(g.plan, e),
    });
  }
  if (!f.enkelbestemming.length) rijen.push({ label: 'Bestemming', tekst: 'geen' });
  rijen.push({ label: 'Dubbelbestemming', tekst: namen(f.dubbelbestemmingen) || 'geen' });
  rijen.push({ label: 'Bouwvlak', tekst: f.binnenBouwvlak ? 'binnen een bouwvlak' : 'niet binnen een bouwvlak' });
  if (f.maatvoering.length) for (const m of f.maatvoering) rijen.push({ label: 'Maatvoering', tekst: waarden(m) });
  else rijen.push({ label: 'Maatvoering', tekst: 'geen maatvoering op dit punt' });
  if (f.bouwaanduidingen.length) rijen.push({ label: 'Bouwaanduiding', tekst: namen(f.bouwaanduidingen) });
  if (f.functieaanduidingen.length) rijen.push({ label: 'Functieaanduiding', tekst: namen(f.functieaanduidingen) });
  if (f.gebiedsaanduidingen.length) rijen.push({ label: 'Gebiedsaanduiding', tekst: namen(f.gebiedsaanduidingen) });
  return rijen;
});

/** Artikel volgens de regeltekst; de tekst is leidend als de GML een ander nummer noemt. */
function artikelTekst(e) {
  if (e.tekst_ontbreekt) return `geen artikel in de regeltekst (de kaart noemt artikel ${e.artikel_gml})`;
  if (!e.artikel) return '';
  return e.artikel_gml ? `artikel ${e.artikel} (de kaart noemt artikel ${e.artikel_gml})` : `artikel ${e.artikel}`;
}
</script>

<template>
  <nldd-container layout="stack" gap="24">
    <nldd-title
      size="display-3"
      :heading-level="2"
      overline="Punt"
      :text="`RD ${rd[0].toFixed(0)}, ${rd[1].toFixed(0)}`"
      :supporting-text="`WGS84 ${stapel.punt[1].toFixed(6)}, ${stapel.punt[0].toFixed(6)}`"
    ></nldd-title>

    <nldd-banner
      v-for="(m, i) in stapel.meldingen"
      :key="i"
      :variant="MELDING[m.soort]?.variant ?? 'warning'"
      :text="MELDING[m.soort]?.titel ?? 'Let op'"
      :supporting-text="m.tekst"
      :heading-level="3"
    ></nldd-banner>

    <!-- Wat geldt -->
    <nldd-container layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="2" text="Tijdelijk deel: wat hier geldt"></nldd-title>
      <nldd-list v-if="geldend" :appearance="lijst" accessible-label="Geldend plan">
        <nldd-list-item>
          <nldd-title-cell size="display-5" :overline="planRegel(geldend.plan)" :text="geldend.plan.naam" :supporting-text="geldend.plan.grond"></nldd-title-cell>
          <nldd-cell width="fit-content">
            <nldd-link :href="geldend.plan.viewer" target="_blank" rel="noopener" text="Plan" end-icon="external-link"></nldd-link>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-text v-if="stapel.tam" size="sm">
        Geen bestemmingsplannen op dit punt. Alleen de bruidschat (hoofdstuk 22) blijft, voor zover niet in strijd met het
        TAM-plan. In de toets staan de regels van het TAM-plan in de kolom nieuw.
      </nldd-text>
      <nldd-text v-if="!geldend" size="sm" color="secondary">Geen geldend plan in de bronnen.</nldd-text>
    </nldd-container>

    <!-- Feiten -->
    <nldd-container v-if="geldend" layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="2" text="Feiten op het punt"></nldd-title>
      <nldd-list :appearance="lijst" accessible-label="Feiten op het punt">
        <nldd-list-item v-for="(r, i) in feitenRijen" :key="i" size="sm">
          <nldd-text-cell :overline="r.label" :text="r.tekst" :supporting-text="r.extra || ''"></nldd-text-cell>
          <nldd-cell v-if="r.link" width="fit-content">
            <nldd-link :href="r.link" target="_blank" rel="noopener" :text="r.linkTekst ?? 'Regels'"></nldd-link>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-container>

    <!-- Daarbovenop -->
    <nldd-container v-if="stapel.aanvullend.length" layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="2" text="Daarbovenop"></nldd-title>
      <nldd-list :appearance="lijst" accessible-label="Lagen daarbovenop">
        <template v-for="(a, i) in stapel.aanvullend" :key="i">
          <template v-if="a.soort === 'paraplu-artikelen'">
            <nldd-list-item v-for="ar in a.artikelen" :key="`${i}-${ar.nr}`" size="sm">
              <nldd-text-cell :overline="`${a.plan.naam}, artikel ${ar.nr}`" :text="ar.onderwerp" :supporting-text="`vervangt de regels van ${stapel.basisplan.plan.naam} volgens ${ar.tabel}`"></nldd-text-cell>
            </nldd-list-item>
          </template>
          <nldd-list-item v-else size="sm">
            <nldd-text-cell
              :overline="a.soort === 'archeologie' ? `${a.plan.naam}, archeologie` : `${a.plan.naam} (${ROL[a.plan.rol].toLowerCase()})`"
              :text="namen(a.objecten)"
              :supporting-text="a.twijfel ? 'Of dit onder dit basisplan geldt, is een open vraag.' : ''"
            ></nldd-text-cell>
            <nldd-cell v-if="a.twijfel" width="fit-content">
              <nldd-tag color="warning" text="open"></nldd-tag>
            </nldd-cell>
          </nldd-list-item>
        </template>
      </nldd-list>
    </nldd-container>

    <!-- Opzij -->
    <nldd-container v-if="stapel.opzij.length" layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="2" text="Op dit punt, maar opzij gezet"></nldd-title>
      <nldd-list :appearance="lijst" accessible-label="Opzij gezette plannen">
        <nldd-list-item v-for="o in stapel.opzij" :key="o.plan.id" size="sm">
          <nldd-text-cell :overline="planRegel(o.plan)" :text="o.plan.naam" :supporting-text="o.reden"></nldd-text-cell>
          <nldd-cell width="fit-content">
            <nldd-tag v-if="o.twijfel" color="warning" text="twijfel"></nldd-tag>
            <nldd-link v-else :href="o.plan.viewer" target="_blank" rel="noopener" text="Plan"></nldd-link>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-container>

    <!-- Beoogd -->
    <nldd-container v-if="stapel.beoogd" layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="2" :text="`Beoogd: ${stapel.beoogd.plan.naam}`"
        :supporting-text="`Ontwerp ${stapel.beoogd.plan.datum}. Geldt niet; telt als beoogd.`"></nldd-title>
      <nldd-banner v-if="stapel.beoogd.interferentie" variant="neutral" text="Interferentiegebied"
        supporting-text="Na vaststelling blijven hier de oude plannen gelden, met artikel 13 en 66 van het TAM-plan als aanvulling."
        :heading-level="3"></nldd-banner>
      <nldd-list :appearance="lijst" accessible-label="Beoogde regels op dit punt">
        <nldd-list-item v-for="e in stapel.beoogd.feiten.enkelbestemming" :key="e.id" size="sm">
          <nldd-text-cell overline="Gebied of activiteit" :text="e.naam" :supporting-text="artikelTekst(e)"></nldd-text-cell>
          <nldd-cell width="fit-content">
            <nldd-link :href="regelLink(stapel.beoogd.plan, e)" target="_blank" rel="noopener" text="Regels"></nldd-link>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="stapel.beoogd.feiten.bouwaanduidingen.length" size="sm">
          <nldd-text-cell overline="Bouwaanduiding" :text="namen(stapel.beoogd.feiten.bouwaanduidingen)"></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-if="stapel.beoogd.feiten.gebiedsaanduidingen.length" size="sm">
          <nldd-text-cell overline="Gebiedsaanduiding" :text="namen(stapel.beoogd.feiten.gebiedsaanduidingen)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-container>

    <!-- Nieuw deel -->
    <nldd-container layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="2" text="Nieuw deel omgevingsplan"></nldd-title>
      <nldd-text size="sm" color="secondary">
        Geldt overal, naast het tijdelijk deel; beide worden getoond. Afdeling 5.3 Bouwactiviteiten
        staat op "Gereserveerd": geen hoogteregel.
      </nldd-text>
    </nldd-container>
  </nldd-container>
</template>
