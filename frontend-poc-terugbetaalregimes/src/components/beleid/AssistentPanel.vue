<template>
  <div class="assistent">
    <nldd-banner v-if="health === 'onbereikbaar'" variant="accent">
      De assistent-backend draait niet. Start de assistent met <code>just poc-assistent terugbetaalregimes</code> in een tweede terminal.
    </nldd-banner>
    <nldd-banner v-else-if="health === 'geen-cli'" variant="warning">
      De backend draait, maar de Claude Code CLI is niet gevonden. Installeer die en herstart met <code>just poc-assistent terugbetaalregimes</code>.
    </nldd-banner>

    <!-- Doel staat voor en is de standaard: dat is de vraag waarmee een
         beleidsmaker binnenkomt, en de modus waar het optimalisatiepad
         hieronder voor bestaat. -->
    <nldd-segmented-control size="sm" :value="modus" @change="modus = $event.detail?.value ?? modus">
      <nldd-segmented-control-item value="vraag" text="Vraag"></nldd-segmented-control-item>
      <nldd-segmented-control-item value="doel" text="Doel"></nldd-segmented-control-item>
      <nldd-segmented-control-item value="instructie" text="Instructie"></nldd-segmented-control-item>
    </nldd-segmented-control>


    <optimization-path-chart
      v-if="modus === 'doel' && pad.length"
      :measurements="pad"
      :series="reeksen"
      :axis-titles="['%']"
      :axis-formatters="[(v) => `${v}%`]"
      :selected="gekozenPunt"
      @select="kiesPunt"
    />

    <!-- Wat er op de aangeklikte meting stond, met de mogelijkheid die
         tussenstand over te nemen zonder op convergentie te wachten. -->
    <div v-if="gekozenStand" class="as-punt">
      <div class="as-punt-kop">
        <strong>Meting {{ gekozenStand.iteration }}</strong>
        <span>{{ percent(gekozenStand.pct, 1) }} betalingsproblemen<span v-if="gekozenStand.n">, n={{ gekozenStand.n }}</span></span>
        <nldd-icon-button icon="remove" size="sm" accessible-label="Sluit deze meting" @click="gekozenPunt = null"></nldd-icon-button>
      </div>
      <ul v-if="gekozenStand.wijzigingen?.length" class="as-punt-lijst">
        <li v-for="w in gekozenStand.wijzigingen" :key="`${w.artikel}:${w.naam}`">
          art. {{ w.artikel }} · <code>{{ w.naam }}</code>: {{ w.oud }} → {{ w.nieuw }}
        </li>
      </ul>
      <p v-else class="as-hint">Op deze meting was er nog niets gewijzigd.</p>
      <p v-if="gekozenStand.structuurGewijzigd?.length" class="as-hint">
        Ook de structuur van {{ gekozenStand.structuurGewijzigd.length === 1 ? 'een document' : `${gekozenStand.structuurGewijzigd.length} documenten` }}
        is herschreven; die wijziging staat niet als losse waarden in deze lijst.
      </p>
      <nldd-button
        v-if="gekozenStand.wijzigingen?.length"
        size="sm"
        appearance="secondary"
        start-icon="save"
        :text="`Neem deze stand over in ${werkversieLabel}`"
        @click="neemStandOver(gekozenStand)"
      ></nldd-button>
    </div>

    <div v-if="feed.length || streaming || afronding" ref="feedEl" class="as-feed">
      <div v-for="(item, i) in feed" :key="i" class="as-item" :class="`as-${item.type}`">
        <span v-if="item.type === 'tekst'" class="as-md" v-html="eenvoudigeMarkdown(item.tekst)"></span>
        <template v-else-if="item.type === 'tool'">
          🔧 {{ item.regel }}
        </template>
        <template v-else-if="item.type === 'wijziging'">
          ✏️ <strong>{{ item.document_key }}</strong>: {{ item.toelichting }}
        </template>
        <template v-else-if="item.type === 'simulatie'">
          📊 Simulatie ({{ item.doel }}<span v-if="item.n">, n={{ item.n }}</span>):
          {{ item.pct !== null ? percent(item.pct, 1) + ' betalingsproblemen' : 'klaar' }}
        </template>
        <template v-else-if="item.type === 'vraag'">{{ item.vraag }}</template>
        <template v-else-if="item.type === 'gebruiker'">{{ item.tekst }}</template>
        <template v-else-if="item.type === 'fout'">⚠️ {{ item.melding }}</template>
      </div>

      <!-- Wat er nu gebeurt, onderaan het gesprek: op de plek waar het
           volgende bericht komt, zoals de "denkt na"-regel in elke chat. Boven
           de feed stond hij los van waar je kijkt. -->
      <div v-if="streaming || afronding" class="as-status">
        <span class="as-status-wat">
          <nldd-icon v-if="openVraag" icon="help" size="16"></nldd-icon>
          <nldd-icon v-else-if="afronding" icon="checked" size="16"></nldd-icon>
          <nldd-activity-indicator v-else size="16" timing="instant"></nldd-activity-indicator>
          <span>{{ openVraag ? 'Wacht op jouw keuze.' : statusWat }}</span>
        </span>
        <span v-if="!openVraag && statusTeller" class="as-status-teller">{{ statusTeller }}</span>
      </div>

      <!-- De openstaande keuze staat onderaan het gesprek, op dezelfde plek
           waar het antwoord van de assistent zou komen: een vraag van de
           assistent is een beurt, geen waarschuwing boven het scherm. -->
      <div v-if="openVraag" class="as-keuze">
        <p v-if="openVraag.toelichting" class="as-keuze-toelichting">{{ openVraag.toelichting }}</p>
        <div class="as-opties">
          <template v-if="openVraag.meerkeuze">
            <!-- Het gevolg staat in het label zelf: nldd-checkbox-field heeft
                 geen supporting-label, en zonder gevolg is het geen keuze die
                 een beleidsmaker kan maken. -->
            <nldd-checkbox-field
              v-for="(o, i) in openVraag.opties"
              :key="i"
              :label="o.gevolg ? `${o.label} — ${o.gevolg}` : o.label"
              :checked="aangevinkt.includes(o.label) ? true : undefined"
              @change="vinkAan(o.label, $event)"
            ></nldd-checkbox-field>
            <nldd-button
              size="sm"
              appearance="primary"
              text="Doorgeven"
              :disabled="!aangevinkt.length ? true : undefined"
              @click="beantwoord(aangevinkt)"
            ></nldd-button>
          </template>
          <template v-else>
            <nldd-button
              v-for="(o, i) in openVraag.opties"
              :key="i"
              size="sm"
              :appearance="i === 0 ? 'primary' : 'secondary'"
              :text="o.gevolg ? `${o.label} — ${o.gevolg}` : o.label"
              @click="beantwoord([o.label])"
            ></nldd-button>
          </template>
        </div>
      </div>
    </div>

    <nldd-banner v-if="overlays && !Object.keys(overlays).length" variant="accent">
      De assistent heeft niets gewijzigd.
    </nldd-banner>
    <nldd-button
      v-if="overlays && Object.keys(overlays).length"
      :text="`Overnemen in ${werkversieLabel}`"
      start-icon="save"
      appearance="secondary"
      @click="takeOverlays"
    ></nldd-button>
    <p v-if="overlays && Object.keys(overlays).length" class="as-hint">
      Zet de wijziging van de assistent als bewerking in de werkversie ({{ Object.keys(overlays).length }}
      {{ Object.keys(overlays).length === 1 ? 'document' : 'documenten' }}); zichtbaar via Bekijk wijzigingen, terug te
      draaien met Terugzetten, te bewaren met Bewaar als variant.
    </p>

    <!-- Het invoerveld staat onder het gesprek, zoals bij elke andere
         assistent: je leest naar beneden en typt onderaan. Eén veld voor de
         eerste opdracht en voor alles wat daarna komt; een apart vak voor een
         vervolgbericht was een tweede plek om hetzelfde te doen. -->
    <nldd-form-field
      :label="loopt ? 'Stuur er iets achteraan' : { vraag: 'Vraag', doel: 'Doel', instructie: 'Instructie' }[modus]"
      :supporting-label="loopt ? 'komt binnen bij de volgende beurt van de assistent' : modusUitleg"
    >
      <nldd-multi-line-text-field
        :value="prompt"
        :placeholder="loopt ? 'Let ook op de kwijtscheldingskosten' : placeholder"
        :rows="loopt ? '2' : '3'"
        @input="prompt = $event.detail?.value ?? prompt"
      ></nldd-multi-line-text-field>
    </nldd-form-field>

    <!-- Voorbeelden per modus. Open tot de assistent voor het eerst gebruikt
         is: wie het blok nooit openklapt ziet ze nooit, en dat is precies de
         bezoeker voor wie ze bedoeld zijn. De toelichting per regel zegt wát je
         te zien krijgt, zodat de lijst ook uitlegt hoe de drie modi verschillen. -->
    <details v-if="!loopt && !feed.length" class="as-voorbeelden" :open="voorbeeldenOpen">
      <summary @click.prevent="voorbeeldenOpen = !voorbeeldenOpen">
        Voorbeelden ({{ voorbeelden.length }})
      </summary>
      <nldd-list appearance="simple">
        <nldd-list-item
          v-for="(v, i) in voorbeelden"
          :key="i"
          size="sm"
          button
          @click="kiesVoorbeeld(v)"
        >
          <nldd-text-cell size="sm" :text="v.tekst" :supporting-text="v.toelichting"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </details>

    <div class="as-knoppen">
      <nldd-button
        text="Verstuur"
        start-icon="send"
        appearance="primary"
        :disabled="!beschikbaar || !prompt.trim() || (loopt && !!openVraag)"
        @click="verstuur"
      ></nldd-button>
      <nldd-button v-if="loopt" text="Stop" start-icon="remove" appearance="secondary" @click="stop"></nldd-button>
    </div>
  </div>
