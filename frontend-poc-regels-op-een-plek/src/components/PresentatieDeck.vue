<script setup>
// Presentatie over de PoC: waarom, hoe de toets werkt, wat de tests lieten zien, waar het
// misgaat, RFC-057 en de activiteiten. Schermvullend boven de kaart; pijltjes, spatie en Escape
// werken. De dia staat in de adresregel (?presentatie=3), zodat je kunt linken. De cijfers komen
// uit de analyses van de casus.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';

const open = defineModel('open', { type: Boolean, default: false });

// Een dia met `stappen` bouwt zich op: pijl naar rechts gaat eerst door de stappen.
const DIAS = [
  { titel: 'Titel' },
  { titel: 'Waarom' },
  { titel: 'Van plek naar oordeel' },
  { titel: 'Wat de tests lieten zien' },
  { titel: 'Waar het misgaat' },
  { titel: 'Hoe de fout ontstaat', stappen: 4 },
  { titel: 'Wat het gemiste artikel doet' },
  { titel: 'RFC-057: de plek in de engine' },
  { titel: 'De activiteit' },
  { titel: 'Elke overheid eigen activiteiten' },
];

// De bundel van activiteiten bij het bouwplan, met de groep waaronder het DSO de regels hangt.
const BUNDEL = [
  { naam: 'Bouwen', wie: 'bouwactiviteit ruimtelijk' },
  { naam: 'Technisch bouwen', wie: 'bouwactiviteit technisch' },
  { naam: 'Wonen', wie: 'woonactiviteit, gebruik' },
  { naam: 'Woningen toevoegen', wie: 'geluid, bodem, parkeren' },
  { naam: 'Funderen', wie: 'werken, bodem' },
  { naam: 'Bouwput bemalen', wie: 'grondwater, lozen' },
  { naam: 'Aansluiten op riool', wie: 'aansluitactiviteit' },
  { naam: 'Uitrit maken', wie: 'uitritactiviteit' },
];

const nu = ref(0);
const stap = ref(0);
const laatste = DIAS.length - 1;
const aantalStappen = (i) => DIAS[i].stappen ?? 1;
const deck = ref(null);
let vorigeFocus = null;

function naar(i, s = 0) {
  nu.value = Math.min(laatste, Math.max(0, i));
  stap.value = Math.min(Math.max(0, s), aantalStappen(nu.value) - 1);
}
function volgende() {
  if (stap.value < aantalStappen(nu.value) - 1) stap.value += 1;
  else if (nu.value < laatste) naar(nu.value + 1);
}
function vorige() {
  if (stap.value > 0) stap.value -= 1;
  else if (nu.value > 0) naar(nu.value - 1, aantalStappen(nu.value - 1) - 1);
}
const helemaalAan = computed(() => nu.value === laatste && stap.value === aantalStappen(laatste) - 1);

// Dia "Hoe de fout ontstaat": één punt in Woongebieden 2018, bestemming Gemengd, in de
// veiligheidszone van de hoogspanning (RD 166380, 448060; scenario 1 van de woontoren). De
// lijsten komen uit de gesloten lijst van de casus, 5 lagen, alles gunstig, leiding nog in gebruik:
// 40 artikelen op het punt, 15 met rol toets, 9 gerekend, 6 gemist.
//   via    hoe het artikel in YAML kwam (keten, kaart of tabel); leeg = niet in YAML
//   rol    uit artikelen.json: toets, route, hulp, anders, vervangen
const CEL = [
  {
    naam: 'Omgevingsplan (bruidschat)', grond: 'hele gemeente',
    artikelen: [
      { nr: '22.1', titel: 'Voorrang bruidschat', rol: 'hulp' },
      { nr: '22.26', titel: 'Vergunningplicht bouwen', rol: 'toets', via: 'keten' },
      { nr: '22.27', titel: 'Uitzonderingen', rol: 'toets', via: 'keten' },
      { nr: '22.29', titel: 'Beoordelingsregels', rol: 'toets', via: 'keten' },
      { nr: '22.32', titel: 'Wijzigingsbevoegdheid', rol: 'route' },
      { nr: '22.33', titel: 'Voorbereidingsbesluit', rol: 'route' },
      { nr: '22.35', titel: 'Aanvraagvereisten', rol: 'hulp' },
      { nr: '22.280', titel: 'Afwijkingsvergunning', rol: 'route' },
      { nr: '22.281', titel: 'Afwijkingsvergunning', rol: 'route' },
    ],
  },
  {
    naam: 'Woongebieden 2018', grond: 'basisplan op het punt', breed: true,
    artikelen: [
      { nr: '1', titel: 'Begrippen', rol: 'hulp' },
      { nr: '2', titel: 'Wijze van meten', rol: 'hulp' },
      { nr: '5.1', titel: 'Gemengd: omschrijving', rol: 'toets', via: 'kaart' },
      { nr: '5.2', titel: 'Gemengd: bouwregels', rol: 'toets', via: 'kaart' },
      { nr: '20', titel: 'Anti-dubbeltelregel', rol: 'toets', mist: 'plan' },
      { nr: '21.1', titel: 'Ondergronds bouwen', rol: 'vervangen' },
      { nr: '22.1', titel: 'Strijdig gebruik', rol: 'toets', mist: 'plan' },
      { nr: '23.1', titel: 'Grondwaterbescherming', rol: 'anders' },
      { nr: '23.5', titel: 'Veiligheidszone hoogspanning', rol: 'toets', mist: 'wonen', sleutel: true },
      { nr: '24.1', titel: 'Afwijken: maatvoering', rol: 'route' },
      { nr: '24.2', titel: 'Afwijken: afbreuk', rol: 'route' },
      { nr: '25.1', titel: 'Wijzigen', rol: 'route' },
      { nr: '26.1', titel: 'Parkeren', rol: 'vervangen' },
      { nr: '27', titel: 'Overgangsrecht', rol: 'anders' },
      { nr: '28', titel: 'Slotregel', rol: 'anders' },
    ],
  },
  {
    naam: 'Parapluplan 2022', grond: 'via zijn tabellen',
    artikelen: [
      { nr: '2', titel: 'Wijze van meten', rol: 'hulp' },
      { nr: '3', titel: 'Definitie hoofdgebouw', rol: 'toets', via: 'tabel' },
      { nr: '4', titel: 'Bouwen en gebruik', rol: 'toets', via: 'tabel' },
      { nr: '10', titel: 'Algemene bouwregels', rol: 'toets', via: 'tabel' },
      { nr: '11', titel: 'Parkeren e.a.', rol: 'toets', via: 'tabel' },
      { nr: '12', titel: 'Overgangsrecht', rol: 'anders' },
      { nr: '13', titel: 'Slotregel', rol: 'anders' },
    ],
  },
  {
    naam: 'Ondergrondse hoogspanning', grond: 'leiding op het punt',
    artikelen: [
      { nr: '1', titel: 'Begrippen', rol: 'hulp' },
      { nr: '4.1', titel: 'Leiding: omschrijving', rol: 'toets', mist: 'vlak' },
      { nr: '4.2', titel: 'Wijzigingsbevoegdheid', rol: 'route' },
      { nr: '5', titel: 'Anti-dubbeltelregel', rol: 'toets', mist: 'plan' },
      { nr: '6.1', titel: 'Sloop masten', rol: 'anders' },
      { nr: '7.1', titel: 'Van toepassing', rol: 'toets', mist: 'plan' },
      { nr: '7.2', titel: 'Verwijzing', rol: 'hulp' },
      { nr: '8', titel: 'Overgangsrecht', rol: 'anders' },
      { nr: '9', titel: 'Slotregel', rol: 'anders' },
    ],
  },
];
const CEL_STAPPEN = ['Op het punt', 'Zo zochten we', 'De checklist', 'Het verschil'];

