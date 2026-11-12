<template>
  <!-- Levert deze keuze nu niets op, dan alleen een regel: de mogelijkheid
       bestaat, maar hij hoort niet als knop tussen de bruikbare keuzes. -->
  <p v-if="nietZinvol" class="keuze-regel">
    <strong>{{ meta.vraag }}</strong>
    {{ meta.geenReden ?? 'Dit levert je nu niets op.' }}
  </p>

  <nldd-card v-else class="keuze">
    <nldd-container padding="20" gap="12">
      <nldd-title heading-level="3" size="display-4">
        <span>{{ meta.vraag }}</span>
      </nldd-title>
      <p class="keuze-uitleg">{{ meta.uitleg }}</p>

      <!-- Jokerjaren: pauze van 1, 2 of 5 jaar -->
      <div v-if="choice === 'jokerjaren'" class="keuze-control">
        <nldd-dropdown size="sm" @change="onJokerChange">
          <select :value="String(modelJoker)" aria-label="Lengte van de pauze">
            <option value="0">Geen pauze</option>
            <option value="12">1 jaar pauze</option>
            <option value="24">2 jaar pauze</option>
            <option value="60">5 jaar pauze</option>
          </select>
        </nldd-dropdown>
      </div>
      <!-- Overige keuzes: aan/uit. Uit betekent niets doen, en dat staat er ook. -->
      <div v-else class="keuze-control">
        <nldd-switch-field
          :label="meta.actie"
          :checked="modelOn"
          @change="onToggle"
        ></nldd-switch-field>
        <span class="keuze-stand">{{ modelOn ? 'Aan: dit vraag je aan' : NIETS_DOEN }}</span>
      </div>

      <p class="keuze-effect" :class="{ 'keuze-effect-actief': isActief && !geenEffect }">
        {{ effectZin }}
      </p>

      <nldd-banner v-if="meta.waarschuwing" variant="warning">
        {{ meta.waarschuwing }}
      </nldd-banner>
    </nldd-container>
  </nldd-card>
</template>

<script setup>
import { computed, watch } from 'vue';
import { runSimulation } from '../../composables/useSimulation.js';
import { useLawStore } from '../../engine/lawStore.js';
import { keuzeEffectZin } from '../../lib/burgerTekst.js';

const props = defineProps({
  persona: { type: Object, required: true },
  choice: { type: String, required: true },
  baseChoices: { type: Object, required: true },
});
const emit = defineEmits(['update', 'zinvol']);

const { version } = useLawStore();

/**
 * Niets doen is ook een keuze, en op deze pagina de standaard. De schakelaar
 * uit betekent "ik laat het zoals het is"; dat hoort er te staan in plaats
 * van alleen de naam van de actie.
 */
const NIETS_DOEN = 'Niets doen (laat het zoals het is)';

// Per keuze: welke sleutel + de vraag/uitleg/actie in B1, tweede persoon.
const META = {
  overstap: {
    key: 'overstapAangevraagd',
    vraag: 'Wil je overstappen naar de nieuwe regels?',
    uitleg: 'Je kunt DUO vragen of de nieuwere regels voor jou mogen gelden.',
    actie: 'Ja, overstappen',
    geenReden: 'Overstappen levert jou nu niets op.',
  },
  draagkrachtmeting: {
    key: 'draagkrachtAangevraagd',
    vraag: 'Wil je minder per maand betalen?',
    uitleg: 'Verdien je niet veel? Dan kun je vragen om een bedrag dat past bij wat je kunt betalen.',
    actie: 'Ja, kijk wat ik kan betalen',
    geenReden: 'Een bedrag dat bij je inkomen past, is niet lager dan wat je nu betaalt.',
  },
  peiljaarverlegging: {
    key: 'peiljaarverlegging',
    vraag: 'Is je inkomen gedaald?',
    uitleg: 'DUO rekent met je inkomen van twee jaar geleden. Verdien je nu veel minder? Dan mag DUO een recenter jaar gebruiken.',
    actie: 'Ja, gebruik een recenter jaar',
    geenReden: 'Je verdient nu niet 15% minder dan twee jaar geleden. Dat is wel nodig.',
  },
  jokerjaren: {
    key: 'jokerMaanden',
    vraag: 'Wil je een pauze in je aflossing?',
    uitleg: 'Je mag een tijd stoppen met betalen. Dat heet jokerjaren. Je betaalt in die tijd wel rente.',
    actie: 'Pauze inplannen',
  },
  partner_meetellen: {
    key: 'partnerMeetellen',
    vraag: 'Wil je dat het inkomen van je partner niet meetelt?',
    uitleg: 'Bij jouw regels mag je vragen om het inkomen van je partner niet mee te tellen.',
    actie: 'Ja, partnerinkomen niet meetellen',
    waarschuwing:
      'Let op: je betaalt dan elk jaar langer door. En aan het eind moet je alles zelf betaald hebben.',
  },
};