</template>

<script setup>
import { formatToolCall } from '@regelrecht/frontend-shared/formatToolCall';
import { watch, ref, computed, nextTick, onMounted, onUnmounted } from 'vue';
import { useAssistent } from '../../composables/useAssistent.js';
import { useLawStore } from '../../engine/lawStore.js';
import { percent } from '../../lib/format.js';
import { resolveToken } from '../../lib/tokens.js';
import { b } from '../../basePad.js';
import { VOORBEELDEN } from '../../lib/assistentVoorbeelden.js';
import { persistedRef } from '../../composables/useBewaardeStand.js';
import OptimizationPathChart from '@regelrecht/frontend-shared/components/OptimizationPathChart.vue';

// Geen `metrics`-emit meer. De assistent meet met een eigen n op een eigen
// tussenstand; die cijfers naast de tegels zetten zou de doorrekening van de
// gebruiker stil overschrijven met een tussenmeting. Ze horen thuis in de
// feed en in het optimalisatiepad hieronder, en nergens anders.
const {
  streaming, run, hervat, stuur, antwoord, abort,
  // Gedeelde state: leeft buiten dit paneel, zodat een routewissel het gesprek
  // niet wist. Zie de kop van useAssistent.
  gesprekId, feed, voortgang, afronding, openVraag, overlays,
  modus, prompt, pad, gekozenPunt,
  meld, vraagNotificatieToestemming,
} = useAssistent();
const {
  applyOverlays, applyDefinitionChange, docPathVoorKey,
  lawDocsFor, werkversie, werkversieLabel,
} = useLawStore();

