<script setup>
// Tab Toets in het kaartpaneel: kan op het gekozen punt een woontoren van N lagen?
// Tijdelijk en nieuw deel, elk gerekend door een eigen RegelRecht-engine; het eindoordeel komt
// uit artikel 22.29 van het omgevingsplan. Het punt komt van de kaart. Deze component rekent
// niets; zie toets/toets.js.
import { ref, watch } from 'vue';
import BouwplanInvoer from './BouwplanInvoer.vue';
import { maakEngine } from '../engine.js';
import { bouwplan } from '../toets/bouwplan.js';
import { aannames, aantalIngevuld, naarParams } from '../toets/aannames.js';
import { laadRegelingen } from '../toets/laden.js';
import { KORT } from '../toets/plannen.js';
import { rekenToets } from '../toets/toets.js';

const props = defineProps({
  stapel: { type: Object, default: null },
  rd: { type: Array, default: null },
});

const fout = ref(null);
const bezig = ref(false);
const uitkomst = ref(null);
const weergave = ref({ tijdelijk: 'stappen', nieuw: 'stappen' });
const vandaag = new Date().toISOString().slice(0, 10);

async function reken() {
  if (!props.stapel) { uitkomst.value = null; return; }
  bezig.value = true;
  try {
    const { index, varianten } = await laadRegelingen();
    uitkomst.value = await rekenToets({ stapel: props.stapel, bouwplan: { ...bouwplan }, aannames: naarParams(), datum: vandaag, index, varianten, maakEngine });
    fout.value = null;
  } catch (e) {
    fout.value = String(e?.message ?? e);
  } finally {
    bezig.value = false;
  }
}
watch([() => props.stapel, () => ({ ...bouwplan }), () => ({ ...aannames })], reken, { deep: true, immediate: true });

const KOLOM = { tijdelijk: 'Tijdelijk deel', nieuw: 'Nieuw deel' };
const OORDEEL = {
  toegestaan: { variant: 'success', tekst: 'Toegestaan' },
  'niet toegestaan': { variant: 'critical', tekst: 'Niet toegestaan' },
  onthouding: { variant: 'warning', tekst: 'Onthouding' },
};
const STATUS = {
  'niet-van-toepassing': { variant: 'neutral', tekst: 'Niet van toepassing' },
  'niet-gemodelleerd': { variant: 'neutral', tekst: 'Niet in YAML gezet' },
  'bron-ontbreekt': { variant: 'warning', tekst: 'Onthouding: bron ontbreekt of alleen als scan' },
  'geen-regels': { variant: 'neutral', tekst: 'Geen regels over bouwen' },
  'geen-plan': { variant: 'warning', tekst: 'Geen plan gevonden' },
  'geen-bestemming': { variant: 'neutral', tekst: 'Geen bestemming op dit punt' },
};
const TAG = { toegestaan: 'success', 'niet toegestaan': 'critical', onthouding: 'warning', fout: 'critical' };
// Ingangen van 22.29 die alleen de uitkomst van een plan doorgeven: niet zelf "ontbrekend".
const DOORGEEF = new Set(['bouwregels_ter_plaatse_voldaan', 'gebruiksregels_ter_plaatse_voldaan', 'aanvullende_regels_ter_plaatse_voldaan', 'parkeren_voldaan']);

const leesbaar = (naam) => naam.replaceAll('_', ' ');

// Ontbrekende gegevens in gewone taal: verwante parameters onder één noemer.
const GROEP = [
  [/welstand/, 'welstandsoordeel'],
  [/bodem|sanerende|toelaatbare_kwaliteit/, 'bodemgegevens'],
  [/parkeer|parkeren/, 'parkeren'],
  [/laad|laden_lossen/, 'laden en lossen'],
  [/afstand/, 'afstand tot de perceelsgrens'],
  [/puntensysteem/, 'oordeel puntensysteem'],
  [/hoogteaccent/, 'voorwaarden hoogteaccent'],
  [/sociale/, 'aandeel sociale woningen'],
];
const noemer = (n) => GROEP.find(([re]) => re.test(n))?.[1] ?? leesbaar(n);

/** Per artikel de gegevens die echt ontbreken, zonder de doorgegeven planuitkomsten. */
function ontbrekend(k) {
  const per = [];
  for (const s of k.stappen) {
    const namen = [...new Set(s.ontbrekend.filter((n) => !DOORGEEF.has(n)).map(noemer))];
    if (namen.length) per.push(`${KORT[s.regeling] ?? s.regeling} art. ${s.artikel} (${namen.join(', ')})`);
  }
  return per;
}

function banner(k) {
  if (k.status !== 'gerekend') return { ...STATUS[k.status], sub: k.toelichting };
  const o = k.eindoordeel;
  const basis = OORDEEL[o.uitkomst] ?? { variant: 'critical', tekst: o.fout ? 'Fout in de berekening' : 'Geen uitkomst' };
  const tekst = k.label === 'beoogd' ? `Beoogd: ${basis.tekst.toLowerCase()}` : basis.tekst;
  let sub = '';
  if (o.uitkomst === 'niet toegestaan') {
    sub = 'Past niet binnen de regels ter plaatse. Buiten het plan kan de gemeente toch een vergunning verlenen, met een eigen afweging (Omgevingswet 5.21, tweede lid, onder b).';
  } else if (o.uitkomst === 'onthouding') {
    const mist = ontbrekend(k);
    sub = `Niet te beoordelen zonder gegevens uit ${mist.join('; ')}${k.nietGemodelleerd?.length ? '; en de regels die hier nog niet in YAML staan' : ''}. Zie de stappen.`;
  } else if (o.uitkomst === 'toegestaan') {
    sub = 'Binnen het plan te vergunnen volgens artikel 22.29.';
  } else if (o.fout) {
    sub = o.fout;
  }
  return { variant: basis.variant, tekst, sub };
}

