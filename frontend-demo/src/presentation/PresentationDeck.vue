<script setup lang="ts">
import { computed } from 'vue';
import { usePresentation } from './usePresentation';
import { useDemo } from '../store/demoStore';
import { intlLocale } from '../data/format';
import { useI18n } from '../i18n/index.js';
import { HINTS, hintSegments } from './keyHints';
import { clockEpoch } from '../walkthrough/clock';

// The deck: a Rijkshuisstijl-blue panel, full-screen for the intro and the
// closing, a left rail while the live demo runs on the right. Slides are data
// (demo-config.yaml); this component only knows how to render each kind.

const p = usePresentation();
const { state } = useDemo();
const { t } = useI18n();

// Een computed: het dek blijft staan tijdens een taalwissel, dus een datum die
// eenmalig is uitgerekend zou in de oude taal blijven hangen.
// Ook afhankelijk van `clockEpoch`: tijdens een opgenomen rondleiding staat
// de klok op het moment van opnemen, en de datum moet daarna weer vandaag zijn.
const today = computed(() => {
  void clockEpoch.value;
  return new Date().toLocaleDateString(intlLocale(), { day: 'numeric', month: 'long', year: 'numeric' });
});
const counter = computed(() => `${p.index.value + 1} / ${p.total.value}`);
const progress = computed(() => (p.total.value ? `${((p.index.value + 1) / p.total.value) * 100}%` : '0%'));
const isLast = computed(() => p.index.value === p.total.value - 1);
// Een computed, zodat de toetsregel meeverandert bij een taalwissel.
const hints = computed(() => HINTS.map((id) => ({ id, segments: hintSegments(t(id)) })));

/** `**bold**` in a statement line → <strong>, everything else escaped. */
function emphasize(line: string): string {
  const escaped = String(line).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  return escaped.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
}
function saveName(e: Event) {
  state.presenterName = (e.target as HTMLInputElement | null)?.value ?? state.presenterName;
}
</script>

