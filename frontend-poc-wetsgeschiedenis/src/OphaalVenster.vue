<template>
  <dialog ref="venster" class="ophaal" aria-labelledby="ophaal-titel-vue">
    <div class="ophaal-kop">
      <h2 id="ophaal-titel-vue">Een wet ophalen</h2>
      <span>
        <nldd-button appearance="neutral-transparent" size="md" text="Diagnose" @click="diagnose"></nldd-button>
        <nldd-button appearance="neutral-transparent" size="md" text="Sluiten" @click="sluit"></nldd-button>
      </span>
    </div>
    <div class="ophaal-binnen">
      <p>
        Geef het BWB-id van een wet, een algemene maatregel van bestuur of een ministeriële regeling; het staat in
        het adres op wetten.overheid.nl. De dienst speelt de regeling door de tijd af, met de nota's van toelichting,
        en haalt daarna op de achtergrond haar Kamerdossiers op. Wat is opgehaald, zien ook de anderen die deze
        pagina openen.
      </p>
      <form class="ophaal-zoek" @submit.prevent="zoek">
        <input v-model.trim="bwb" type="text" placeholder="BWBR0007376" aria-label="BWB-id" spellcheck="false" autocomplete="off" />
        <nldd-button appearance="primary" size="md" text="Zoek" @click="zoek"></nldd-button>
      </form>

      <div aria-live="polite">
        <p v-if="fout" class="ophaal-fout">{{ fout }}</p>
        <p v-if="bezig">{{ bezig }} <span>{{ teller }} s</span></p>

        <!-- De regeling: één knop voor de wet, en per versie haar dossiers. -->
        <template v-if="reg && !taak && !diag">
          <h3>{{ reg.titel }}<template v-if="reg.afkorting"> ({{ reg.afkorting }})</template></h3>
          <p>
            <button type="button" class="ophaal-actie" @click="haal({ keten: true })">
              {{ reg.keten ? 'Haal de wet opnieuw op' : 'Haal de wet op' }}
            </button>
            <button v-if="reg.keten" type="button" class="ophaal-actie" @click="open(reg.bwb)">Open</button>
          </p>
          <p class="klein">
            Elke versie op wetten.overheid.nl, elke overgang afgespeeld uit haar bekendmaking, met de nota van
            toelichting die daarin staat.
            <template v-if="aantalDossiers">
              Daarna haalt de dienst op de achtergrond de {{ aantalDossiers }} Kamerdossiers hieronder op, de jongste
              eerst; elk dossier dat klaar is, hangt zijn toelichting aan zijn overgang.
            </template>
            <template v-else>
              Geen versie kwam uit een Kamerdossier: een algemene maatregel van bestuur of een ministeriële regeling
              gaat niet langs de Kamer, en haar toelichting staat in haar eigen bekendmaking.
            </template>
          </p>
          <table v-if="reg.versies.length" class="ophaal-lijst">
            <thead>
              <tr><th>In werking</th><th>Wat</th><th>Bekendmaking</th><th>Kamerdossier</th></tr>
            </thead>
            <tbody>
              <tr v-for="(v, i) in reg.versies" :key="i">
                <td>{{ v.inwerking ? datumTekst(v.inwerking) : 'nog niet' }}</td>
                <td>{{ v.betreft }}</td>
                <td>{{ bekendmaking(v.staatsblad) }}</td>
                <td>
                  <button v-for="d in v.dossiers" :key="d.nr" type="button" class="ophaal-actie"
                    @click="d.beschikbaar ? open(d.nr) : haal({ dossier: d.nr })">
                    {{ d.beschikbaar ? 'Open ' + d.nr : 'Haal nu op: ' + d.nr }}{{ stand(d) }}
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </template>

        <!-- Een opdracht, ook als zij nog in de rij staat. -->
        <template v-if="taak">
          <h3>{{ KOP[taak.status] }}{{ naam(taak) }}</h3>
          <p v-if="taak.status === 'wacht'">
            Plaats {{ taak.plaats }} in de rij<template v-if="rij.huidig">; nu bezig: {{ naam(rij.huidig) }}</template>.
          </p>
          <p v-if="taak.fout" class="ophaal-fout">{{ taak.fout }}</p>
          <p v-if="taak.status === 'klaar'">
            <button type="button" class="ophaal-actie" @click="open(taak.uitkomst)">Open {{ taak.uitkomst }}</button>
          </p>
          <p v-if="taak.status === 'bezig'">De stukken ophalen en afspelen duurt een paar minuten.</p>
          <p v-if="achter.length" class="klein">
            Op de achtergrond nog {{ achter.length }} Kamerdossier{{ achter.length === 1 ? '' : 's' }} voor {{ wet }}:
            {{ achter.map((x) => x.dossier + (x.status === 'bezig' ? ' (nu)' : '')).join(', ') }}. Je kunt de wet al
            openen; wat later klaar is, staat erin als je de pagina opnieuw laadt.
          </p>
          <pre v-if="taak.log.length" class="ophaal-log">{{ taak.log.join('\n') }}</pre>
        </template>

        <!-- Wat de dienst bereikt, en wat hij de laatste tijd deed. -->
        <template v-if="diag">
          <h3 :class="{ 'ophaal-fout': diag.oordeel !== 'ophalen kan' }">{{ diag.oordeel }}</h3>
          <table class="ophaal-lijst">
            <thead><tr><th>Host</th><th>Bereikbaar</th><th>Waarvoor</th></tr></thead>
            <tbody>
              <tr v-for="(h, naamHost) in diag.hosts" :key="naamHost">
                <td>{{ naamHost }}</td>
                <td>
                  <template v-if="h.ok">ja, HTTP {{ h.http }} in {{ h.totaal_s }} s</template>
                  <span v-else class="ophaal-fout">nee: {{ h.uitleg || h.fout || 'onbekend' }}</span>
                </td>
                <td>{{ h.waarvoor }}</td>
              </tr>
            </tbody>
          </table>
          <p>
            Schrijven in {{ diag.data.pad }}:
            <template v-if="diag.data.schrijfbaar">ja, {{ diag.data.vrij_mb }} MB vrij</template>
            <span v-else class="ophaal-fout">nee</span>.
            {{ diag.curl }}. Werker: {{ diag.werker.ok ? 'python ' + diag.werker.python : 'ontbreekt' }}.
            Proxy: {{ proxy }}. Dossiers: {{ diag.dossiers.in_het_image }} in het image, {{ diag.dossiers.opgehaald }}
            opgehaald (ten hoogste {{ diag.dossiers.max }}).
          </p>
          <p v-if="diag.opzoeking">
            Opzoeking van {{ diag.opzoeking.bwb }}:
            <template v-if="diag.opzoeking.ok">{{ diag.opzoeking.titel }}, {{ diag.opzoeking.versies_met_dossier }} versies met een Kamerdossier</template>
            <span v-else class="ophaal-fout">{{ diag.opzoeking.fout }}</span>
            ({{ diag.opzoeking.s }} s).
          </p>
          <h3>Logboek</h3>
          <pre class="ophaal-log">{{ logboek || 'leeg' }}</pre>
          <p>
            Als JSON, om door te sturen:
            <a :href="link('api/diagnose' + (isBwb(bwbGroot) ? '?bwb=' + bwbGroot : ''))" target="_blank" rel="noopener">api/diagnose</a>,
            <a :href="link('api/log')" target="_blank" rel="noopener">api/log</a>.
          </p>
        </template>
      </div>
    </div>
  </dialog>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { api, bekendmaking, datumTekst, isBwb, link, open, post } from './api.js';