const traceTekst = (k) => k.stappen.map((s) => `# ${KORT[s.regeling] ?? s.regeling}, artikel ${s.artikel}: ${s.output}\n${s.trace ?? s.fout ?? ''}`).join('\n\n');
</script>

<template>
  <nldd-container layout="stack" gap="16" :class="{ bezig }">
    <nldd-title
      size="display-4"
      :heading-level="2"
      overline="Toets"
      :text="`Kan hier een woontoren van ${bouwplan.lagen} lagen?`"
      :supporting-text="rd ? `Punt RD ${Math.round(rd[0])}, ${Math.round(rd[1])}. Tijdelijk en nieuw deel naast elkaar. Verkenning, geen besluit.` : ''"
    ></nldd-title>

    <BouwplanInvoer />
    <nldd-banner
      v-if="aantalIngevuld()"
      variant="warning" :heading-level="3"
      :text="`Met ${aantalIngevuld()} aannames`"
      supporting-text="Ingevuld onder Simuleren, Gegevens invullen. Een aanname is geen feit: de uitkomst hieronder geldt alleen als die aannames kloppen."
    ></nldd-banner>

    <nldd-banner v-if="fout" variant="critical" text="De toets kon niet rekenen" :supporting-text="fout" :heading-level="3"></nldd-banner>
    <nldd-text v-if="!stapel" size="sm"><p>Klik op een plek op de kaart, of kies een testpunt.</p></nldd-text>

    <template v-if="uitkomst && stapel">
      <nldd-text size="sm">
        <p>
          Vergunningplicht (omgevingsplan 22.26 en 22.27):
          <strong>{{ uitkomst.vergunningplicht.waarde === true ? 'ja, een omgevingsvergunning is nodig' : uitkomst.vergunningplicht.uitkomst }}</strong>.
        </p>
      </nldd-text>

      <div class="kolommen">
        <nldd-container v-for="k in [uitkomst.tijdelijk, uitkomst.nieuw]" :key="k.kolom" layout="stack" gap="8">
          <nldd-title
            size="display-5"
            :heading-level="3"
            :text="KOLOM[k.kolom]"
            :overline="k.label === 'beoogd' ? 'Beoogd: ontwerp, geen geldend recht' : (k.label ? 'Geldend' : '')"
            :supporting-text="k.plan ? `${k.plan.naam}${k.bestemming ? ' · ' + k.bestemming : ''}` : ''"
          ></nldd-title>
          <nldd-banner :variant="banner(k).variant" :text="banner(k).tekst" :supporting-text="banner(k).sub" :heading-level="4" size="sm"></nldd-banner>

          <template v-if="k.status === 'gerekend'">
            <nldd-segmented-control :value="weergave[k.kolom]" size="sm" accessible-label="Weergave" @change="weergave[k.kolom] = $event.target.value">
              <nldd-segmented-control-item value="stappen" text="Stappen" :selected="weergave[k.kolom] === 'stappen' || undefined"></nldd-segmented-control-item>
              <nldd-segmented-control-item value="trace" text="Trace" :selected="weergave[k.kolom] === 'trace' || undefined"></nldd-segmented-control-item>
            </nldd-segmented-control>

            <nldd-list v-if="weergave[k.kolom] === 'stappen'" appearance="simple" accessible-label="Stappen">
              <nldd-list-item v-for="(s, i) in k.stappen" :key="i" size="sm">
                <nldd-text-cell
                  :overline="`${KORT[s.regeling] ?? s.regeling}, artikel ${s.artikel}`"
                  :text="leesbaar(s.output)"
                  :supporting-text="s.ontbrekend.filter((n) => !DOORGEEF.has(n)).length ? 'Ontbreekt: ' + [...new Set(s.ontbrekend.filter((n) => !DOORGEEF.has(n)).map(noemer))].join(', ') : (s.fout ?? '')"
                ></nldd-text-cell>
                <nldd-cell width="fit-content">
                  <nldd-tag :color="TAG[s.uitkomst] ?? 'neutral'" :text="s.uitkomst"></nldd-tag>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item v-for="(n, i) in k.nietGemodelleerd" :key="'n' + i" size="sm">
                <nldd-text-cell overline="Ligt hier ook, nog niet in YAML" :text="n"></nldd-text-cell>
                <nldd-cell width="fit-content"><nldd-tag color="warning" text="open"></nldd-tag></nldd-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-code-viewer v-else wrap max-height="420px" language="text">{{ traceTekst(k) }}</nldd-code-viewer>
          </template>
        </nldd-container>
      </div>
    </template>
  </nldd-container>
</template>

<style scoped>
.bezig { opacity: 0.6; transition: opacity 0.15s; }
.kolommen { display: grid; grid-template-columns: minmax(0, 1fr); gap: 20px; }
@container paneel (min-width: 720px) {
  .kolommen { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); align-items: start; }
}
</style>
