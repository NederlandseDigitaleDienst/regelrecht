<template>
  <div class="iwa">
    <div class="iwa-knoppen">
      <nldd-form-field
        :label="`Wat je per jaar verdient: ${euroWhole(inkomen)}`"
        supporting-label="voor belasting; schuif om een ander bedrag te proberen"
      >
        <input
          class="iwa-slider"
          type="range"
          :min="0"
          :max="maxInkomen"
          :step="100000"
          :value="inkomen"
          :aria-label="`Wat je per jaar verdient, nu ${euroWhole(inkomen)}`"
          @input="inkomen = Number($event.target.value)"
        />
      </nldd-form-field>

      <nldd-form-field
        v-if="persona.heeft_partner"
        :label="`Wat je partner per jaar verdient: ${euroWhole(partnerinkomen)}`"
        :supporting-label="partnerTelt ? 'dit telt mee' : 'dit telt niet mee, dat heb je zelf gevraagd'"
      >
        <input
          class="iwa-slider"
          type="range"
          :min="0"
          :max="maxInkomen"
          :step="100000"
          :value="partnerinkomen"
          :aria-label="`Inkomen van je partner, nu ${euroWhole(partnerinkomen)}`"
          @input="partnerinkomen = Number($event.target.value)"
        />
      </nldd-form-field>

      <nldd-button
        v-if="gewijzigd"
        text="Terug naar je eigen gegevens"
        start-icon="undo"
        appearance="neutral-transparent"
        size="sm"
        @click="herstel"
      ></nldd-button>
    </div>

    <div v-if="proef" class="iwa-uitkomst">
      <div class="iwa-vak">
        <span class="iwa-label">Maandbedrag</span>
        <strong class="iwa-waarde">{{ euro(proef.maandbedrag) }}</strong>
        <span v-if="gewijzigd" class="iwa-delta" :class="klasse(proef.maandbedrag, huidig.maandbedrag, true)">
          {{ deltaTekst(proef.maandbedrag, huidig.maandbedrag, 'euro') }}
        </span>
      </div>
      <div class="iwa-vak">
        <span class="iwa-label">Klaar met betalen</span>
        <strong class="iwa-waarde">{{ proef.einde }}</strong>
        <span v-if="gewijzigd && proef.einde !== huidig.einde" class="iwa-delta">
          was {{ huidig.einde }}
        </span>
      </div>
      <div class="iwa-vak">
        <span class="iwa-label">Hoef je niet te betalen</span>
        <strong class="iwa-waarde">{{ euroWhole(proef.kwijt) }}</strong>
        <span v-if="gewijzigd" class="iwa-delta" :class="klasse(proef.kwijt, huidig.kwijt, true)">
          {{ deltaTekst(proef.kwijt, huidig.kwijt, 'euro') }}
        </span>
      </div>
    </div>

    <!-- Peiljaarverlegging: zodra de wet hem toestaat, is het een keuze en
         geen mededeling. -->
    <section v-if="magVerleggen" class="iwa-verlegging">
      <h4 class="iwa-kop">Je verdient genoeg minder om een recenter jaar te vragen</h4>
      <p class="iwa-uitleg">
        DUO rekent nu met je inkomen van twee jaar geleden: {{ euroWhole(peiljaarinkomen) }}. Je verdient nu meer dan
        15 procent minder. Daarom mag DUO een recenter jaar gebruiken. Dat heet peiljaarverlegging.
      </p>

      <!-- Onder hoofdstuk 10a stelt DUO de draagkracht alleen op aanvraag
           vast (artikel 10a.7). Zonder die meting doet verlegging niets, dus
           bied hem hier aan in plaats van ernaar te verwijzen. -->
      <nldd-switch-field
        v-if="metingNodig"
        label="Vraag DUO om naar je inkomen te kijken"
        :checked="meting ? true : undefined"
        @change="meting = $event.detail?.checked ?? meting"
      ></nldd-switch-field>
      <span v-if="metingNodig" class="iwa-stand">
        {{ meting ? 'Aan: DUO kijkt naar je inkomen' : 'Uit: je betaalt een vast bedrag' }}
      </span>

      <nldd-switch-field
        label="Vraag DUO om een recenter jaar te gebruiken"
        :checked="verlegging ? true : undefined"
        :disabled="metingNodig && !meting ? true : undefined"
        @change="verlegging = $event.detail?.checked ?? verlegging"
      ></nldd-switch-field>
      <span class="iwa-stand">
        {{ metingNodig && !meting
          ? 'Dit kan pas als DUO naar je inkomen kijkt'
          : (verlegging ? 'Aan: dit vraag je aan' : 'Uit: DUO blijft met het oude jaar rekenen') }}
      </span>

      <p v-if="effect.zin" class="iwa-effect" :class="{ 'iwa-effect-winst': effect.soort === 'winst' }">
        {{ effect.zin }}
      </p>

      <ul v-if="effect.soort !== 'geen'" class="iwa-gerust">
        <li v-for="regel in VERLEGGING_GERUSTSTELLING" :key="regel">{{ regel }}</li>
      </ul>
    </section>

  </div>