<template>
  <Teleport to="body">
    <div v-if="p.active.value && p.current.value && p.visible.value" class="deck" :class="{ full: p.isFull.value }" role="region" :aria-label="t('deck.label')">
      <!-- Het podium: de tekstkolom van de dia. Op het hele scherm is dat een
           gecentreerde kolom van hooguit 1600px, in de rail de hele kolom. In
           beide gevallen is dit de container waar de typografie zich op meet,
           zodat één ladder voor allebei volstaat. -->
      <div class="stage">
      <div class="content">
        <div class="content-main">
          <!-- Title slide -->
          <template v-if="p.current.value.kind === 'title'">
            <span class="overline">{{ today }}</span>
            <h1 class="title title-hero">{{ p.current.value.title }}</h1>
            <p v-if="p.current.value.subtitle" class="lead lead-hero">{{ p.current.value.subtitle }}</p>
            <div class="title-meta">
              <input class="presenter" :value="state.presenterName" :placeholder="t('deck.presenter_name')" :aria-label="t('deck.presenter_name')" @change="saveName" />
              <span v-if="p.current.value.footer" class="affiliation">{{ p.current.value.footer }}</span>
            </div>
          </template>

          <!-- Statement slide: a few big lines -->
          <template v-else-if="p.current.value.kind === 'statement'">
            <span v-if="p.current.value.overline" class="overline">{{ p.current.value.overline }}</span>
            <h1 class="title statement">
              <span v-for="(line, j) in p.current.value.lines" :key="j" class="statement-line" v-html="emphasize(line)"></span>
            </h1>
          </template>

          <!-- Section or closing slide: a big title with a few plain lines -->
          <template v-else-if="p.current.value.kind === 'closing' || p.current.value.kind === 'section'">
            <span v-if="p.current.value.overline" class="overline">{{ p.current.value.overline }}</span>
            <h1 class="title title-hero">{{ p.current.value.title }}</h1>
            <ul v-if="p.current.value.lines?.length" class="bullets bullets-plain">
              <li v-for="(line, j) in p.current.value.lines" :key="j" v-html="emphasize(line)"></li>
            </ul>
            <a v-if="p.current.value.link" class="slide-link" :href="p.current.value.link.href" target="_blank" rel="noopener noreferrer">{{ p.current.value.link.label }}</a>
          </template>

          <!-- Demo slide: the rail next to the live app -->
          <template v-else>
            <span v-if="p.current.value.overline" class="overline">{{ p.current.value.overline }}</span>
            <h1 class="title">{{ p.current.value.title }}</h1>
            <p v-if="p.current.value.lead" class="lead">{{ p.current.value.lead }}</p>
            <ul v-if="p.current.value.bullets?.length" class="bullets">
              <li v-for="(b, j) in p.current.value.bullets" :key="j" v-html="emphasize(b)"></li>
            </ul>
          </template>
        </div>
      </div>
      </div>

      <!-- Slots for the recorded walkthrough (walkthrough/Replay*.vue): under
           the slide its bubble and questions, in place of the footer its
           controls. Empty, the deck is the presentation it always was. -->
      <slot name="aside"></slot>
      <slot name="footer">
      <div class="footer">
        <!-- De tellerregel en de toetsenregel staan links onder elkaar; de
             knoppen staan daar rechts naast, gecentreerd over allebei. Eerder
             stonden ze op de tellerregel, waardoor ze hoog naast een lege regel
             hingen terwijl de toetsen eronder de breedte vulden. -->
        <div class="footer-text">
          <span class="counter" :aria-label="t('deck.slide_of', { n: p.index.value + 1, total: p.total.value })">{{ counter }}</span>
          <!-- De toetsen als echte toetsen: nldd-keyboard-shortcut rendert een
               <kbd> per toets, met de OS-detectie en de semantiek erbij. Dit
               waren drie <span>'s met een eigen tekstkleur. `color="inherit"`
               is er voor een gevuld vlak zoals dit dek. -->
          <div class="hints">
            <!-- Elke pijl apart, niet `←+→`: dat zet er een plusteken tussen en
                 leest als 'allebei tegelijk', terwijl het hier om de een of de
                 ander gaat. De zinnen komen uit het woordenboek met een
                 `{placeholder}` per toets (keyHints.ts); stonden ze hier als
                 tekst, dan bleven ze Nederlands in elke taal. -->
            <span v-for="hint in hints" :key="hint.id" class="hint">
              <template v-for="(seg, j) in hint.segments" :key="j">
                <nldd-keyboard-shortcut v-if="seg.key" size="sm" color="inherit" :keys="seg.key" always-visible></nldd-keyboard-shortcut>
                <template v-else>{{ seg.text }}</template>
              </template>
            </span>
          </div>
        </div>
        <!-- Knoppen uit het design system in plaats van eigen <button>'s met
             ingetekende chevrons. De `inherit`-varianten zijn hier precies voor
             gemaakt: ze leiden hun kleur af van `currentColor`, dus ze kloppen
             op het donkerblauwe vlak zonder eigen kleurregels. -->
        <nldd-button-bar>
          <nldd-icon-button
            appearance="inherit-tinted"
            icon="back"
            :text="t('deck.previous')"
            tooltip-timing="never"
            :disabled="p.index.value === 0 || undefined"
            @click="p.prev()"
          ></nldd-icon-button>
          <nldd-button
            v-if="isLast"
            appearance="inherit-tinted"
            :text="t('deck.close')"
            @click="p.stop()"
          ></nldd-button>
          <nldd-icon-button
            v-else
            appearance="inherit-tinted"
            icon="forward"
            :text="t('deck.next')"
            tooltip-timing="never"
            @click="p.next()"
          ></nldd-icon-button>
        </nldd-button-bar>
      </div>
      </slot>
      <div class="progress" aria-hidden="true"><div class="progress-fill" :style="{ width: progress }"></div></div>
    </div>
  </Teleport>