/** Minimale markdown voor de assistenttekst: vet, code, regeleinden; de rest blijft tekst. */
function eenvoudigeMarkdown(tekst) {
  const esc = String(tekst ?? '').replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
  return esc
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\n/g, '<br>');
}

function stop() {
  abort();
  feed.value.push({ type: 'tekst', tekst: 'Afgebroken.' });
}

// 'onbekend' (nog niet gepeild) | 'onbereikbaar' | 'geen-cli' | 'ok'
const health = ref('onbekend');
const beschikbaar = computed(() => health.value === 'ok');
let healthTimer = null;

async function peilHealth() {
  try {
    const res = await fetch(b('/api/health'));
    if (!res.ok) throw new Error();
    const body = await res.json();
    health.value = body.cli ? 'ok' : 'geen-cli';
  } catch {
    health.value = 'onbereikbaar';
  }
  // Blijf peilen als de backend (nog) niet bereikbaar is, zodat "just server"
  // starten in een tweede terminal het paneel vanzelf ontgrendelt.
  if (health.value !== 'ok' && !healthTimer) {
    healthTimer = setInterval(async () => {
      await peilHealth();
      if (health.value === 'ok') {
        clearInterval(healthTimer);
        healthTimer = null;
      }
    }, 5000);
  }
}