</template>

<!--
  "Wat als mijn inkomen verandert." Het inkomensmodel van de engine kan dit
  al: bij een verlaging zetten we inkomen_voorheen en inkomensdaling_jaar,
  zodat het peiljaar (t-2) nog het oude, hogere inkomen laat zien. Precies
  daar bijt artikel 6.12, en daarom wijst de melding op peiljaarverlegging.
-->

<script setup>
import { ref, computed, watch } from 'vue';
import { runSimulation } from '../../composables/useSimulation.js';
import { inkomenInJaar } from '../../sim/simulate.js';
import { euro, euroWhole } from '../../lib/format.js';
import { draagkrachtmetingIsKeuze, verleggingEffect, VERLEGGING_GERUSTSTELLING } from '../../lib/burgerTekst.js';

const props = defineProps({
  persona: { type: Object, required: true },
  choices: { type: Object, default: () => ({}) },
  startJaar: { type: Number, default: 2026 },
});
const emit = defineEmits(['wijzigingen']);

const inkomen = ref(props.persona.inkomen ?? 0);
const partnerinkomen = ref(props.persona.partnerinkomen ?? 0);
// Peiljaarverlegging is een keuze, geen aanname: standaard uit, en uit
// betekent dat DUO met het peiljaar blijft rekenen.
const verlegging = ref(false);
// Onder hoofdstuk 10a (SF15-oud) stelt DUO de draagkracht alleen op aanvraag
// vast; onder hoofdstuk 6 gaat dat ambtshalve. Alleen in het eerste geval is
// dit een keuze die de burger moet maken.
const meting = ref(!!props.choices?.draagkrachtAangevraagd);

// Bij een andere persona weer met diens eigen gegevens beginnen.
watch(() => props.persona, (p) => {
  inkomen.value = p?.inkomen ?? 0;
  partnerinkomen.value = p?.partnerinkomen ?? 0;
  verlegging.value = false;
  meting.value = !!props.choices?.draagkrachtAangevraagd;
}, { deep: false });

/** Ruime bovengrens, zodat de schuif ook voor hoge inkomens bruikbaar blijft. */
const maxInkomen = computed(() => Math.max(10000000, Math.round((props.persona.inkomen ?? 0) * 2.5 / 1e6) * 1e6));

const partnerTelt = computed(() => props.choices?.partnerMeetellen !== false);

const gewijzigd = computed(
  () => inkomen.value !== (props.persona.inkomen ?? 0)
    || partnerinkomen.value !== (props.persona.partnerinkomen ?? 0),
);

function herstel() {
  inkomen.value = props.persona.inkomen ?? 0;
  partnerinkomen.value = props.persona.partnerinkomen ?? 0;
  verlegging.value = false;
  meting.value = !!props.choices?.draagkrachtAangevraagd;
}