</template>

<style scoped>
/* Custom CSS on purpose: the design system has no presentation component. De
 * kleuren komen wél uit het design system, als tokens en niet als hex. Dat is
 * niet alleen netter: de hexen die hier stonden bestónden niet in het palet
 * (#154273 zat er 5 naast lintblauw-750, #ffb612 zelfs 19 naast donkergeel-200),
 * en een vaste hex negeert `light-dark()`, waar het hele palet op gebouwd is.
 * De typografie blijft RijksoverheidSerif voor titels en RijksSans voor de rest.
 *
 * Het dek is altijd donkerblauw met witte tekst, ook als de bezoeker in donkere
 * modus kijkt: een presentatie heeft één verschijning, en op een beamer is dat
 * die. Daarom `color-scheme: light` op het dek — niet omdat het licht is, maar
 * omdat de kleurschalen omkeren: `lintblauw-750` is donkerblauw in lichte modus
 * (L 0.39) en juist lichtblauw in donkere (L 0.76), en `donkergeel-200` gaat van
 * helder geel naar donkerbruin. De lichte kant van de schaal is hier de goede,
 * in beide modi. */
.deck {
  /* De maat volgt de BREEDTE van het vlak waarin de tekst staat (`1cqw`), niet
     de kortste zijde. Met `cqmin` won op elk normaal venster de hoogte, en die
     zegt niets over hoe groot een letter mag zijn: op een breed scherm stond de
     titel op 26px in een vlak van 720px, met de rest van de dia leeg. Een dia
     vult zijn regel; de hoogte begrenst, maar is geen maatstaf. */
  container-type: inline-size;
  /* Twee rollen, twee schalen. Op het hele scherm is de dia het beeld en mag
     de titel de regel vullen; in de rail is hij een bijschrift naast de demo,
     en daar is diezelfde maat schreeuwerig. `--scale` is de enige knop: de
     rail zet hem lager, de rest van de typografie hieronder is één ladder. */
  /* De rail is geen verkleinde dia maar een leespaneel naast de demo. Eén
     procent van een kolom van 630px is 6,3px, en de dia-ladder maakte daar
     titels van 28px en opsommingen van 13px van: kleiner dan de tekst van de
     demo ernaast. De ondergrens tilt de hele ladder in één keer op, zodat de
     verhoudingen blijven kloppen en alleen het formaat leesbaar wordt. */
  --scale: 1;
  --slide-floor: 11px;
  position: fixed;
  inset: 0 auto 0 0;
  width: 36vw;
  z-index: 80;
  display: flex;
  flex-direction: column;
  /* Onder ruimer dan eerst (1.5rem): de bedieningsregel bestaat nu uit echte
     keycaps in plaats van platte tekst, en die zijn hoger. Ze stonden daardoor
     tegen de voortgangsbalk aan geplakt. */
  padding: 3rem 2.75rem 3rem;
  box-sizing: border-box;
  color-scheme: light;
  /* De inkt van het dek: één token, en elke doorzichtige variant eruit afgeleid
     met `color-mix`. Zo staat de kleur één keer in het bestand in plaats van
     twintig keer als `rgba(255, 255, 255, …)`, en volgt een wijziging in het
     palet vanzelf. */
  /* `coolgray-0`, niet `neutral-0`: de neutrale schaal van dit design system
     heet coolgray. `neutral-*` bestaat niet, en zo'n naam faalt stil — de tekst
     leek wit omdat een ongeldige `color` de geërfde waarde laat staan. */
  --ink: var(--primitives-color-coolgray-0);
  --ink-96: color-mix(in srgb, var(--ink) 96%, transparent);
  --ink-85: color-mix(in srgb, var(--ink) 85%, transparent);
  --ink-72: color-mix(in srgb, var(--ink) 72%, transparent);
  --ink-55: color-mix(in srgb, var(--ink) 55%, transparent);
  --ink-35: color-mix(in srgb, var(--ink) 35%, transparent);
  --ink-18: color-mix(in srgb, var(--ink) 18%, transparent);
  color: var(--ink);
  background: linear-gradient(
    160deg,
    var(--primitives-color-lintblauw-750) 0%,
    var(--primitives-color-lintblauw-700) 100%
  );
  box-shadow: 4px 0 24px rgb(0 0 0 / 0.25);
  font-family: 'RijksSans', system-ui, sans-serif;
  transition: width 0.5s cubic-bezier(0.22, 1, 0.36, 1), padding 0.5s cubic-bezier(0.22, 1, 0.36, 1);
}
/* Een dia op het hele scherm is geen vlak met marges eromheen maar een
   gecentreerde tekstkolom met een begrensde regellengte. Zonder die grens
   bepaalde het vensterformaat de regel: op 2000px bleef er na de marges 1680px
   over voor een regel van 26px, en de dia was vooral leeg blauw. */