const feedEl = ref(null);

const placeholder = computed(() => ({
  vraag: 'Welk regime is voor een debiteur met een laag inkomen het gunstigst?',
  doel: 'Minimaliseer het aantal debiteuren met betalingsproblemen zonder de kwijtscheldingskosten meer dan te verdubbelen',
  instructie: 'Verhoog de draagkrachtvrije voet van SF15-oud naar 84% van het belastbaar minimumloon',
}[modus.value]));

/** Eén reeks: het aandeel debiteuren met betalingsproblemen, in procenten. */
const reeksen = [{
  key: 'pct',
  label: 'betalingsproblemen',
  color: resolveToken('--semantics-content-accent-color', '#154273'),
  formatter: (v) => `${v}%`,
}];

/** Laatste voortgangsmelding van de backend, en de afronding na afloop. */

/** De keuze die nu voorligt, als de assistent er een stelde. */
const aangevinkt = ref([]);

/**
 * Loopt er een gesprek? Zo ja, dan stuurt het invoerveld een vervolgbericht in
 * plaats van een nieuwe opdracht. De knop blijft "Verstuur": het is
 * dezelfde handeling, en wát je verstuurt staat al in het label boven het veld.
 */
const loopt = computed(() => streaming.value);

/** De voorbeelden die bij de gekozen modus horen. */
const voorbeelden = computed(() => VOORBEELDEN[modus.value] ?? []);

/**
 * Staat het voorbeeldenblok open? Open tot de assistent voor het eerst is
 * gebruikt; daarna weet de bezoeker wat hier kan en is het blok in de weg.
 * Het onthouden loopt via de bewaarde stand, dus het overleeft een ververs.
 */
const voorbeeldenGebruikt = persistedRef('assistent.voorbeeldenGezien', false);
const voorbeeldenOpen = ref(!voorbeeldenGebruikt.value);

/** Zet het voorbeeld in het veld; versturen doet de gebruiker zelf. */
function kiesVoorbeeld(v) {
  prompt.value = v.tekst;
  voorbeeldenOpen.value = false;
}

/**
 * Eén knop voor twee dingen: loopt er nog niets, dan start dit een gesprek;
 * loopt er wel iets, dan gaat de tekst als bericht naar de lopende beurt.
 */
async function verstuur() {
  if (loopt.value) return stuurVervolg();
  return submit();
}

function vinkAan(label, ev) {
  const aan = ev?.detail?.checked ?? ev?.target?.checked;
  aangevinkt.value = aan
    ? [...new Set([...aangevinkt.value, label])]
    : aangevinkt.value.filter((l) => l !== label);
}

async function beantwoord(keuzes) {
  const vraag = openVraag.value;
  if (!vraag) return;
  openVraag.value = null;
  aangevinkt.value = [];
  // Het antwoord zet de assistent weer aan het werk; zie stuurVervolg.
  afronding.value = null;
  try {
    await antwoord(vraag.id, keuzes);
  } catch (e) {
    // Valt de verbinding weg terwijl de vraag openstaat, dan is het gesprek
    // voorbij en heeft terugzetten van de keuze geen zin: er is niemand meer
    // om de keuze aan te geven. Zeg dat, in plaats van een knop te tonen die het
    // opnieuw niet doet.
    const weg = !streaming.value;
    feed.value.push({
      type: 'fout',
      melding: weg
        ? 'De verbinding met de assistent is weggevallen, dus je keuze kon niet meer aankomen. Stel de vraag opnieuw.'
        : `Antwoord doorgeven mislukt: ${e?.message ?? e}`,
    });
    if (!weg) openVraag.value = vraag;
  }
}