function tegelKlasse(a) {
  const s = stap.value;
  if (s === 0) return 'neutraal';
  if (s === 1) return a.via ? 'gevonden' : 'stil';
  if (s === 2) return a.rol === 'toets' ? 'telt' : 'nvt';
  if (a.rol !== 'toets') return 'nvt';
  return a.via ? 'gedekt' : a.sleutel ? 'gemist sleutel' : 'gemist';
}
function tegelLabel(a) {
  const s = stap.value;
  if (s === 1) return a.via ?? '';
  if (s >= 2 && a.rol !== 'toets') return a.rol;
  if (s === 2) return 'telt mee';
  if (s === 3) return a.via ? 'gerekend' : 'gemist';
  return '';
}

// Openen vanuit de adresregel: ?presentatie, ?presentatie=4, of ?presentatie=6.3 (dia 6, stap 3).
const uitUrl = new URLSearchParams(window.location.search).get('presentatie');
if (uitUrl !== null) {
  open.value = true;
  const [d, s] = uitUrl.split('.').map((x) => parseInt(x, 10));
  naar((d || 1) - 1, (s || 1) - 1);
}

function zetUrl() {
  const url = new URL(window.location.href);
  if (open.value) url.searchParams.set('presentatie', aantalStappen(nu.value) > 1 ? `${nu.value + 1}.${stap.value + 1}` : String(nu.value + 1));
  else url.searchParams.delete('presentatie');
  window.history.replaceState(null, '', url);
}

// Toetsen vangen in de capture-fase, zodat de kaart eronder niet meeschuift.
function opToets(e) {
  if (!open.value) return;
  const k = e.key;
  let gedaan = true;
  if (k === 'ArrowRight' || k === 'PageDown' || k === ' ') volgende();
  else if (k === 'ArrowLeft' || k === 'PageUp') vorige();
  else if (k === 'Home') naar(0);
  else if (k === 'End') naar(laatste);
  else if (k === 'Escape') open.value = false;
  else gedaan = false;
  if (gedaan) { e.preventDefault(); e.stopPropagation(); }
}

watch(open, async (isOpen) => {
  zetUrl();
  if (isOpen) {
    vorigeFocus = document.activeElement;
    window.addEventListener('keydown', opToets, true);
    await nextTick();
    deck.value?.focus();
  } else {
    window.removeEventListener('keydown', opToets, true);
    vorigeFocus?.focus?.();
  }
}, { immediate: true });
watch([nu, stap], zetUrl);
onBeforeUnmount(() => window.removeEventListener('keydown', opToets, true));

const teller = computed(() => `${nu.value + 1} / ${DIAS.length}`);
const stapTekst = computed(() => (aantalStappen(nu.value) > 1 ? `stap ${stap.value + 1} van ${aantalStappen(nu.value)}` : ''));
</script>