const meta = computed(() => META[props.choice]);

const modelOn = computed(() => {
  if (props.choice === 'partner_meetellen') return props.baseChoices.partnerMeetellen === false;
  return !!props.baseChoices[meta.value.key];
});
const modelJoker = computed(() => props.baseChoices.jokerMaanden ?? 0);
const isActief = computed(() =>
  props.choice === 'jokerjaren' ? modelJoker.value > 0 : modelOn.value,
);

function choicesWith(value) {
  const c = { ...props.baseChoices };
  if (props.choice === 'partner_meetellen') {
    c.partnerMeetellen = value ? false : true; // "aan" = niet meetellen
  } else if (props.choice === 'jokerjaren') {
    c.jokerMaanden = value;
  } else {
    c[meta.value.key] = value;
  }
  return c;
}

function summarize(result) {
  if (!result) {
    return {
      maandbedragNu: null, eindejaar: null, kwijtgescholden: 0,
      levenslang: false, totaalBetaald: 0, totaalRente: 0,
    };
  }
  const { timeline, totals } = result;
  return {
    maandbedragNu: timeline.length ? timeline[0].maandbedrag : null,
    eindejaar: totals.eindejaar,
    kwijtgescholden: totals.kwijtgescholden,
    levenslang: totals.levenslang,
    totaalBetaald: totals.totaalBetaald,
    totaalRente: totals.totaalRente,
  };
}

/** Twee uitkomsten zijn "gelijk" als niets voor de burger merkbaar verandert. */
function gelijk(a, b) {
  return (
    a.maandbedragNu === b.maandbedragNu &&
    a.eindejaar === b.eindejaar &&
    a.kwijtgescholden === b.kwijtgescholden &&
    a.levenslang === b.levenslang
  );
}

// Basissituatie (zonder deze keuze aan) om het effect tegen af te zetten.
const baseSummary = computed(() => {
  version.value;
  const uit = props.choice === 'jokerjaren' ? 0 : false;
  return summarize(runSimulation(props.persona, choicesWith(uit)));
});

// Uitkomst mét de keuze aan.
const metSummary = computed(() => {
  version.value;
  const value = props.choice === 'jokerjaren' ? (modelJoker.value || 24) : true;
  return summarize(runSimulation(props.persona, choicesWith(value)));
});

// Verandert er niets voor de burger? Dan een eerlijke regel i.p.v. het effect.
const geenEffect = computed(() => gelijk(baseSummary.value, metSummary.value));

const effectZin = computed(() =>
  geenEffect.value
    ? `Dit levert je nu niets op. ${meta.value.geenReden ?? ''}`.trim()
    : keuzeEffectZin(baseSummary.value, metSummary.value, props.choice),
);

/**
 * Een keuze die niets verandert hoort niet als knop tussen de bruikbare
 * keuzes te staan: dan vraag je iets aan waar DUO werk aan heeft en jij
 * niets. Helemaal weglaten is ook niet goed, want dan weet je niet of de
 * mogelijkheid niet bestaat of alleen nu niet helpt. Dus: één regel onder de
 * kaarten, met de reden. Staat de keuze al aan, dan blijft de kaart staan,
 * anders kun je hem niet meer uitzetten.
 */
const nietZinvol = computed(() => geenEffect.value && !isActief.value);

// De ouder zet de bruikbare keuzes in het raster en de rest eronder.
watch(nietZinvol, (v) => emit('zinvol', props.choice, !v), { immediate: true });

function onToggle(event) {
  const checked = event.detail?.checked ?? event.target?.checked;
  emit('update', choicesWith(checked));
}
function onJokerChange(event) {
  const value = Number(event.detail?.value ?? event.target?.value ?? 0);
  emit('update', choicesWith(value));
}
</script>

<style scoped>
.keuze { width: 100%; }
.keuze-stand { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.keuze-regel { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.keuze-regel strong { color: var(--semantics-content-color); font-weight: 600; }
.keuze-uitleg {
  margin: 0;
  color: var(--semantics-content-secondary-color);
  line-height: 1.5;
}
.keuze-control { display: flex; flex-direction: column; align-items: flex-start; gap: var(--primitives-space-4); }
.keuze-effect {
  margin: 0;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  line-height: 1.5;
  color: var(--semantics-content-color);
}
.keuze-effect-actief {
  background: var(--primitives-color-hemelblauw-50);
}
</style>