async function stuurVervolg() {
  const tekst = prompt.value.trim();
  if (!tekst) return;
  prompt.value = '';
  // Een nieuwe beurt begint: de afronding van de vorige hoort weg, anders staat
  // er "Klaar" boven een assistent die net weer begonnen is.
  afronding.value = null;
  try {
    await stuur(tekst);
  } catch (e) {
    feed.value.push({ type: 'fout', melding: `Versturen mislukt: ${e?.message ?? e}` });
    prompt.value = tekst;
  }
}

/** Namen van de tools zoals een beleidsmaker ze zou noemen. */
const TOOLTEKST = {
  lees_regelgeving: 'leest de regelgeving',
  wijzig_definitie: 'past een waarde aan',
  wijzig_regelgeving: 'herschrijft een artikel',
  lees_handelingen: 'leest het uitvoeringslastmodel',
  wijzig_handelingen: 'past het uitvoeringslastmodel aan',
  simuleer_personas: "rekent de persona's door",
  simuleer_populatie: 'rekent de populatie door',
};

/**
 * De statusregel staat in twee stukken, zodat hij niet midden in een zin
 * afbreekt: links wat hij doet, rechts de teller. Op een smalle kolom valt de
 * teller als geheel naar de volgende regel in plaats van "beurt" en "4 · 25 s"
 * uit elkaar te trekken.
 */
const statusWat = computed(() => {
  if (afronding.value) return 'Klaar';
  const v = voortgang.value;
  if (!v) return 'Bezig…';
  const wat = v.tool ? (TOOLTEKST[v.tool] ?? v.tool)
    : v.status === 'requesting' ? 'denkt na'
      : 'bezig';
  return `${wat[0].toUpperCase()}${wat.slice(1)}`;
});

const statusTeller = computed(() => {
  const beurtLabel = (n) => `${n} ${n === 1 ? 'beurt' : 'beurten'}`;
  if (afronding.value) {
    const { beurten, seconden } = afronding.value;
    return `${duur(seconden)} · ${beurtLabel(beurten)}`;
  }
  const v = voortgang.value;
  if (!v) return '';
  return v.beurten ? `${duur(v.seconden)} · ${beurtLabel(v.beurten)}` : duur(v.seconden);
});

function duur(seconden) {
  const s = Number(seconden) || 0;
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  const rest = s % 60;
  return rest ? `${m} min ${rest} s` : `${m} min`;
}

/** Index van het aangeklikte punt in `pad`, of null. */
const gekozenStand = computed(() => (gekozenPunt.value === null ? null : pad.value[gekozenPunt.value] ?? null));

function kiesPunt(index) {
  gekozenPunt.value = gekozenPunt.value === index ? null : index;
}

/**
 * Neem de stand van één meting over in de werkversie. Dat is niet de eindstand
 * van de assistent maar een tussenstap, en dat is precies waar dit voor is: een
 * pad loopt soms door een uitkomst heen die je beter bevalt dan waar hij
 * uitkomt.
 */
function neemStandOver(stand) {
  const mislukt = [];
  for (const w of stand.wijzigingen) {
    // De sleutel van de assistent is "id@valid_from"; de store vindt daar zijn
    // documentpad bij.
    const pad = docPathVoorKey(w.document_key);
    if (!pad) { mislukt.push(`${w.naam} (document ${w.document_key} niet gevonden)`); continue; }
    try {
      applyDefinitionChange(pad, w.artikel, w.naam, w.nieuw);
    } catch (e) {
      mislukt.push(`${w.naam} (${e?.message ?? e})`);
    }
  }
  const aantal = stand.wijzigingen.length - mislukt.length;
  feed.value.push({
    type: mislukt.length ? 'fout' : 'tekst',
    tekst: `Meting ${stand.iteration} overgenomen in ${werkversieLabel.value}: ${aantal} van de ${stand.wijzigingen.length} wijzigingen.`,
    melding: mislukt.length ? `Niet overgenomen: ${mislukt.join('; ')}.` : undefined,
  });
  gekozenPunt.value = null;
}

/**
 * Wat de twee modi van elkaar onderscheidt, in één regel. Zonder dit is het
 * verschil alleen uit het gedrag af te leiden: doel rekent iteratief naar een
 * uitkomst toe en mag daar zestig beurten over doen, instructie voert één
 * wijziging uit en laat het effect zien.
 */