.deck.full {
  width: 100vw;
  /* Onderin ruimte voor de bedieningsregel: die bestaat uit keycaps en die
     stonden anders tegen de voortgangsbalk aan. */
  padding: 2vh 0 1.5rem;
  /* De deck zelf meet niets meer: het podium binnenin is de container. */
  container-type: normal;
}
.deck.full .stage {
  /* Het podium vult de vrije ruimte; het dwingt géén 16:9 af. Dat deed het
     eerder wel, en dan bepaalde de vorm van het vlak of de tekst paste: de
     titeldia is hoger dan 16:9 bij deze lettergrootte, en "Nederlandse
     Digitale Dienst" werd er onderaan afgesneden.
     De maat komt nu van de breedte (`--slide-measure`), begrensd op een
     leesbare regellengte, en de hoogte is vrij. Zo bepaalt de inhoud de
     hoogte en de regellengte de typografie, in plaats van andersom. */
  --scale: 1;
  --slide-floor: 0px;
  container-type: inline-size;
  flex: 1 1 auto;
  min-height: 0;
  width: min(100%, 1600px);
  margin: 0 auto;
  padding: 3vh clamp(2rem, 5vw, 6rem);
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}
/* De voettekst en de voortgangsbalk horen bij de deck, niet bij de dia: ze
   staan naast het verhaal en mogen het podium niet uit verhouding duwen. */
.deck.full .footer {
  padding-inline: clamp(1.5rem, 4vw, 4rem);
}

/* In de rail is het podium gewoon de kolom: geen 16:9, want daar staat de dia
   naast de demo en is hij een bijschrift, geen beeld. */
.stage {
  /* `--slide-unit` staat hier en niet op de deck: `1cqw` moet zich meten aan
     het podium, en `--scale` moet de waarde van dít element zijn. Op de deck
     werd de rail-schaal van 0.62 ook op een volledige dia toegepast. */
  --slide-unit: max(var(--slide-floor, 0px), calc(1cqw * var(--scale)));
  container-type: inline-size;
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* A slide never scrolls. Scrolling hides the bottom of an argument behind a
 * gesture nobody makes while presenting, and on a projector the speaker cannot
 * see that there is more. De typografie schaalt in plaats daarvan mee met de
 * breedte van het podium (`--slide-unit`, een percentage daarvan), met een
 * ondergrens in de rail zodat het bijschrift daar niet kleiner wordt dan de
 * tekst van de demo ernaast. */
.content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.content-main {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: max(0.5rem, calc(1.6 * var(--slide-unit)));
}

/* Eén typografische ladder, in stappen van `--slide-unit` (een percentage van
   de podiumbreedte maal `--scale`). Geen `clamp()`-plafonds meer: die kapten
   op een beamer juist af wat daar goed was, en de vloer is overbodig zolang de
   dia een podium met vaste verhoudingen is dat zelf niet kleiner wordt dan het
   venster. Alleen een ondergrens blijft, voor een heel smal venster. */
.overline {
  font-size: max(0.72rem, calc(1.6 * var(--slide-unit)));
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--ink-72);
}
.title {
  font-family: 'RijksoverheidSerif', Georgia, serif;
  font-weight: 700;
  font-size: max(1.2rem, calc(5.2 * var(--slide-unit)));
  line-height: 1.06;
  letter-spacing: -0.015em;
  text-wrap: balance;
  margin: 0;
  color: var(--ink);
}
.title-hero {
  font-size: max(1.6rem, calc(8.5 * var(--slide-unit)));
}
.statement {
  font-size: max(1.1rem, calc(5.2 * var(--slide-unit)));
  line-height: 1.18;
  font-weight: 400;
  letter-spacing: -0.01em;
}
/* One authored line per block; a line that still has to wrap balances its
 * halves instead of leaving one word behind. */