function samenvatting(r) {
  if (!r) return null;
  const eerste = r.timeline[0];
  return {
    maandbedrag: eerste ? eerste.maandbedrag : 0,
    einde: r.totals.levenslang ? 'levenslang' : String(r.totals.eindejaar),
    kwijt: r.totals.kwijtgescholden,
    // Bindt de draagkracht? Zo niet, dan betaal je het volle termijnbedrag en
    // doet je inkomen niets voor je maandbedrag.
    draagkrachtBindt: eerste ? eerste.termijn_naar_draagkracht !== null : false,
    draagkrachtGevraagd: !!props.choices?.draagkrachtAangevraagd,
  };
}

const huidig = computed(() => samenvatting(runSimulation(props.persona, props.choices)) ?? { maandbedrag: 0, einde: '—', kwijt: 0 });

/**
 * Het proefrecord. Bij een verlaging leggen we de daling vast met
 * inkomen_voorheen en inkomensdaling_jaar, want dan blijft het peiljaar (t-2)
 * op het oude inkomen staan; dat is precies wat een debiteur in het echt ook
 * merkt, en wat peiljaarverlegging oplost.
 */
const proefPersona = computed(() => {
  const p = { ...props.persona, inkomen: inkomen.value, partnerinkomen: partnerinkomen.value };
  const origineel = props.persona.inkomen ?? 0;
  if (inkomen.value < origineel) {
    p.inkomen_voorheen = origineel;
    p.inkomensdaling_jaar = props.startJaar;
  } else {
    delete p.inkomen_voorheen;
    delete p.inkomensdaling_jaar;
  }
  // Eigen sleutel, anders komt het antwoord uit de cache van de echte persona.
  p.bsn = `${props.persona.bsn}::proef${inkomen.value}:${partnerinkomen.value}`;
  return p;
});

/** Keuzes van de proefberekening: de bestaande keuzes plus de schakelaars. */
const proefKeuzes = computed(() => ({
  ...props.choices,
  draagkrachtAangevraagd: meting.value,
  peiljaarverlegging: verlegging.value,
}));

/**
 * Moet deze debiteur de draagkrachtmeting zelf aanvragen? Dat is zo onder
 * hoofdstuk 10a (artikel 10a.7); onder hoofdstuk 6 stelt DUO de draagkracht
 * ambtshalve vast en is er niets te kiezen.
 */
const metingNodig = computed(() => {
  const sim = runSimulation(
    { ...proefPersona.value, bsn: `${proefPersona.value.bsn}::zondermeting` },
    { ...props.choices, draagkrachtAangevraagd: false },
  );
  return draagkrachtmetingIsKeuze(sim);
});

const proef = computed(() => samenvatting(runSimulation(proefPersona.value, proefKeuzes.value)));

/** Dezelfde situatie zónder verlegging, om het verschil te kunnen tonen. */
const proefZonder = computed(
  () => samenvatting(runSimulation(
    { ...proefPersona.value, bsn: `${proefPersona.value.bsn}::zonder` },
    { ...props.choices, draagkrachtAangevraagd: meting.value, peiljaarverlegging: false },
  )),
);

/** En mét, ook als de schakelaar uitstaat: zo weet je wat hij zou opleveren. */
const proefMet = computed(
  () => samenvatting(runSimulation(
    { ...proefPersona.value, bsn: `${proefPersona.value.bsn}::met` },
    { ...props.choices, draagkrachtAangevraagd: meting.value, peiljaarverlegging: true },
  )),
);

/** Het inkomen waar DUO nu mee rekent (peiljaar t-2). */
const peiljaarinkomen = computed(
  () => inkomenInJaar(proefPersona.value, props.startJaar, props.startJaar - 2),
);