<template>
  <div
    v-if="open"
    ref="deck"
    class="deck"
    role="dialog"
    aria-modal="true"
    aria-label="Presentatie over de PoC"
    tabindex="-1"
  >
    <header class="balk">
      <span class="balk-titel">Presentatie · PoC Spoorzone</span>
      <nav class="stippen" aria-label="Dia's">
        <button
          v-for="(d, i) in DIAS"
          :key="d.titel"
          type="button"
          class="stip"
          :class="{ actief: i === nu }"
          :aria-label="`Dia ${i + 1}: ${d.titel}`"
          :aria-current="i === nu ? 'step' : undefined"
          :title="d.titel"
          @click="naar(i)"
        ></button>
      </nav>
      <div class="balk-rechts">
        <nldd-icon-button size="sm" icon="chevron-left" text="Vorige" appearance="neutral-tinted" :disabled="(nu === 0 && stap === 0) || undefined" @click="vorige"></nldd-icon-button>
        <span class="teller" aria-live="polite">{{ teller }}<template v-if="stapTekst"> · {{ stapTekst }}</template></span>
        <nldd-icon-button size="sm" icon="chevron-right" text="Volgende" appearance="neutral-tinted" :disabled="helemaalAan || undefined" @click="volgende"></nldd-icon-button>
        <nldd-icon-button size="sm" icon="close" text="Sluiten" appearance="neutral-tinted" @click="open = false"></nldd-icon-button>
      </div>
    </header>

    <div class="podium">
      <!-- 1. Titel -->
      <section v-if="nu === 0" class="dia dia-titel">
        <p class="kicker">PoC Spoorzone Veenendaal · RegelRecht in de fysieke leefomgeving</p>
        <h2 class="groot">Kan hier een woontoren<br />van 5 lagen?</h2>
        <p class="lead">
          Wat we bouwden, hoe we het toetsten, waar het misgaat
          en hoe RegelRecht verder kan met regels die van de plek afhangen.
        </p>
        <p class="voet">Verkenning, geen besluit · stand 17 november 2026</p>
      </section>

      <!-- 2. Waarom -->
      <section v-else-if="nu === 1" class="dia">
        <p class="kicker">Waarom</p>
        <h2>Bij bouwen is de eenheid geen wet, maar een plek</h2>
        <div class="twee">
          <div class="kolom">
            <p>
              RegelRecht maakt wetten uitvoerbaar. Een wet is één tekst, met verwijzingen naar
              andere wetten.
            </p>
            <p>
              Bij bouwen hangt het antwoord af van waar je staat. Welke regels gelden, verschilt
              per meter, en een deel van de norm staat op de kaart, niet in de tekst.
            </p>
            <p class="nadruk">
              De vraag van deze PoC: werkt de werkwijze van RegelRecht ook hier?
            </p>
          </div>
          <div class="kolom">
            <dl class="cijfers">
              <div><dt>circa 160 ha</dt><dd>rond station Veenendaal-Centrum</dd></div>
              <div><dt>circa 4.500</dt><dd>woningen tot ongeveer 2040</dd></div>
              <div><dt>35 plannen</dt><dd>bestemmingsplannen met de bruidschat erbovenop: het tijdelijk deel</dd></div>
              <div><dt>3 TAM-plannen</dt><dd>per locatie, het nieuwe deel; één nog in ontwerp</dd></div>
            </dl>
          </div>
        </div>
        <p class="voet">Het doel is niet de Spoorzone zelf, maar leren wat RegelRecht nodig heeft.</p>
      </section>

      <!-- 3. Van plek naar oordeel -->
      <section v-else-if="nu === 2" class="dia">
        <p class="kicker">Hoe de toets werkt</p>
        <h2>Van plek naar oordeel</h2>
        <ol class="keten">
          <li><strong>Plek</strong><span>klik op de kaart</span></li>
          <li><strong>Stapel plannen</strong><span>basisplan, lagen erboven, wat opzij gaat</span></li>
          <li><strong>Regels in YAML</strong><span>elke regel met een letterlijk citaat</span></li>
          <li><strong>RegelRecht-engine</strong><span>de echte engine, in de browser</span></li>
          <li><strong>Artikel 22.29</strong><span>“niet in strijd met de regels over het bouwen”</span></li>
          <li><strong>Oordeel</strong><span>per kolom, met de artikelen waar het uit komt</span></li>
        </ol>
        <div class="twee onder">
          <div class="kolom">
            <h3>Twee kolommen naast elkaar</h3>
            <p>
              Het tijdelijk deel (bestemmingsplannen) en het nieuwe deel (TAM-plannen) rekenen
              apart. Er is geen voorrangsregel tussen die twee: we tonen beide.
            </p>
          </div>
          <div class="kolom">
            <h3>Drie uitkomsten</h3>
            <p class="oordelen">
              <span class="chip niet">niet toegestaan</span>
              <span class="pijl">gaat voor</span>
              <span class="chip onth">onthouding</span>
              <span class="pijl">gaat voor</span>
              <span class="chip ja">toegestaan</span>
            </p>
            <p class="klein">Ontbreekt een gegeven, dan onthoudt de toets zich en zegt hij wat ontbreekt.</p>
          </div>
        </div>
      </section>

      <!-- 4. Wat de tests lieten zien -->
      <section v-else-if="nu === 3" class="dia">
        <p class="kicker">Uitslag</p>
        <h2>Een nee klopt bijna altijd. Een ja nooit helemaal.</h2>
        <div class="tegels">
          <div class="tegel">
            <span class="getal">4 van 5</span>
            <span>scenario's met “niet toegestaan” uit de toelichting kloppen</span>
          </div>
          <div class="tegel rood">
            <span class="getal">0 van 85</span>
            <span>punten met “toegestaan” hebben alle artikelen gerekend die daar gelden</span>
          </div>
          <div class="tegel">
            <span class="getal">13 van 17</span>
            <span>blinde scenario's goed, in beide sporen</span>
          </div>
          <div class="tegel">
            <span class="getal">0</span>
            <span>keer “toegestaan” waar de gemeente buitenplans moest afwijken</span>
          </div>
        </div>
        <div class="twee onder">
          <div class="kolom">
            <h3>Waarom een nee sterker is</h3>
            <p>
              De engine rekent met drie waarden en 22.29 is een EN over alle voorwaarden. Een nee
              uit één artikel blijft staan, wat er verder ook ontbreekt.
            </p>
          </div>
          <div class="kolom">
            <h3>Waarom een ja zwakker is</h3>
            <p>
              Een ja zegt: alle gerekende voorwaarden zijn waar. Over de artikelen die niet gerekend
              zijn, zegt het niets. Een ja is een lijst van wat is nagekeken, geen vergunning.
            </p>
          </div>
        </div>
      </section>

      <!-- 5. Waar het misgaat -->
      <section v-else-if="nu === 4" class="dia">
        <p class="kicker">Waar het misgaat</p>
        <h2>Niet bij het rekenen, maar bij het kiezen van de artikelen</h2>
        <p class="lead klein-lead">
          Artikel 22.29 zegt “niet in strijd met de regels over het bouwen”, maar niet welke. De
          engine deed wat de YAML zei. De YAML was niet volledig, en niets in de keten kon dat zien.
        </p>
        <div class="kaarten">
          <div class="kaartje">
            <strong>Verwijzingen lopen achterstevoren</strong>
            <span>“In aanvulling op artikel 22.29 …” Het plan wijst naar de spil, de spil niet naar het plan. Zo misten we TAM Het Ambacht 53.4 en 68.</span>
            <q>De omgevingsvergunning als bedoeld in artikel 22.26 van het plan wordt alleen verleend als: a. wordt voldaan aan de ontwikkelprincipes van een bouwveld …</q>
            <span class="klein">TAM Het Ambacht (ontwerp), 53.4. Geldt op alle 70 punten waar de toets ja zei. Het hangt aan geen enkel vlak op de kaart.</span>
          </div>
          <div class="kaartje">
            <strong>Algemene regels liggen over alles heen</strong>
            <span>Een verbod in een zone geldt voor elke bestemming. Wij modelleerden het via één bestemming.</span>
          </div>
          <div class="kaartje">
            <strong>De maat telde plekken, geen artikelen</strong>
            <span>“Elk punt heeft een antwoord” zegt niets over een artikel dat ontbreekt en het antwoord omdraait.</span>
          </div>
          <div class="kaartje">
            <strong>Kaart en tekst lopen uiteen</strong>
            <span>Verschoven artikelnummers in vijf plannen. Eén plan bestaat alleen als scan: 239 punten zonder antwoord.</span>
          </div>
          <div class="kaartje">
            <strong>De activiteit bestaat niet als gegeven</strong>
            <span>Ook het DSO kent geen “woongebouw bouwen”. Van 866 activiteitnamen in het gebied kozen we er 35, met de hand.</span>
          </div>
        </div>
      </section>

      <!-- 6. Hoe de fout ontstaat: één punt, vier stappen (gegevens in CEL, zie het script) -->
      <section v-else-if="nu === 5" class="dia dia-cel">
        <div class="cel-kop">
          <div>
            <p class="kicker">Hoe de fout ontstaat</p>
            <h2>Eén punt, twee manieren van zoeken</h2>
          </div>
          <div class="stapknoppen" role="group" aria-label="Stappen van deze dia">
            <button
              v-for="(t, i) in CEL_STAPPEN"
              :key="t"
              type="button"
              :class="{ actief: i === stap, gedaan: i < stap }"
              :aria-pressed="i === stap"
              @click="stap = i"
            ><span>{{ i + 1 }}</span>{{ t }}</button>
          </div>
        </div>

        <div class="cel-body">
          <div class="cel-links">
            <svg class="kaartje-svg" viewBox="0 0 300 220" role="img" aria-label="Schets van het punt: bestemming Gemengd met bouwvlak, doorsneden door de veiligheidszone van de hoogspanningsleiding">
              <defs>
                <pattern id="arcering" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                  <line x1="0" y1="0" x2="0" y2="8" class="arceer" />
                </pattern>
              </defs>
              <rect x="4" y="4" width="292" height="212" rx="6" class="svg-grond" />
              <rect x="20" y="28" width="190" height="164" rx="3" class="svg-gemengd" :class="{ licht: stap === 1 }" />
              <rect x="36" y="58" width="150" height="104" class="svg-bouwvlak" />
              <polygon points="30,4 90,4 220,216 160,216" class="svg-zone" :class="{ dof: stap === 1, rood: stap === 3 }" />
              <polygon points="30,4 90,4 220,216 160,216" fill="url(#arcering)" :class="{ dof: stap === 1 }" />
              <line x1="60" y1="4" x2="190" y2="216" class="svg-leiding" :class="{ dof: stap === 1 }" />
              <circle cx="121" cy="109" r="6" class="svg-punt" />
              <text x="122" y="46" class="svg-tekst sterk">Gemengd · art. 5</text>
              <text x="40" y="154" class="svg-tekst">bouwvlak 18 m</text>
              <text x="222" y="104" class="svg-tekst" :class="{ dof: stap === 1, rood: stap === 3 }">veiligheids-</text>
              <text x="222" y="116" class="svg-tekst" :class="{ dof: stap === 1, rood: stap === 3 }">zone · 23.5</text>
              <text x="222" y="140" class="svg-tekst" :class="{ dof: stap === 1 }">leiding ·</text>
              <text x="222" y="152" class="svg-tekst" :class="{ dof: stap === 1 }">OH art. 4</text>
              <text x="10" y="210" class="svg-tekst klein">grondwater · 23.1</text>
            </svg>
            <p class="kaart-onder">Schets, niet op schaal. De stip is het punt (RD 166380, 448060). Bouwplan: 5 lagen, 16 m.</p>

            <div class="uitleg" aria-live="polite">
              <template v-if="stap === 0">
                <p><strong>Op dit punt</strong> gelden vier regelingen met samen 40 artikelen. De kaart toont drie vlakken: de bestemming Gemengd, de veiligheidszone en de leiding.</p>
              </template>
              <template v-else-if="stap === 1">
                <p><strong>Een artikel kwam in YAML als iets ernaar wees:</strong> de keten vanaf de vergunningplicht, het vlak van de bestemming, of de tabel van het Parapluplan.</p>
                <p class="resultaat">9 artikelen gerekend. Toets: <span class="chip ja">toegestaan</span></p>
              </template>
              <template v-else-if="stap === 2">
                <p><strong>De checklist loopt elk artikel af.</strong> Het telt mee, tenzij de titel zegt dat het over iets anders gaat: begrippen, afwijken, overgangsrecht.</p>
                <p class="resultaat">15 van de 40 artikelen tellen mee voor bouwen.</p>
              </template>
              <template v-else>
                <p><strong>9 gerekend, 6 gemist.</strong></p>
                <ul class="waarom-lijst">
                  <li><b>20, 22.1, OH 5, OH 7.1</b> gelden in het hele plan. Geen vlak wees ernaar.</li>
                  <li><b>OH 4.1</b>: het vlak van de leiding ligt er, het artikel stond niet in YAML.</li>
                  <li class="sleutel-regel"><b>23.5</b>: de zone ligt er, maar het verbod zat alleen in de regels voor Wonen (13.2.2). Het verbiedt hier nieuwe woningen.</li>
                </ul>
                <p class="resultaat">Toets: <span class="chip ja">toegestaan</span> Juist: <span class="chip niet">niet toegestaan</span></p>
              </template>
            </div>
          </div>

          <div class="cel-rechts">
            <div v-for="r in CEL" :key="r.naam" class="regeling" :class="{ breed: r.breed }">
              <p class="regeling-kop">{{ r.naam }}<span>{{ r.grond }}</span></p>
              <ul class="tegeltjes">
                <li v-for="a in r.artikelen" :key="a.nr" class="tegeltje" :class="tegelKlasse(a)" :title="`${a.nr} ${a.titel} (${a.rol})`">
                  <span class="boven"><b>{{ a.nr }}</b><span v-if="tegelLabel(a)" class="tag">{{ tegelLabel(a) }}</span></span>
                  <span class="t">{{ a.titel }}</span>
                </li>
              </ul>
            </div>
          </div>
        </div>
      </section>

      <!-- 7. Wat het gemiste artikel doet -->
      <section v-else-if="nu === 6" class="dia">
        <p class="kicker">Wat het gemiste artikel doet</p>
        <h2>Gemengd in de veiligheidszone van de hoogspanning</h2>
        <div class="twee">
          <div class="kolom">
            <p>
              Een punt in Woongebieden 2018, bestemming Gemengd, maximaal 18 meter. Het bouwplan is
              16 meter. De toets zei: toegestaan.
            </p>
            <blockquote>
              “Ter plaatse van de aanduiding ‘Veiligheidszone - leiding hoogspanningsverbinding’ is
              het, onverminderd het elders in dit plan bepaalde, verboden nieuwe kwetsbare functies
              (bijvoorbeeld woningen …) op te richten.”
              <cite>Woongebieden 2018, artikel 23.5.1</cite>
            </blockquote>
            <p class="klein">
              Hetzelfde verbod staat voor Wonen nog eens in 13.2.2. Alleen die versie hadden we
              gemodelleerd.
            </p>
            <p class="klein">
              Het klassieke RegelRecht-proces had dit wel gevonden: dat leest het hele plan en geeft
              elk artikel een antwoord. Wij lazen alleen wat de vraag op de plek raakte.
            </p>
          </div>
          <div class="kolom">
            <table class="proef">
              <caption>Zelfde zone, drie keer gerekend. Is de oude leiding buiten gebruik?</caption>
              <thead>
                <tr><th scope="col">Bestemming</th><th scope="col">ja</th><th scope="col">nee</th><th scope="col">onbekend</th></tr>
              </thead>
              <tbody>
                <tr>
                  <th scope="row">Wonen</th>
                  <td><span class="chip ja">toegestaan</span></td>
                  <td><span class="chip niet">niet</span></td>
                  <td><span class="chip onth">onthouding</span></td>
                </tr>
                <tr>
                  <th scope="row">Gemengd</th>
                  <td><span class="chip ja">toegestaan</span></td>
                  <td><span class="chip ja fout">toegestaan</span></td>
                  <td><span class="chip ja fout">toegestaan</span></td>
                </tr>
              </tbody>
            </table>
            <p class="nadruk">
              Zelfde verbod, zelfde zone. Alleen de bestemming verschilt, en daarmee of wij het
              artikel hadden.
            </p>
          </div>
        </div>
      </section>

      <!-- 8. RFC-057 (pull request 1997, nog in behandeling). Wat de PoC zou aanpassen is een
           voorstel, nog niet gedaan. "Wat open blijft" staat deels in de RFC zelf ("Not decided
           here") en is deels onze lezing voor de Spoorzone. -->
      <section v-else-if="nu === 7" class="dia">
        <div class="rfc-kop">
          <div>
            <p class="kicker">Het voorstel</p>
            <h2>De plek wordt een waarde in de wet, en de engine kiest zelf de regeling</h2>
          </div>
          <div class="rfc-badge">
            <span class="rfc-nr">RFC-057</span>
            <span class="rfc-sub">voorstel in behandeling</span>
            <span class="rfc-sub">pull request #1997</span>
          </div>
        </div>
        <ol class="keten vier">
          <li><strong>Locatie als vorm</strong><span>Wie de vraag stelt, geeft een punt of een vlak mee. Geen gemeente, geen lijst van gebieden.</span></li>
          <li><strong>Gebied als uitkomst</strong><span>Het artikel of de bijlage die een gebied aanwijst, geeft de vorm van dat gebied als uitkomst.</span></li>
          <li><strong>Overlapt het?</strong><span>Een artikel toetst zelf of de locatie zijn gebied raakt, met de nieuwe operatie OVERLAPS.</span></li>
          <li><strong>Welke gemeente</strong><span>De engine legt de locatie tegen de gemeentegrens uit de Kadasterwet en vraagt het plan van die gemeente, artikel voor artikel.</span></li>
        </ol>
        <div class="vakken">
          <div class="vak neutraal">
            <h3>Wat we aanpassen</h3>
            <ul>
              <li>In het schema: het type <code>shape</code>, de operatie <code>OVERLAPS</code>, en op een open term <code>location</code>, <code>territory</code> en <code>combine</code>.</li>
              <li>In de PoC: de vlakken van de kaart als vorm in de YAML, en de toets op de plek van de app naar de engine.</li>
            </ul>
          </div>
          <div class="vak idee">
            <h3>Welk probleem dit oplost</h3>
            <ul>
              <li>Nu kiest de app welke regels op een punt gelden, en die keuze wordt nergens vastgelegd. Straks staat de toets op de plek in de uitleg van de engine.</li>
              <li>“Er is hier geen plan” en “het plan verbiedt dit niet” blijven twee verschillende antwoorden.</li>
            </ul>
          </div>
          <div class="vak">
            <h3>Wat open blijft</h3>
            <ul>
              <li>“Geheel binnen het bouwvlak” kan nog niet: de toets is “raakt het gebied”.</li>
              <li>Grote kaarten: Woongebieden 2018 heeft duizenden vlakken.</li>
              <li>Welke artikelen bij de activiteit horen, en welk plan een ander opzij zet.</li>
            </ul>
          </div>
        </div>
      </section>

      <!-- 9. De activiteit -->
      <section v-else-if="nu === 8" class="dia">
        <p class="kicker">De activiteit</p>
        <h2>Geen bron kent “woongebouw bouwen”: het werd een bundel</h2>
        <div class="bundel">
          <div class="bouwplan">
            <strong>Het bouwplan</strong>
            <span>Gestapeld woongebouw, 5 lagen, 20 woningen</span>
          </div>
          <span class="bundel-pijl" aria-hidden="true">→</span>
          <ul class="bundel-lijst" aria-label="De acht activiteiten in de bundel">
            <li v-for="a in BUNDEL" :key="a.naam"><b>{{ a.naam }}</b><span>{{ a.wie }}</span></li>
          </ul>
        </div>
        <p class="klein bundel-onder">
          Acht activiteiten, met de hand gekozen. Een regel telt alleen mee als hij bij een van de
          acht hoort. Dat is een keuze, geen bron.
        </p>
        <div class="vakken">
          <div class="vak">
            <h3>Nieuw deel: de activiteit is een gegeven</h3>
            <ul>
              <li>866 namen in het gebied, 35 gekozen.</li>
              <li>Te ruim: het Bbl hangt 804 regels aan één activiteit.</li>
              <li>Te krap: bouwen bij het spoor (Bal 9.31) viel buiten de bundel.</li>
              <li>Wat de regel doet, staat er niet: 96 % is “anders geduid”.</li>
            </ul>
          </div>
          <div class="vak">
            <h3>Oude plannen: de activiteit is een kopje</h3>
            <ul>
              <li>“Bouwregels” en “Specifieke gebruiksregels” zijn te herkennen.</li>
              <li>“Algemeen” en “Toepassingsregel” niet: 123 van de 3.812 artikelen bleven zonder.</li>
            </ul>
          </div>
          <div class="vak idee">
            <h3>Idee: de activiteit als open term</h3>
            <p>
              Artikel 22.29 noemt “de regels over het bouwen”. Die open term bakent de activiteit
              al af. Elk artikel dat erbij hoort, zegt dat zelf.
            </p>
            <p class="klein">
              Dan is er geen aparte lijst van activiteiten nodig. Nog niet nagelopen: of gebruiken
              en funderen ook zo'n artikel hebben.
            </p>
          </div>
        </div>
      </section>

      <!-- 10. Elke overheid eigen activiteiten. Namen en groepen uit de annotaties van de
           regelingen in het DSO (pre-productie): "Gebouw bouwen"
           in gm0345 (omgevingsplan), "Bouwactiviteit (technisch)" in mnre1034 (reg0001), "Bouwen van
           een bodemgevoelig gebouw" in pv26, "Bouwwerk aanleggen, wijzigen of verwijderen bij een
           niet verholen waterkering" in ws0662; bouwen bij een hoofdspoorweg in mnre1034. Bouwlaag:
           Woongebieden 2018 artikel 1.28 en ontwerp-TAM Het Ambacht artikel 1.12. Dat de
           kap in het ene plan wel en in het andere niet meetelt, is onze lezing van die twee zinnen. -->
      <section v-else-if="nu === 9" class="dia">
        <p class="kicker">De activiteit</p>
        <h2>Elke overheid benoemt haar eigen activiteiten</h2>
        <div class="namen">
          <p class="namen-kop">
            Landelijk liggen alleen de groepen vast. Zo heet bouwen bij de vier overheden rond de
            Spoorzone:
          </p>
          <ul class="namen-lijst" aria-label="Vier namen voor bouwen, elk met de groep">
            <li><span>Gemeente Veenendaal</span><b>Gebouw bouwen</b><i>groep: bouwactiviteit ruimtelijk</i></li>
            <li><span>Rijk</span><b>Bouwactiviteit (technisch)</b><i>groep: bouwactiviteit technisch</i></li>
            <li><span>Provincie Utrecht</span><b>Bouwen van een bodemgevoelig gebouw</b><i>groep: gebruiken-van-bouwwerkenactiviteit</i></li>
            <li><span>Waterschap Vallei en Veluwe</span><b>Bouwwerk aanleggen, wijzigen of verwijderen bij een niet verholen waterkering</b><i>groep: beperkingengebiedactiviteit m.b.t. een waterkering</i></li>
          </ul>
          <p class="namen-band">Vier namen in vier groepen. Wie zoekt op de groep “bouwactiviteit” vindt alleen de eerste twee.</p>
        </div>
        <div class="vakken">
          <div class="vak">
            <h3>Zelfde handeling, andere naam</h3>
            <ul>
              <li>De naam is vrije tekst. Elke overheid geeft haar activiteit een eigen identificatie.</li>
              <li>De boom van activiteiten helpt alleen binnen één regeling.</li>
              <li>Bouwen bij het spoor staat bij het rijk in weer een andere groep.</li>
            </ul>
          </div>
          <div class="vak">
            <h3>Zelfde woord, andere betekenis</h3>
            <p>
              Een bouwlaag is in Woongebieden 2018 “met uitsluiting van onderbouw en zolder”, in het
              ontwerp-TAM Het Ambacht “met inbegrip van de begane grond en de kap”.
            </p>
            <p class="klein">
              Vier verdiepingen met een kap: vier bouwlagen in het ene plan, vijf in het andere.
            </p>
          </div>
          <div class="vak idee">
            <h3>Wat dit vraagt</h3>
            <p>
              Zoeken op activiteit vindt regels die erover kunnen gaan. Of een regel geldt, volgt
              uit de tekst en de definities van die regeling.
            </p>
            <p class="klein">
              Dus: de feiten van het bouwplan als invoer, en elke regeling telt en benoemt zelf.
            </p>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.deck {
  --dk-bg: light-dark(#f3f3f1, #121417);
  --dk-dia: light-dark(#ffffff, #1c1f24);
  --dk-ink: light-dark(#1b1b1b, #ececea);
  --dk-muted: light-dark(#5a5a57, #a9aaa6);
  --dk-lijn: light-dark(#dcdcd8, #33363c);
  --dk-accent: light-dark(#154273, #8fb4e3);
  --dk-vlak: light-dark(#eef3f9, #22303f);
  --dk-ja: #0ca30c;
  --dk-onth: #fab219;
  --dk-niet: #d03b3b;

  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  flex-direction: column;
  background: var(--dk-bg);
  color: var(--dk-ink);
  font-family: var(--primitives-font-family-body, system-ui, sans-serif);
  outline: none;
}

/* Bovenbalk met titel, stippen en knoppen. */
.balk {
  flex: none;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--dk-lijn);
  background: var(--dk-dia);
}
.balk-titel { font-weight: 700; font-size: 14px; color: var(--dk-accent); white-space: nowrap; }
.stippen { flex: 1; display: flex; justify-content: center; gap: 8px; }
.stip {
  width: 10px; height: 10px; padding: 0; border-radius: 50%;
  border: 1.5px solid var(--dk-accent); background: transparent; cursor: pointer;
}
.stip.actief { background: var(--dk-accent); }
.stip:focus-visible { outline: 2px solid var(--dk-accent); outline-offset: 2px; }
.balk-rechts { display: flex; align-items: center; gap: 6px; }
.teller { font-size: 13px; color: var(--dk-muted); min-width: 44px; text-align: center; font-variant-numeric: tabular-nums; }

/* Podium: de dia blijft 16:9 en schaalt mee met het venster. */
.podium {
  flex: 1;
  min-height: 0;
  container-type: size;
  display: grid;
  place-items: center;
  padding: 24px;
}
.dia {
  width: min(100cqw, 100cqh * 16 / 9);
  aspect-ratio: 16 / 9;
  container-type: inline-size;
  box-sizing: border-box;
  padding: 4.2cqi 5cqi 3.4cqi;
  background: var(--dk-dia);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgb(0 0 0 / 0.08), 0 8px 24px rgb(0 0 0 / 0.06);
  display: flex;
  flex-direction: column;
  gap: 1.6cqi;
  overflow: hidden;
  font-size: 1.55cqi;
  line-height: 1.45;
}

/* Tekst */
.kicker {
  margin: 0;
  font-size: 0.82em;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--dk-accent);
}
h2 { margin: 0; font-size: 2.05em; line-height: 1.15; font-weight: 700; text-wrap: balance; }
h3 { margin: 0 0 0.35em; font-size: 1.05em; font-weight: 700; color: var(--dk-accent); }
p { margin: 0 0 0.7em; }
p:last-child { margin-bottom: 0; }
.lead { font-size: 1.2em; color: var(--dk-muted); max-width: 46em; }
.klein-lead { font-size: 1.05em; }
.klein { font-size: 0.85em; color: var(--dk-muted); }
.nadruk { font-weight: 600; }
.voet { margin-top: auto; font-size: 0.85em; color: var(--dk-muted); }
ul { margin: 0 0 0.7em; padding-left: 1.1em; }
li { margin-bottom: 0.35em; }

/* Titeldia */
.dia-titel { justify-content: center; gap: 2.2cqi; padding-left: 7cqi; border-left: 1.2cqi solid var(--dk-accent); }
.dia-titel .groot { font-size: 3.6em; line-height: 1.05; }
.dia-titel .voet { margin-top: 2cqi; }

/* Kolommen */
.twee { display: grid; grid-template-columns: 1fr 1fr; gap: 4cqi; min-height: 0; }
.twee.breed { gap: 3.2cqi; font-size: 0.9em; }
.twee.onder { margin-top: auto; padding-top: 1.6cqi; border-top: 1px solid var(--dk-lijn); }
.kolom { min-width: 0; }

/* Kerncijfers */
.cijfers { margin: 0; display: grid; gap: 1.2cqi; }
.cijfers div { padding: 1cqi 1.4cqi; background: var(--dk-vlak); border-radius: 6px; }
.cijfers dt { font-size: 1.45em; font-weight: 700; color: var(--dk-accent); line-height: 1.2; }
.cijfers dd { margin: 0; color: var(--dk-muted); }

/* Keten van plek naar oordeel */
.keten { list-style: none; margin: 1cqi 0 0; padding: 0; display: grid; grid-template-columns: repeat(6, 1fr); gap: 2.4cqi; }
.keten li {
  position: relative;
  margin: 0;
  padding: 1.4cqi 1.4cqi 1.6cqi;
  background: var(--dk-vlak);
  border-top: 0.4cqi solid var(--dk-accent);
  border-radius: 4px;
  display: flex; flex-direction: column; gap: 0.4em;
}
.keten li:not(:last-child)::after {
  content: '→';
  position: absolute;
  right: -1.9cqi; top: 50%;
  transform: translateY(-50%);
  font-size: 1.4em; color: var(--dk-accent); font-weight: 700;
}
.keten.vier { grid-template-columns: repeat(4, 1fr); }
.keten strong { font-size: 1.05em; }
.keten span { font-size: 0.85em; color: var(--dk-muted); }
.keten-klein { font-weight: 600; color: var(--dk-accent); font-size: 0.95em; }

/* Oordelen */
.oordelen { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5em; }
.pijl { font-size: 0.8em; color: var(--dk-muted); }
.chip {
  display: inline-block;
  padding: 0.12em 0.6em;
  border-radius: 999px;
  font-size: 0.85em;
  font-weight: 700;
  color: #fff;
  white-space: nowrap;
}
.chip.ja { background: var(--dk-ja); }
.chip.onth { background: var(--dk-onth); color: #1b1b1b; }
.chip.niet { background: var(--dk-niet); }
.chip.fout { outline: 0.25cqi dashed var(--dk-niet); outline-offset: 0.2cqi; }

/* Tijdlijn */

/* Trap van toetsen */
.trap { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(4, 1fr); gap: 2cqi; }
.trap li { margin: 0; display: flex; gap: 1cqi; align-items: flex-start; }
.trap li > div { display: flex; flex-direction: column; gap: 0.3em; }
.trap li span:not(.nr) { font-size: 0.95em; color: var(--dk-muted); }
.trap .nr {
  flex: none;
  width: 2.4cqi; height: 2.4cqi;
  border-radius: 50%;
  display: inline-flex; align-items: center; justify-content: center;
  background: var(--dk-accent); color: var(--dk-dia);
  font-weight: 700; font-size: 0.9em;
}
.trap li:nth-child(1) .nr { opacity: 0.55; }
.trap li:nth-child(2) .nr { opacity: 0.7; }
.trap li:nth-child(3) .nr { opacity: 0.85; }
.regels { margin-top: auto; display: grid; grid-template-columns: repeat(3, 1fr); gap: 1.6cqi; }
.regels span {
  padding: 1cqi 1.3cqi;
  background: var(--dk-vlak);
  border-radius: 6px;
  font-size: 0.88em;
  font-weight: 600;
}

/* Tegels met cijfers */
.tegels { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1.8cqi; }
.tegel { padding: 2cqi 1.8cqi; background: var(--dk-vlak); border-radius: 6px; display: flex; flex-direction: column; gap: 0.3em; }
.tegel span:last-child { font-size: 0.88em; color: var(--dk-muted); }
.getal { font-size: 2.7em; white-space: nowrap; font-weight: 700; line-height: 1.1; color: var(--dk-accent); font-variant-numeric: tabular-nums; }
.tegel.rood { background: light-dark(#fbeaea, #3a2020); }
.tegel.rood .getal { color: var(--dk-niet); }

/* Oorzaken */
.kaarten { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1.6cqi; margin-top: auto; }
.kaartje {
  padding: 1.3cqi 1.5cqi;
  border: 1px solid var(--dk-lijn);
  border-left: 0.4cqi solid var(--dk-niet);
  border-radius: 6px;
  display: flex; flex-direction: column; gap: 0.35em;
}
.kaartje span { font-size: 0.88em; color: var(--dk-muted); }
.kaartje:first-child { grid-row: span 2; background: light-dark(#fbeaea, #3a2020); }
.kaartje:first-child span { font-size: 1em; }
.kaartje q { display: block; margin-top: auto; padding-left: 1cqi; border-left: 0.3cqi solid var(--dk-niet); font-style: italic; font-size: 0.9em; quotes: '“' '”'; }
.kaartje .klein { font-size: 0.8em; }

/* Voorbeeld */
blockquote {
  margin: 0 0 0.8em;
  padding: 1cqi 1.6cqi;
  border-left: 0.4cqi solid var(--dk-accent);
  background: var(--dk-vlak);
  font-style: italic;
}
cite { display: block; margin-top: 0.5em; font-style: normal; font-size: 0.85em; color: var(--dk-muted); }
.proef { width: 100%; border-collapse: collapse; margin-bottom: 1.4cqi; }
.proef caption { text-align: left; font-size: 0.88em; color: var(--dk-muted); margin-bottom: 0.6em; }
.proef th, .proef td { padding: 0.7cqi 0.8cqi; border-bottom: 1px solid var(--dk-lijn); text-align: left; }
.proef thead th { font-size: 0.85em; color: var(--dk-muted); font-weight: 600; }

/* Schets van een YAML met een geo-object */
.schets {
  margin: 0.4em 0 0.8em;
  padding: 1.2cqi 1.4cqi;
  background: light-dark(#1d2633, #0c1016);
  color: #e6edf5;
  border-radius: 6px;
  font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
  font-size: 0.78em;
  line-height: 1.5;
  overflow: hidden;
  white-space: pre;
}
.schets .c { color: #8fb4e3; }

/* Dia "Hoe de fout ontstaat" */
.dia-cel { gap: 1.2cqi; padding-top: 3cqi; padding-bottom: 2.6cqi; }
.cel-kop { display: flex; justify-content: space-between; align-items: flex-end; gap: 2cqi; }
.cel-kop h2 { font-size: 1.8em; }
.stapknoppen { display: flex; gap: 0.4cqi; flex: none; }
/* RFC-dia: het nummer van het voorstel rechtsboven */
.rfc-kop { display: flex; justify-content: space-between; align-items: flex-start; gap: 3cqi; }
.rfc-kop h2 { max-width: 22em; }
.rfc-badge { flex: none; display: flex; flex-direction: column; align-items: flex-end; gap: 0.2em; padding: 1.2cqi 1.8cqi; border: 0.25cqi solid var(--dk-accent); border-radius: 8px; background: var(--dk-vlak); }
.rfc-nr { font-size: 2.4em; font-weight: 800; line-height: 1; letter-spacing: 0.02em; color: var(--dk-accent); white-space: nowrap; }
.rfc-sub { font-size: 0.8em; color: var(--dk-muted); white-space: nowrap; }
.stapknoppen button {
  font: inherit; font-size: 0.78em; font-weight: 600; cursor: pointer;
  display: inline-flex; align-items: center; gap: 0.45em;
  padding: 0.4em 0.8em; border-radius: 999px;
  border: 1px solid var(--dk-lijn); background: transparent; color: var(--dk-muted);
}
.stapknoppen button span {
  display: inline-flex; align-items: center; justify-content: center;
  width: 1.5em; height: 1.5em; border-radius: 50%;
  background: var(--dk-lijn); color: var(--dk-ink); font-size: 0.85em;
}
.stapknoppen button.gedaan { color: var(--dk-ink); }
.stapknoppen button.actief { background: var(--dk-accent); border-color: var(--dk-accent); color: var(--dk-dia); }
.stapknoppen button.actief span { background: var(--dk-dia); color: var(--dk-accent); }
.stapknoppen button:focus-visible { outline: 2px solid var(--dk-accent); outline-offset: 2px; }

.cel-body { flex: 1; min-height: 0; display: grid; grid-template-columns: 0.95fr 2.05fr; gap: 2.4cqi; }
.cel-links { display: flex; flex-direction: column; gap: 0.8cqi; min-width: 0; }
.kaartje-svg { width: 100%; height: auto; display: block; }
.kaart-onder { margin: 0; font-size: 0.7em; color: var(--dk-muted); }
.svg-grond { fill: light-dark(#f4f6f2, #1f2522); stroke: var(--dk-lijn); stroke-dasharray: 3 3; }
.svg-gemengd { fill: light-dark(#dbe7f5, #23384f); stroke: var(--dk-accent); stroke-width: 1.5; transition: all 0.3s; }
.svg-gemengd.licht { fill: light-dark(#bcd3ee, #2f5784); stroke-width: 3; }
.svg-bouwvlak { fill: none; stroke: var(--dk-accent); stroke-width: 1; stroke-dasharray: 4 3; }
.svg-zone { fill: light-dark(rgb(208 59 59 / 0.12), rgb(208 59 59 / 0.18)); stroke: var(--dk-niet); stroke-width: 1; transition: all 0.3s; }
.svg-zone.rood { fill: rgb(208 59 59 / 0.32); stroke-width: 2.5; }
.arceer { stroke: var(--dk-niet); stroke-width: 1; opacity: 0.35; }
.svg-leiding { stroke: var(--dk-ink); stroke-width: 2; stroke-dasharray: 8 4; transition: opacity 0.3s; }
.svg-punt { fill: var(--dk-ink); }
.svg-tekst { font-size: 10px; fill: var(--dk-ink); font-family: inherit; }
.svg-tekst.sterk { font-weight: 700; fill: var(--dk-accent); }
.svg-tekst.klein { font-size: 9px; fill: var(--dk-muted); }
.svg-tekst.rood { fill: var(--dk-niet); font-weight: 700; }
.kaartje-svg .dof { opacity: 0.25; }

.uitleg { font-size: 0.82em; }
.uitleg p { margin-bottom: 0.5em; }
.resultaat { font-weight: 600; }
.waarom-lijst { margin: 0 0 0.6em; padding-left: 1.1em; }
.waarom-lijst li { margin-bottom: 0.3em; }
.sleutel-regel { color: var(--dk-niet); }
.sleutel-regel b { color: var(--dk-niet); }

.cel-rechts { min-width: 0; display: grid; grid-template-columns: 1fr 1.9fr 1fr 1fr; gap: 1cqi; align-content: start; }
.regeling { min-width: 0; }
.regeling-kop { margin: 0 0 0.5em; font-size: 0.72em; font-weight: 700; color: var(--dk-accent); line-height: 1.25; }
.regeling-kop span { display: block; font-weight: 400; color: var(--dk-muted); }
.tegeltjes { list-style: none; margin: 0; padding: 0; display: grid; gap: 0.35cqi; }
.regeling.breed .tegeltjes { grid-template-columns: 1fr 1fr; grid-auto-flow: column; grid-template-rows: repeat(8, auto); column-gap: 0.6cqi; }
.tegeltje {
  margin: 0; min-width: 0;
  display: flex; flex-direction: column; gap: 0.1em;
  padding: 0.3em 0.5em;
  font-size: 0.66em; line-height: 1.3;
  border: 1px solid var(--dk-lijn); border-radius: 4px;
  background: var(--dk-dia);
  transition: opacity 0.3s, background 0.3s, border-color 0.3s;
}
.tegeltje .boven { display: flex; justify-content: space-between; align-items: center; gap: 0.4em; min-height: 1.35em; }
.tegeltje b { font-variant-numeric: tabular-nums; }
.tegeltje .t { min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--dk-muted); }
.tegeltje .tag {
  flex: none; font-size: 0.82em; font-weight: 700; text-transform: uppercase; letter-spacing: 0.03em;
  padding: 0.05em 0.4em; border-radius: 3px; background: var(--dk-lijn); color: var(--dk-ink);
}
.tegeltje.stil, .tegeltje.nvt { opacity: 0.32; }
.tegeltje.gevonden { border-color: var(--dk-accent); background: var(--dk-vlak); border-width: 1.5px; }
.tegeltje.gevonden .tag { background: var(--dk-accent); color: var(--dk-dia); }
.tegeltje.telt { border-color: var(--dk-ink); border-width: 1.5px; }
.tegeltje.telt .t { color: var(--dk-ink); }
.tegeltje.gedekt { border-color: var(--dk-ja); background: light-dark(#e6f5e6, #1d3320); border-width: 1.5px; }
.tegeltje.gedekt .tag { background: var(--dk-ja); color: #fff; }
.tegeltje.gemist { border-color: var(--dk-niet); background: light-dark(#fbeaea, #3a2020); border-width: 1.5px; }
.tegeltje.gemist .tag { background: var(--dk-niet); color: #fff; }
.tegeltje.sleutel { outline: 0.25cqi solid var(--dk-niet); outline-offset: 0.15cqi; }
.tegeltje.sleutel .t { color: var(--dk-niet); font-weight: 700; }

/* De activiteit: het bouwplan als bundel */
.bundel { display: grid; grid-template-columns: 0.8fr auto 3fr; gap: 1.6cqi; align-items: center; }
.bouwplan { padding: 1.4cqi 1.6cqi; background: var(--dk-accent); color: var(--dk-dia); border-radius: 6px; display: flex; flex-direction: column; gap: 0.3em; }
.bouwplan span { font-size: 0.85em; }
.bundel-pijl { font-size: 1.8em; color: var(--dk-muted); }
.bundel-lijst { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 0.8cqi; }
.bundel-lijst li { margin: 0; padding: 0.8cqi 1cqi; background: var(--dk-vlak); border-radius: 6px; display: flex; flex-direction: column; gap: 0.1em; }
.bundel-lijst span { font-size: 0.8em; color: var(--dk-muted); }
.bundel-onder { margin: 0; }
.vakken { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 1.6cqi; margin-top: auto; }
.vak { padding: 1.1cqi 1.4cqi; border: 1px solid var(--dk-lijn); border-top: 0.4cqi solid var(--dk-niet); border-radius: 6px; font-size: 0.85em; }
.vak.idee { border-top-color: var(--dk-ja); }
.vak.neutraal { border-top-color: var(--dk-accent); }
.vak code { font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace; font-size: 0.92em; }
.vak h3 { margin-bottom: 0.5em; }
.vak ul { margin: 0; padding-left: 1.1em; }
.vak li { margin-bottom: 0.3em; }

/* Elke overheid eigen activiteiten: vier namen voor bouwen, elk met de eigen groep */
.namen { display: flex; flex-direction: column; }
.namen-kop { margin: 0 0 0.5em; font-size: 0.88em; font-weight: 600; color: var(--dk-muted); }
.namen-lijst { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 0.8cqi; }
.namen-lijst i { margin-top: auto; padding-top: 0.4em; font-style: normal; font-size: 0.8em; color: var(--dk-muted); }
.namen-lijst li { margin: 0; font-size: 0.92em; padding: 0.8cqi 1.1cqi; background: var(--dk-vlak); border-radius: 6px 6px 0 0; display: flex; flex-direction: column; gap: 0.2em; }
.namen-lijst span { font-size: 0.8em; font-weight: 700; color: var(--dk-accent); }
.namen-band { margin: 0; padding: 0.7cqi 1.3cqi; background: var(--dk-accent); color: var(--dk-dia); border-radius: 0 0 6px 6px; font-weight: 600; }

/* Smal scherm: geen 16:9, de dia scrolt en de tekst heeft een vaste maat. */
@media (max-width: 760px) {
  .balk-titel, .stippen { display: none; }
  .balk { justify-content: flex-end; }
  .podium { display: block; overflow-y: auto; padding: 12px 16px; container-type: normal; }
  .dia { width: 100%; aspect-ratio: auto; font-size: 15px; padding: 20px 18px; gap: 14px; overflow: visible; }
  .dia-titel .groot { font-size: 2.2em; }
  .twee, .keten,   .namen-lijst li { border-radius: 6px; }
  .keten li:not(:last-child)::after { content: none; }
  .bundel-lijst { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .bundel-pijl { display: none; }
  .kaartje:first-child { grid-row: auto; }
  .cel-kop { flex-direction: column; align-items: flex-start; }
  .stapknoppen { flex-wrap: wrap; }
  .cel-body, .cel-rechts { grid-template-columns: minmax(0, 1fr); }
  .regeling.breed .tegeltjes { grid-template-columns: 1fr; grid-auto-flow: row; grid-template-rows: none; }
  .tegeltje { font-size: 13px; }
  .schets { font-size: 11px; overflow-x: auto; }
  .trap .nr { width: 24px; height: 24px; }
  .voet { margin-top: 8px; }
}
</style>