.statement-line {
  display: block;
  text-wrap: balance;
}
.statement :deep(strong) {
  font-weight: 700;
}
.lead {
  font-size: max(0.85rem, calc(2.6 * var(--slide-unit)));
  line-height: 1.4;
  margin: 0;
  font-weight: 500;
  text-wrap: pretty;
}
.lead-hero {
  font-family: 'RijksoverheidSerif', Georgia, serif;
  font-style: italic;
  font-weight: 400;
  font-size: max(1rem, calc(3.6 * var(--slide-unit)));
  color: color-mix(in srgb, var(--ink) 90%, transparent);
}
.bullets {
  font-size: max(0.8rem, calc(2.2 * var(--slide-unit)));
  line-height: 1.45;
  margin: 0;
  padding-left: 1.3em;
  display: flex;
  flex-direction: column;
  gap: calc(0.9 * var(--slide-unit));
  color: var(--ink-96);
  text-wrap: pretty;
}
.bullets li::marker {
  color: var(--ink-72);
}
.bullets-plain {
  list-style: none;
  padding-left: 0;
  font-size: max(0.9rem, calc(3 * var(--slide-unit)));
}
.bullets :deep(strong) {
  font-weight: 700;
}
.title-meta {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-top: calc(1.5 * var(--slide-unit));
  font-size: max(0.9rem, calc(1.9 * var(--slide-unit)));
  color: var(--ink-85);
}
.presenter {
  font: inherit;
  font-weight: 600;
  color: var(--ink);
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--ink-35);
  padding: 0.1rem 0;
  width: min(24ch, 100%);
  outline: none;
}
.presenter::placeholder {
  color: color-mix(in srgb, var(--ink) 50%, transparent);
}
.presenter:focus {
  border-bottom-color: var(--ink);
}
.slide-link {
  align-self: flex-start;
  margin-top: calc(1.5 * var(--slide-unit));
  font-size: max(0.95rem, calc(2 * var(--slide-unit)));
  font-weight: 600;
  color: var(--ink);
  text-decoration: underline;
  text-underline-offset: 4px;
}

.footer {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding-top: 1rem;
}
/* Teller en toetsenregel onder elkaar; de knoppen ernaast zijn over die twee
   regels heen gecentreerd (`align-items: center` op de footer). */
.footer-text {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  min-width: 0;
}
.counter {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--ink-72);
  font-variant-numeric: tabular-nums;
}
.hints {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem 1rem;
  font-size: 0.8rem;
  color: color-mix(in srgb, var(--ink) 70%, transparent);
}
/* De toetsen staan op de regel van hun uitleg, met de tekst ertussen. */
.hint {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}
.progress {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 3px;
  background: var(--ink-18);
}
.progress-fill {
  height: 100%;
  background: var(--primitives-color-donkergeel-200);
  transition: width 0.3s ease;
}
/* The walkthrough's controls replace the footer, so on a full slide they take
   the footer's inset too. */
.deck.full :slotted(.controls) {
  padding-inline: clamp(1.5rem, 4vw, 4rem);
}
@media (max-width: 1024px) {
  .deck {
    width: 100vw;
  }
}
</style>