/**
 * Mag deze debiteur verlegging aanvragen? De wet zegt: bij een daling van
 * ten minste 15 procent (artikel 6.12). De engine rekent dat zelf uit, dus
 * we nemen zijn oordeel over in plaats van de grens hier na te bouwen.
 */
const magVerleggen = computed(() => {
  const sim = runSimulation(proefPersona.value, proefKeuzes.value);
  return !!sim?.timeline?.[0]?.peiljaarverlegging_mogelijk;
});

/** Wat verlegging in dit geval doet, in woorden en op basis van de sim. */
const effect = computed(
  () => verleggingEffect(proefZonder.value, proefMet.value, meting.value),
);

const doorgeven = computed(() => {
  const uit = [];
  if (inkomen.value !== (props.persona.inkomen ?? 0)) {
    uit.push(`Je verdient ${euroWhole(inkomen.value)} per jaar (was ${euroWhole(props.persona.inkomen ?? 0)})`);
  }
  if (partnerinkomen.value !== (props.persona.partnerinkomen ?? 0)) {
    uit.push(`Je partner verdient ${euroWhole(partnerinkomen.value)} per jaar (was ${euroWhole(props.persona.partnerinkomen ?? 0)})`);
  }
  return uit;
});

const aanvragen = computed(() => {
  const uit = [];
  if (meting.value && !props.choices?.draagkrachtAangevraagd) {
    uit.push('DUO kijkt naar je inkomen om je maandbedrag te bepalen');
  }
  if (verlegging.value) uit.push('DUO rekent met een recenter jaar (peiljaarverlegging)');
  return uit;
});

// De knop staat één niveau hoger: alles wat je op deze pagina aanpast gaat in
// één aanvraag naar DUO. Dit blok meldt alleen wát het zou willen doorgeven.
watch([doorgeven, aanvragen], ([d, a]) => emit('wijzigingen', { doorgeven: d, aanvragen: a }), {
  immediate: true,
  deep: true,
});

function deltaTekst(a, b, soort) {
  if (a === b) return 'gelijk';
  const teken = a > b ? '+' : '−';
  const abs = Math.abs(a - b);
  return `${teken}${soort === 'euro' ? euro(abs) : abs}`;
}

/** beterLager: minder betalen is goed nieuws voor de debiteur. */
function klasse(a, b, beterLager) {
  if (a === b) return '';
  return (a < b) === beterLager ? 'iwa-goed' : 'iwa-slecht';
}
</script>

<style scoped>
.iwa { display: flex; flex-direction: column; gap: var(--primitives-space-16); }
.iwa-verlegging {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--primitives-space-8);
  padding: var(--primitives-space-16);
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-base-background-color);
}
.iwa-kop { margin: 0; font-size: 1em; font-weight: 600; }
.iwa-uitleg { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.iwa-stand { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.iwa-effect {
  margin: 0;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  font-size: 0.9em;
}
.iwa-effect-winst { color: var(--semantics-content-success-color); }
.iwa-gerust {
  margin: 0;
  padding-left: var(--primitives-space-20);
  font-size: 0.85em;
  color: var(--semantics-content-secondary-color);
}
.iwa-gerust li + li { margin-top: var(--primitives-space-4); }
/* Elk label hoort bij de lijst eronder: kop en lijst dicht op elkaar, lucht
   tussen de paren. De eerste kop plakt niet tegen de bovenrand. */
.iwa-knoppen { display: flex; flex-direction: column; gap: var(--primitives-space-12); align-items: flex-start; }
.iwa-slider { width: min(420px, 100%); accent-color: var(--semantics-content-accent-color); }
.iwa-uitkomst {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: var(--primitives-space-16);
}
.iwa-vak { display: flex; flex-direction: column; gap: var(--primitives-space-4); }
.iwa-label { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.iwa-waarde { font-size: 1.4em; font-variant-numeric: tabular-nums; }
.iwa-delta { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.iwa-goed { color: var(--semantics-content-success-color); }
.iwa-slecht { color: var(--semantics-content-critical-color); }
</style>