const KOP = { wacht: 'In de rij: ', bezig: 'Bezig: ', klaar: 'Klaar: ', fout: 'Mislukt: ' };

const venster = ref(null);
const bwb = ref('');
const fout = ref('');
const bezig = ref('');
const teller = ref(0);
const reg = ref(null);
const taak = ref(null);
const rij = ref({ huidig: null, wachtend: [] });
const diag = ref(null);
const logboek = ref('');
let gevolgd = null;
let klok = null;
let volgTimer = null;

const bwbGroot = computed(() => bwb.value.toUpperCase());
const aantalDossiers = computed(() => new Set((reg.value?.versies ?? []).flatMap((v) => v.dossiers.map((d) => d.nr))).size);
const wet = computed(() => (taak.value?.wat === 'keten' ? taak.value.bwb : taak.value?.voor));
const achter = computed(() => [rij.value.huidig, ...rij.value.wachtend]
  .filter((x) => x && x.voor && x.voor === wet.value && x.id !== taak.value?.id));
const proxy = computed(() => Object.entries(diag.value?.proxy ?? {}).map(([k, v]) => `${k}=${v}`).join(', ') || 'geen');

const naam = (x) =>
  x.wat === 'keten' ? `${x.bwb} door de tijd` : x.wat === 'geschiedenis' ? `${x.bwb}, alle versies` : `dossier ${x.dossier}`;