const modusUitleg = computed(() => ({
  vraag: 'Je wilt iets weten. De assistent zoekt het uit en rekent door, maar wijzigt niets.',
  doel: 'Je weet wat je wilt bereiken. De assistent probeert wijzigingen, meet het effect en stelt bij tot het niet verder verbetert.',
  instructie: 'Je weet wat je wilt veranderen. De assistent voert die ene wijziging uit en rekent door.',
}[modus.value]));

onMounted(() => {
  peilHealth();
  // Het gesprek van voor de routewissel staat er al; zet het meteen onderaan,
  // want daar staat het laatste bericht.
  scrollNaarBeneden();
  // Loopt er nog een gesprek van voor de routewissel? Haak er weer op aan; de
  // backend stuurt eerst wat er gemist is en gaat daarna live verder.
  if (gesprekId.value && !streaming.value) {
    hervat(gesprekId.value, verwerkEvent).then((gelukt) => {
      if (!gelukt) rondGesprekAf();
    });
  }
});

// Staat het paneel dicht, dan heeft de feed geen hoogte en doet scrollen
// niets. Zodra hij er is (het paneel klapt open), alsnog naar beneden.
watch(feedEl, (el) => {
  if (el) scrollNaarBeneden();
});

// En als het browsertabblad weer de aandacht krijgt. Een verborgen tabblad
// krijgt zijn berichten wel binnen, maar het scrollen erbij landt op een
// pagina die niemand ziet; sommige browsers rekenen er dan ook niet goed mee.
function opZichtbaar() {
  if (document.visibilityState === 'visible') scrollNaarBeneden();
}
document.addEventListener('visibilitychange', opZichtbaar);
onUnmounted(() => {
  document.removeEventListener('visibilitychange', opZichtbaar);
  if (healthTimer) clearInterval(healthTimer);
});

async function submit() {
  // Toestemming voor notificaties vragen we hier en niet bij het laden: de
  // browser weigert de prompt buiten een klik om, en een prompt zodra je
  // binnenkomt is in een demo voor OCW lelijk.
  vraagNotificatieToestemming();
  feed.value = [];
  pad.value = [];
  gekozenPunt.value = null;
  overlays.value = null;
  voortgang.value = null;
  afronding.value = null;
  openVraag.value = null;
  aangevinkt.value = [];
  // De bezoeker heeft de assistent nu gebruikt; de voorbeelden mogen voortaan
  // dicht.
  voorbeeldenGebruikt.value = true;
  voorbeeldenOpen.value = false;

  // De assistent werkt op de werkversie: stuur die documenten mee als beginstand.
  const documenten = (await lawDocsFor(werkversie.value)).map((d) => ({ key: `${d.entry.id}@${d.entry.valid_from ?? ''}`, yaml: d.yaml }));
  // De opdracht zelf als eerste bericht in het gesprek, en het veld leeg: je
  // moet er iets nieuws in kunnen typen zonder eerst te wissen.
  const opdracht = prompt.value.trim();
  prompt.value = '';
  feed.value.push({ type: 'gebruiker', tekst: opdracht });
  feed.value.push({ type: 'tekst', tekst: `Werkt op ${werkversieLabel.value}.` });
  await run({ modus: modus.value, prompt: opdracht, documenten }, verwerkEvent);
  rondGesprekAf();
}

/**
 * Verwerk één bericht uit de stream. Staat los van `submit`, want
 * hervatten na een routewissel voert dezelfde berichten langs dezelfde
 * verwerking.
 */
