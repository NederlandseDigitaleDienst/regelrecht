<script setup>
// Op dit punt: beide sporen naast elkaar, in de vorm van de Toets-tab van de eerste PoC (titel,
// banner met het oordeel, lijst met stappen). Links vóór DSO, rechts met DSO. Onderaan de trace van
// het DSO-spoor op dit vak, opnieuw gerekend in de browser.
import { computed, ref, watch } from 'vue';
import Uitklap from './Uitklap.vue';
import { BANNER, BUNDEL, OVERHEID, TAG, TEKST, UIT_KLASSE, klasseDso, klasseOud, klasseTotaal, kortId } from '../data.js';
import { laadMotor, traceVak } from '../sim.js';

const props = defineProps({ cel: Object, index: Number, scenario: Object, weergave: String, data: Object });
const emit = defineEmits(['artikel']);

const KORT = { parapluplan_2022: 'Parapluplan 2022', omgevingsplan_veenendaal: 'Omgevingsplan' };
const kortRegeling = (r) => KORT[r] ?? String(r).replace(/^bestemmingsplan_/, '').replaceAll('_', ' ');
const LAAG = {
  basisplan: 'basisplan', 'herzien-ter-plaatse': 'herzien ter plaatse, valt weg', facet: 'facetplan, komt erbij', tam: 'TAM-plan, vastgesteld',
  beoogd: 'ontwerp-TAM, beoogd', paraplu: 'paraplu, volgens zijn tabellen', 'ontwerp-opgevolgd': 'ontwerp, al vastgesteld',
  'vervallen-voorbereidingsbesluit': 'voorbereidingsbesluit, vervallen', 'ontwerp-niet-meegeteld': 'ontwerp, telt niet mee',
};
const NIET_MEEGETELD = new Set(['ontwerp-niet-meegeteld', 'ontwerp-opgevolgd', 'vervallen-voorbereidingsbesluit', 'plangrens-zonder-object']);

const beoogd = computed(() => props.weergave === 'beoogd');
const kOud = computed(() => {
  const o = props.cel.oud; if (!o) return null;
  return beoogd.value || o.stapel.tam ? o.nieuw : o.tijdelijk;
});
const plannenOud = computed(() => {
  const st = props.cel.oud?.stapel; if (!st) return [];
  return [['basisplan', st.basisplan], ['TAM-plan, vastgesteld', st.tam], ['ontwerp-TAM, beoogd', st.beoogd],
    ...[...new Set(st.aanvullend ?? [])].map((a) => ['komt erbij', a])].filter(([, v]) => v);
});
const oordeelOud = computed(() => {
  if (props.cel.oud) return klasseOud(props.cel, props.weergave);
  const u = props.scenario?.oud?.uitkomst;
  return u ? (UIT_KLASSE[u] ?? u) : null;
});

const d = computed(() => props.cel.dso);
const kDso = computed(() => (beoogd.value || d.value.plannen.some((p) => p.laag === 'tam') ? d.value.nieuw : d.value.tijdelijk));
const plannenDso = computed(() => d.value.plannen.filter((p) => !NIET_MEEGETELD.has(p.laag)));
const genegeerd = computed(() => d.value.plannen.filter((p) => NIET_MEEGETELD.has(p.laag) && p.laag !== 'plangrens-zonder-object'));
const totaal = computed(() => klasseTotaal(props.cel, props.weergave));
const andere = computed(() => Object.entries(d.value.andere).map(([o, v]) => ({ o: OVERHEID[o], ...v })));
const perOverheid = computed(() => ['gemeente', 'provincie', 'waterschap', 'rijk']
  .filter((o) => d.value.artikelen[o]?.length)
  .map((o) => {
    const per = {};
    for (const [k, u] of d.value.artikelen[o]) { const m = props.data.artikelen[k]; (per[m.rt || m.r] ??= []).push({ k, u, m }); }
    const groepen = Object.entries(per).map(([r, xs]) => ({ r, xs: xs.sort((a, b) => a.m.a.localeCompare(b.m.a, 'nl', { numeric: true })) }));
    return { o, naam: OVERHEID[o], aantal: d.value.artikelen[o].length, groepen };
  }));
const landelijk = computed(() => Object.entries(d.value.telling).filter(([k]) => k.startsWith('rijk|toets')).map(([, n]) => n)[0] ?? 0);