const stand = (d) => (d.beschikbaar ? '' : d.rij === 'bezig' ? ' (wordt nu opgehaald)'
  : d.rij ? ' (in de rij)' : d.mislukt ? ' (eerder mislukt)' : '');
const regel = (r) => [r.t?.slice(11), r.soort, r.methode, r.wat && `${r.wat} ${r.bwb}${r.dossier ? ' ' + r.dossier : ''}`,
  r.host ? r.host + r.pad : r.pad || r.bij, r.status ?? r.http ?? (r.code ? 'curl ' + r.code : ''),
  r.uitleg || r.fout || r.oordeel, r.ms != null ? r.ms + ' ms' : r.s != null ? r.s + ' s' : '']
  .filter((v) => v != null && v !== '').join('  ');

/** Een teller zolang er gewacht wordt; de opzoeking kan tientallen seconden duren. */
function wacht(tekst) {
  fout.value = '';
  bezig.value = tekst;
  teller.value = 0;
  const t0 = performance.now();
  clearInterval(klok);
  klok = setInterval(() => { teller.value = Math.round((performance.now() - t0) / 1000); }, 1000);
}
function klaar() {
  clearInterval(klok);
  bezig.value = '';
}
function leeg() {
  reg.value = null;
  taak.value = null;
  diag.value = null;
  clearTimeout(volgTimer);
}

async function zoek() {
  leeg();
  gevolgd = null;
  if (!isBwb(bwbGroot.value)) {
    fout.value = 'Een BWB-id ziet eruit als BWBR0007376.';
    return;
  }
  wacht(`Op wetten.overheid.nl opzoeken wat ${bwbGroot.value} maakte en wijzigde …`);
  const { ok, x } = await api('api/regeling/' + bwbGroot.value);
  klaar();
  if (!ok) {
    fout.value = x.fout + ' Diagnose laat zien wat de dienst wel en niet bereikt.';
    return;
  }
  reg.value = x;
}

async function haal(wat) {
  const { ok, x } = await post('api/haal', { bwb: reg.value.bwb, ...wat });
  if (!ok) {
    fout.value = x.fout;
    return;
  }
  volg(x.id);
}

/** Een opdracht volgen op haar id, met wat de rij daarna nog voor dezelfde wet ophaalt. */
async function volg(id) {
  if (id) gevolgd = id;
  clearTimeout(volgTimer);
  const [{ ok, x }, { x: st }] = await Promise.all([
    api('api/taak' + (gevolgd ? '?id=' + encodeURIComponent(gevolgd) : '')),
    api('api/status'),
  ]);
  if (!ok) {
    fout.value = x.fout;
    return;
  }
  if (!x) return;
  diag.value = null;
  taak.value = x;
  rij.value = st?.rij ?? { huidig: null, wachtend: [] };
  if (['wacht', 'bezig'].includes(x.status) || achter.value.length) volgTimer = setTimeout(volg, 1500);
}

async function diagnose() {
  leeg();
  const met = isBwb(bwbGroot.value);
  wacht(`De dienst toetst wat hij bereikt${met ? ' en zoekt ' + bwbGroot.value + ' op' : ''} …`);
  const [{ ok, x }, { x: log }] = await Promise.all([
    api('api/diagnose' + (met ? '?bwb=' + bwbGroot.value : '')),
    api('api/log'),
  ]);
  klaar();
  if (!ok) {
    fout.value = x.fout;
    return;
  }
  diag.value = x;
  logboek.value = (Array.isArray(log) ? log : []).slice(-40).map(regel).join('\n');
}

function sluit() {
  venster.value?.close();
}

// De knop 'Wet ophalen' staat in de werkbalk van de bekijker; hij verschijnt
// alleen als de dienst er is.
let knop = null;
async function toon() {
  venster.value?.showModal();
  if (!gevolgd) {
    const { x } = await api('api/status');
    if (x?.taak?.status === 'bezig') volg(x.taak.id);
  }
}
onMounted(async () => {
  const { ok, x } = await api('api/status');
  if (!ok || !x?.ophalen) return;
  document.getElementById('ophalen-plek')?.classList.remove('verborgen');
  knop = document.getElementById('ophalen');
  knop?.addEventListener('click', toon);
});
onBeforeUnmount(() => {
  knop?.removeEventListener('click', toon);
  clearInterval(klok);
  clearTimeout(volgTimer);
});
</script>