function verwerkEvent(ev) {
    if (ev.type === 'voortgang') {
      voortgang.value = ev;
    } else if (ev.type === 'vraag') {
      // Hier staat de assistent stil tot er iemand antwoordt, dus dit is de
      // melding die het meest oplevert.
      meld('question', 'De beleidsassistent wacht op een keuze.');
      openVraag.value = ev;
      aangevinkt.value = [];
      feed.value.push({ type: 'vraag', vraag: ev.vraag });
    } else if (ev.type === 'vraag_verlopen') {
      // Niemand antwoordde; de assistent kiest zelf verder en zegt dat erbij.
      if (openVraag.value?.id === ev.id) openVraag.value = null;
      feed.value.push({ type: 'tekst', tekst: 'Geen antwoord gegeven; de assistent kiest zelf verder.' });
    } else if (ev.type === 'gebruiker') {
      // De eerste opdracht staat er al (submit zet hem erin); alleen wat daarna
      // wordt ingestuurd komt hier nog bij.
      const laatste = feed.value[feed.value.length - 1];
      if (!(laatste?.type === 'gebruiker' && laatste.tekst === ev.tekst)) {
        feed.value.push({ type: 'gebruiker', tekst: ev.tekst });
      }
    } else if (ev.type === 'gesprek') {
      // alleen het id; useAssistent houdt het bij
    } else if (ev.type === 'tekst_deel') {
      // Tekst terwijl die getypt wordt. De losse stukjes gaan in één regel in
      // de feed; het complete `tekst`-event erna vervangt die regel, zodat er
      // niet twee keer hetzelfde komt te staan.
      const laatste = feed.value[feed.value.length - 1];
      if (laatste?.type === 'tekst' && laatste.deels) laatste.tekst += ev.tekst;
      else feed.value.push({ type: 'tekst', tekst: ev.tekst, deels: true });
    } else if (ev.type === 'tekst') {
      const laatste = feed.value[feed.value.length - 1];
      if (laatste?.deels) feed.value[feed.value.length - 1] = { type: 'tekst', tekst: ev.tekst };
      else feed.value.push(ev);
    } else if (ev.type === 'tool') {
      feed.value.push({ type: 'tool', regel: formatToolCall(ev.naam, ev.input) });
    } else if (ev.type === 'simulatie') {
      const pct = ev.metrics?.pctBetalingsprobleem ?? null;
      feed.value.push({ type: 'simulatie', doel: ev.doel, n: ev.n, pct });
      if (ev.doel === 'populatie' && pct !== null) {
        pad.value.push({
          iteration: pad.value.length + 1,
          values: { pct: Math.round(pct * 1000) / 10 },
          // De stand waarop deze meting rust, zodat het punt aanklikbaar is.
          pct,
          n: ev.n ?? null,
          wijzigingen: ev.wijzigingen ?? [],
          structuurGewijzigd: ev.structuurGewijzigd ?? [],
        });
      }
    } else if (ev.type === 'beurt_klaar' || ev.type === 'klaar') {
      // Wie op een andere pagina staat hoort dit te weten; daar is het paneel
      // niet in beeld en blijft hij anders wachten op iets dat al gebeurd is.
      meld('done', ev.type === 'klaar'
        ? 'De beleidsassistent is klaar.'
        : 'De beleidsassistent heeft een antwoord.');
      // beurt_klaar komt na elk antwoord, klaar pas als het gesprek sluit.
      // Allebei betekenen ze: deze beurt is af, dus het spinnertje uit en de
      // wijzigingen klaarzetten om over te nemen.
      overlays.value = ev.overlays ?? null;
      afronding.value = { beurten: ev.beurten ?? 0, seconden: ev.seconden ?? 0 };
      voortgang.value = null;
      openVraag.value = null;
    } else {
      feed.value.push(ev);
    }
    scrollNaarBeneden();
}

/**
 * Houd het gesprek onderaan, waar het laatste bericht staat.
 *
 * Gebeurt bij elk bericht, en ook bij het openen van het paneel: kom je terug
 * van een andere pagina, dan is de feed al gevuld en staat hij anders bovenaan
 * te wachten terwijl het antwoord onderaan staat.
 */
function scrollNaarBeneden() {
  nextTick(() => {
    if (feedEl.value) feedEl.value.scrollTop = feedEl.value.scrollHeight;
  });
}
/**
 * De stream is dicht. Dat hoeft niet te betekenen dat het gesprek voorbij is:
 * sinds een run doorloopt als je naar een ander tabblad gaat, sluit de stream
 * ook bij een routewissel. Alleen als het gesprek echt weg is (`gesprekId` is
 * gewist door een `klaar`, of hervatten gaf 404) valt er iets af te ronden.
 */