function banner(k) {
  return BANNER[k] ?? { variant: 'neutral', tekst: 'Geen uitkomst' };
}
const kenmerk = (m) => [m.rol, ...m.b.map((b) => BUNDEL[b] ?? b), m.g, m.v && m.v !== 'GEGEVEN' ? m.v.toLowerCase() : (m.v ? 'als gegeven' : '')].filter(Boolean).join(' · ');
const deelTekst = (deel) => `Bouwregels ${deel.bouwregels}, gebruik ${deel.gebruiksregels}, overige regels ${deel.aanvullend}.`;
const verwachtKlasse = computed(() => UIT_KLASSE[props.scenario?.verwacht?.uitkomst] ?? props.scenario?.verwacht?.uitkomst);

// --- Trace: pas rekenen als het blok opengaat, en opnieuw bij een ander vak.
const traceOpen = ref(false);
const trace = ref(null); // per overheid: { o, naam, tekst, aantal }
const traceFout = ref(null);
const traceBezig = ref(false);
const JA_NEE = (w) => (w === true ? 'ja' : w === false ? 'nee' : 'onbekend');
async function rekenTrace() {
  if (!traceOpen.value || props.index == null || props.index < 0) return;
  const i = props.index;
  traceBezig.value = true; traceFout.value = null; trace.value = null;
  try {
    const m = await laadMotor();
    const aanroepen = traceVak(m, i, props.cel.rd);
    if (i !== props.index) return;
    const per = {};
    for (const a of aanroepen) {
      const meta = props.data.artikelen[a.sleutel];
      const o = meta?.o ?? 'onbekend';
      const kop = meta ? `${meta.rt}, artikel ${meta.a}${meta.t ? ` (${meta.t})` : ''}` : a.law;
      (per[o] ??= []).push(`# ${kop}\n# ${a.output} = ${JA_NEE(a.waarde)}\n# invoer: ${JSON.stringify(a.params)}\n${a.trace}`);
    }
    trace.value = ['gemeente', 'provincie', 'waterschap', 'rijk', 'onbekend'].filter((o) => per[o])
      .map((o) => ({ o, naam: OVERHEID[o] ?? 'Overig', aantal: per[o].length, tekst: per[o].join('\n\n') }));
  } catch (e) {
    traceFout.value = String(e?.message ?? e);
  } finally {
    if (i === props.index) traceBezig.value = false;
  }
}
watch([traceOpen, () => props.index], rekenTrace);
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-title
      size="display-5" :heading-level="2"
      :text="scenario ? scenario.naam : `Vak bij RD ${Math.round(cel.rd[0])}, ${Math.round(cel.rd[1])}`"
      :overline="beoogd ? 'Beoogd recht: ontwerp-TAM, geen geldend recht' : 'Geldend recht'"
      :supporting-text="scenario ? `RD ${cel.rd[0]}, ${cel.rd[1]} · ${scenario.reeks === 'besluiten' ? 'scenario uit een besluit' : 'blind scenario woontoren'}` : 'Gestapeld woongebouw, 5 lagen van 3,2 m, 20 woningen. Open gegevens gunstig.'"
    ></nldd-title>

    <nldd-banner
      v-if="scenario"
      variant="info" size="sm" :heading-level="3"
      :text="`Verwacht: ${scenario.verwacht.uitkomst}`"
      :supporting-text="[scenario.besluit ? `Besluit ${scenario.besluit}.` : '', scenario.verwacht.weigeringsgrond ? `Weigeringsgrond ${kortId(scenario.verwacht.weigeringsgrond.regeling)} ${scenario.verwacht.weigeringsgrond.artikel}.` : '', scenario.buiten_bereik ? `Buiten bereik van beide sporen: ${scenario.buiten_bereik}.` : ''].filter(Boolean).join(' ')"
    ></nldd-banner>

    <div class="kolommen">
      <!-- Vóór DSO -->
      <nldd-container layout="stack" gap="8" class="kolom oud">
        <nldd-title size="display-5" :heading-level="3" text="Vóór DSO" overline="Eerste PoC" supporting-text="Eigen downloads, lagen en regels met de hand. Alleen de gemeente."></nldd-title>
        <nldd-banner :variant="banner(oordeelOud).variant" :text="banner(oordeelOud).tekst" size="sm" :heading-level="4"
          :supporting-text="scenario && verwachtKlasse ? (oordeelOud === verwachtKlasse ? 'Zoals verwacht.' : 'Anders dan verwacht.') : ''"></nldd-banner>

        <template v-if="cel.oud">
          <nldd-text size="sm"><strong>Plannen</strong></nldd-text>
          <nldd-list appearance="simple" accessible-label="Plannen vóór DSO">
            <nldd-list-item v-for="([rol, id], i) in plannenOud" :key="i" size="sm">
              <nldd-text-cell :text="kortId(id)" :supporting-text="rol"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text size="sm"><strong>Gerekende artikelen ({{ kOud?.stappen?.length ?? 0 }})</strong></nldd-text>
          <nldd-list v-if="kOud?.stappen?.length" appearance="simple" accessible-label="Stappen vóór DSO">
            <nldd-list-item v-for="(s, i) in kOud.stappen" :key="i" size="sm">
              <nldd-text-cell :text="`${kortRegeling(s.regeling)}, artikel ${s.artikel}`"></nldd-text-cell>
              <nldd-cell width="fit-content"><nldd-tag :color="TAG[s.uitkomst] ?? 'neutral'" :text="TEKST[s.uitkomst] ?? s.uitkomst"></nldd-tag></nldd-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text v-else size="sm" color="secondary">{{ kOud?.status === 'geen-regels' ? 'Geen regels over bouwen in dit deel.' : 'Niets gerekend.' }}</nldd-text>
        </template>
        <template v-else-if="scenario?.oud?.gerekend">
          <nldd-text size="sm"><strong>Gerekende artikelen ({{ scenario.oud.gerekend.length }})</strong></nldd-text>
          <nldd-list appearance="simple" accessible-label="Stappen vóór DSO">
            <nldd-list-item v-for="(g, i) in scenario.oud.gerekend" :key="i" size="sm">
              <nldd-text-cell :text="kortId(g)"></nldd-text-cell>
              <nldd-cell v-if="scenario.oud.weigering.includes(g)" width="fit-content"><nldd-tag color="critical" text="weigering"></nldd-tag></nldd-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text v-if="scenario.oud.open?.length" size="xs" color="secondary">Nog open: {{ scenario.oud.open.join('; ') }}</nldd-text>
        </template>
        <nldd-text v-else size="sm" color="secondary">Geen vak van de eerste PoC op deze plek.</nldd-text>
        <nldd-text size="xs" color="secondary">Provincie, waterschap en rijk: niet gerekend.</nldd-text>
      </nldd-container>

      <!-- Met DSO -->
      <nldd-container layout="stack" gap="8" class="kolom dso">
        <nldd-title size="display-5" :heading-level="3" text="Met DSO" overline="DSO-spoor" supporting-text="Plannen, regels en teksten uit het DSO. Alle overheden."></nldd-title>
        <nldd-banner :variant="banner(klasseDso(cel, weergave)).variant" :text="`Gemeente: ${banner(klasseDso(cel, weergave)).tekst.toLowerCase()}`" size="sm" :heading-level="4"
          :supporting-text="kDso.reden ?? (kDso.deel ? deelTekst(kDso.deel) : '')"></nldd-banner>
        <div v-if="!beoogd" class="rij">
          <nldd-text size="sm">Alle overheden samen</nldd-text>
          <nldd-tag :color="TAG[totaal] ?? 'neutral'" :text="TEKST[totaal] ?? totaal"></nldd-tag>
        </div>

        <nldd-text size="sm"><strong>Plannen, gelaagdheid afgeleid uit de gegevens</strong></nldd-text>
        <nldd-list appearance="simple" accessible-label="Plannen met DSO">
          <nldd-list-item v-for="p in plannenDso" :key="p.id + p.laag" size="sm">
            <nldd-text-cell :text="p.naam || kortId(p.id)" :supporting-text="LAAG[p.laag] ?? p.laag"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-if="genegeerd.length" size="xs" color="secondary">Wel in het DSO, telt niet mee: {{ genegeerd.map((p) => `${p.naam || kortId(p.id)} (${LAAG[p.laag] ?? p.laag})`).join(', ') }}.</nldd-text>
        <nldd-banner v-for="(o, i) in cel.open" :key="'o' + i" variant="warning" size="sm" :text="o"></nldd-banner>

        <nldd-text size="sm"><strong>Andere overheden</strong></nldd-text>
        <nldd-list appearance="simple" accessible-label="Andere overheden">
          <nldd-list-item v-for="a in andere" :key="a.o" size="sm">
            <nldd-text-cell :text="a.o" :supporting-text="`${a.gerekend} artikelen gerekend`"></nldd-text-cell>
            <nldd-cell width="fit-content"><nldd-tag :color="TAG[a.uitkomst] ?? 'neutral'" :text="TEKST[a.uitkomst] ?? a.uitkomst"></nldd-tag></nldd-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-text size="sm"><strong>Wat je moet aanvragen of melden</strong></nldd-text>
        <nldd-list v-if="d.plichten.length" appearance="simple" accessible-label="Plichten">
          <nldd-list-item v-for="(p, i) in d.plichten" :key="i" size="sm">
            <nldd-text-cell :overline="OVERHEID[p.overheid]" :text="p.tekst" :supporting-text="p.grond"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-else size="sm" color="secondary">Geen.</nldd-text>
      </nldd-container>
    </div>

    <nldd-container layout="stack" gap="8">
      <nldd-title size="display-5" :heading-level="3" text="Artikelen uit het DSO op deze plek" supporting-text="Bij dit bouwplan, per overheid. Klik op een artikel voor de tekst."></nldd-title>
      <details v-for="g in perOverheid" :key="g.o" class="overheid" :open="g.o === 'gemeente' || undefined">
        <summary><nldd-text size="sm"><strong>{{ g.naam }}</strong> ({{ g.aantal }})</nldd-text></summary>
        <template v-for="grp in g.groepen" :key="grp.r">
          <nldd-text size="xs" color="secondary" class="regeling">{{ grp.r }}</nldd-text>
          <nldd-list appearance="simple" :accessible-label="grp.r">
            <nldd-list-item v-for="x in grp.xs" :key="x.k" size="sm" button @click="emit('artikel', x.k)">
              <nldd-text-cell :text="x.m.t || `Artikel ${x.m.a}`" :supporting-text="kenmerk(x.m)"></nldd-text-cell>
              <nldd-cell v-if="x.u && x.u !== 'toegestaan'" width="fit-content"><nldd-tag :color="TAG[x.u] ?? 'neutral'" :text="TEKST[x.u] ?? x.u"></nldd-tag></nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
      </details>
      <nldd-text size="xs" color="secondary">Rijk: alleen artikelen die op deze plek bijzonder zijn; {{ landelijk }} landelijke artikelen horen bij het bouwplan en gelden overal.</nldd-text>
    </nldd-container>

    <Uitklap v-model:open="traceOpen" titel="Trace van het DSO-spoor" sub="Wat de engine per artikel deed: invoer, stappen en uitkomst. Opnieuw gerekend in je browser, met 5 lagen van 3,2 m en open gegevens gunstig.">
      <nldd-text v-if="traceBezig" size="sm" color="secondary">Engine en regels laden, en rekenen…</nldd-text>
      <nldd-banner v-else-if="traceFout" variant="critical" size="sm" text="De trace kon niet worden gerekend" :supporting-text="traceFout" :heading-level="4"></nldd-banner>
      <template v-else-if="trace">
        <details v-for="t in trace" :key="t.o" class="overheid" :open="t.o === 'gemeente' || undefined">
          <summary><nldd-text size="sm"><strong>{{ t.naam }}</strong> ({{ t.aantal }} {{ t.aantal === 1 ? 'aanroep' : 'aanroepen' }})</nldd-text></summary>
          <nldd-code-viewer wrap max-height="420px" language="text">{{ t.tekst }}</nldd-code-viewer>
        </details>
        <nldd-text v-if="!trace.length" size="sm" color="secondary">Hier rekent het DSO-spoor geen artikelen.</nldd-text>
      </template>
    </Uitklap>
  </nldd-container>
</template>

<style scoped>
.kolommen { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; align-items: start; }
@container paneel (max-width: 620px) { .kolommen { grid-template-columns: 1fr; } }
.kolom { min-width: 0; padding-top: 8px; border-top: 4px solid; }
.kolom.oud { border-top-color: #8a8a86; }
.kolom.dso { border-top-color: #01689b; }
.rij { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.overheid summary { cursor: pointer; padding: 4px 0; }
.overheid summary nldd-text { display: inline-block; }
.regeling { display: block; margin-top: 6px; }
</style>