function rondGesprekAf() {
  if (gesprekId.value) return;
  if (openVraag.value) {
    // Een dialoog laten staan die niets meer doet is erger dan hem weghalen.
    openVraag.value = null;
    feed.value.push({ type: 'tekst', tekst: 'Het gesprek is afgelopen terwijl er een keuze openstond.' });
  }
}

function takeOverlays() {
  if (!overlays.value) return;
  applyOverlays(overlays.value);
  feed.value.push({ type: 'tekst', tekst: `Overgenomen in ${werkversieLabel.value}.` });
  overlays.value = null;
}
</script>

<style scoped>
.assistent { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.as-feed {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 280px;
  overflow-y: auto;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.as-item { font-size: 0.9em; line-height: 1.45; }
.as-tekst { color: var(--semantics-content-color); }
.as-tool { color: var(--semantics-content-secondary-color); font-family: var(--primitives-font-family-monospace, monospace); font-size: 0.85em; }
.as-wijziging { color: var(--semantics-content-accent-color); }
.as-simulatie { color: var(--semantics-content-secondary-color); }
.as-fout { color: var(--semantics-content-critical-color); }
.as-vraag { color: var(--semantics-content-accent-color); font-weight: 600; }
/* De openstaande keuze hoort bij de laatste beurt van de assistent: links
   uitgelijnd, knoppen eronder, geen kader eromheen. Een gecentreerde dialoog
   boven het gesprek las als een systeemmelding in plaats van als een vraag. */
.as-keuze { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.as-keuze-toelichting {
  margin: 0; font-size: 0.85em;
  color: var(--semantics-content-secondary-color);
}
.as-opties {
  display: flex; flex-direction: column; gap: var(--primitives-space-8);
  align-items: flex-start;
}
/* Wat de gebruiker zegt, als bubbel rechts; alles van de assistent blijft
   links. Zo is met één blik te zien wie wat zei, zonder dat er een emoji voor
   hoeft te staan. */
.as-gebruiker {
  align-self: flex-end;
  max-width: 85%;
  padding: 6px var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-base-background-color);
  border: 1px solid var(--semantics-dividers-color);
  color: var(--semantics-content-color);
}
.as-voorbeelden > summary {
  cursor: pointer; list-style: none;
  font-size: 0.85em; font-weight: 600;
  color: var(--semantics-content-secondary-color);
  padding: 2px 0;
}
.as-voorbeelden > summary::-webkit-details-marker { display: none; }
.as-voorbeelden > summary::before { content: '▸ '; }
.as-voorbeelden[open] > summary::before { content: '▾ '; }
.as-knoppen { display: flex; gap: var(--primitives-space-8); align-items: center; }
.as-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.as-status {
  display: flex; align-items: center; gap: var(--primitives-space-8);
  font-size: 0.85em; color: var(--semantics-content-secondary-color);
  font-variant-numeric: tabular-nums;
  min-height: 24px;
}
/* Icoon en tekst horen bij elkaar; de groep krimpt mee maar rekt niet uit. */
.as-status-wat {
  display: inline-flex; align-items: center; gap: var(--primitives-space-8);
  min-width: 0;
}
/* nldd-activity-indicator is een block-element en rekte in de flexregel uit
   tot de halve breedte van het paneel, waardoor de tekst ernaast over twee
   regels brak. Vaste maat, en niet laten groeien of krimpen. */
.as-status-wat > nldd-activity-indicator,
.as-status-wat > nldd-icon {
  flex: none;
  width: 16px; height: 16px;
  display: inline-block;
}
/* De tekst krijgt wat overblijft en breekt niet midden in een woord af. */
.as-status-wat > span {
  min-width: 0;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
/* De teller staat direct achter de tekst, niet tegen de rechterrand: die
   afstand las als twee losse dingen in plaats van een regel. */
.as-status-teller { white-space: nowrap; flex: none; }
.as-punt {
  display: flex; flex-direction: column; gap: var(--primitives-space-8);
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.as-punt-kop { display: flex; align-items: center; gap: var(--primitives-space-8); font-size: 0.9em; }
.as-punt-kop nldd-icon-button { margin-left: auto; }
.as-punt-lijst { margin: 0; padding-left: 1.2em; font-size: 0.85em; display: flex; flex-direction: column; gap: 2px; }
.as-md code { font-size: 0.9em; }
</style>
