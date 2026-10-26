<script setup lang="ts">
import { computed, onActivated, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore';
import QrCode from '../components/QrCode.vue';
import { useI18n } from '../i18n/index.js';
import { localeRouteName } from '../router';
import { useWalkthrough } from '../walkthrough/useWalkthrough';

// De landingspagina op `/`. Wie de demo opent zonder te weten wat het is, leest
// hier in een paar regels wat er te zien valt, start de presentatie met één
// knop, en vindt de weg naar de rest van RegelRecht.
//
// De QR-code wijst naar deze pagina zelf, zodat iemand in de zaal de demo op
// zijn eigen telefoon kan openen terwijl hij naar het scherm kijkt.

const router = useRouter();
const { t, locale } = useI18n();

/** Het pad van een tabblad in de taal die aan staat. */
function pathFor(page: string) {
  return router.resolve({ name: localeRouteName(page, locale.value) }).path;
}
const { ready } = useDemo();
// Only offered when this build carries a recording.
const { timeline: walkthrough } = useWalkthrough();

// De QR-code moet naar het adres wijzen waar déze pagina draait: productie,
// een preview-deploy of een laptop op het netwerk tijdens een presentatie. Die
// URL vastleggen zou op alle drie op één na fout zijn, dus hij komt uit de
// browser. `origin` en niet `href`: de route eronder verandert tijdens de demo
// mee, en de code hoort naar het beginpunt te leiden.
// Een computed en geen eenmalige `onMounted`: de view blijft door keep-alive
// gemount, dus een taalwissel moet de code meenemen. `origin` staat pas vast
// zodra er een window is, vandaar de ref eromheen.
const origin = ref('');
onMounted(() => {
  origin.value = window.location.origin;
});
const pageUrl = computed(() => {
  // Het pad van de voorpagina in de taal die aan staat, niet een vaste `/`:
  // wie tijdens een Engelse presentatie scant hoort in het Engels te landen.
  //
  // Expliciet 'home' en niet `route.name`: deze view blijft door keep-alive
  // gemount, dus zodra de presentator naar een ander tabblad loopt wijst
  // `route` daarheen en zou de code naar dat tabblad verwijzen. Bij het eerste
  // bezoek valt dat samen en daarom viel het niet op.
  if (!origin.value) return '';
  return `${origin.value}${pathFor('home')}`;
});

// Wat er in de demo te zien is, in de volgorde van de tabbladen erboven. Dit is
// een leeswijzer, geen tweede navigatie: de kaarten brengen je naar hetzelfde
// tabblad waar de tabbalk heen gaat.
// De titels zijn dezelfde als in de tabbalk: het zijn dezelfde tabbladen, dus
// ze lenen de sleutels van App.vue in plaats van een tweede naam te krijgen die
// bij een wijziging kan gaan afwijken.
const onderdelen = computed(() => [
  { icon: 'books', title: t('app.tabs.wetten'), text: t('home.parts.wetten.text'), to: pathFor('wetten') },
  { icon: 'centralized-network', title: t('app.tabs.graaf'), text: t('home.parts.graaf.text'), to: pathFor('graaf') },
  { icon: 'checklist', title: t('app.tabs.scenarios'), text: t('home.parts.scenarios.text'), to: pathFor('scenarios') },
  { icon: 'chart-line', title: t('app.tabs.simulatie'), text: t('home.parts.simulatie.text'), to: pathFor('simulatie') },
  { icon: 'user', title: t('app.tabs.portaal'), text: t('home.parts.portaal.text'), to: pathFor('portaal') },
  { icon: 'inbox', title: t('app.tabs.zaaksysteem'), text: t('home.parts.zaaksysteem.text'), to: pathFor('zaaksysteem') },
]);

// Computed en geen vaste lijst: de teksten moeten bij een taalwissel mee, net
// als `tabs` in App.vue. De eerste titel is een adres en blijft zoals hij is.
const links = computed(() => [
  { icon: 'home', title: 'regelrecht.rijks.app', text: t('home.links.site.text'), href: 'https://regelrecht.rijks.app' },
  { icon: 'document', title: t('home.links.docs.title'), text: t('home.links.docs.text'), href: 'https://docs.regelrecht.rijks.app/docs/' },
  { icon: 'library', title: t('home.links.research.title'), text: t('home.links.research.text'), href: 'https://regelrecht.rijks.app/research/' },
]);

function start() {
  router.push(pathFor('presentatie'));
}

// Drie manieren om binnen te komen, elk met wat het is: drie knoppen naast
// elkaar zeiden niet wat het verschil was tussen een presentatie, een
// rondleiding en zelf rondkijken. De rondleiding staat voorop, want wie hier
// zonder presentator landt heeft daar het meest aan; alleen als deze build
// er een heeft.
const ways = computed(() => [
  ...(walkthrough.value
    ? [
        {
          key: 'walkthrough',
          icon: 'video-camera',
          title: t('home.ways.walkthrough.title'),
          text: t('home.ways.walkthrough.text', { minutes: Math.max(1, Math.round((walkthrough.value.main?.duration ?? 0) / 60)) }),
          go: () => router.push(pathFor('rondleiding')),
        },
      ]
    : []),
  { key: 'browse', icon: 'books', title: t('home.ways.browse.title'), text: t('home.ways.browse.text'), go: () => router.push(pathFor('wetten')) },
  { key: 'present', icon: 'play', title: t('home.ways.present.title'), text: t('home.ways.present.text'), go: start },
]);

// De scrollpositie van een keep-alive-view blijft staan. Voor een pagina waar
// iemand op landt is dat verkeerd: wie via het tabblad terugkomt hoort weer
// bovenaan te beginnen.
const page = ref<HTMLElement | null>(null);
onActivated(() => {
  page.value?.scrollTo?.({ top: 0 });
});
</script>

<template>
  <nldd-page ref="page">
    <!-- Het gekleurde vlak draagt zijn eigen contentkleur, dus titel, tekst en
         de inherit-knoppen erbinnen houden hun contrast zonder scheme-override. -->
    <nldd-hero main-width="full" main-background="donkerblauw" padding-bottom="0">
      <!-- Twee kolommen over de volle hoogte van de hero: het verhaal links, de
           QR-code rechts. In een zaal wordt die code in de eerste seconden
           gescand, dus hij hoort meteen in beeld en groot genoeg om vanaf een
           afstand te lezen. Op een smal scherm zakt hij onder de tekst. -->
      <div class="hero-row">
        <div class="hero-text">
          <nldd-title size="1" color="inherit">
            <span slot="overline">{{ t('app.demo.label') }}</span>
            <h1>RegelRecht</h1>
            <span slot="supporting-text">{{ t('home.hero.subtitle') }}</span>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-rich-text color="inherit">
            <p>{{ t('home.hero.lead') }}</p>
          </nldd-rich-text>
          <nldd-spacer size="24"></nldd-spacer>
          <!-- Kaarten in plaats van knoppen: elke manier zegt in één regel
               wat je krijgt. Dezelfde nldd-card als de kaarten verderop. -->
          <nldd-collection layout="grid" item-width="200px" :accessible-label="t('home.ways.label')">
            <nldd-card v-for="w in ways" :key="w.key" button :accessible-label="w.title" @click="ready && w.go()">
              <nldd-container padding="16" gap="8">
                <!-- An explicit color: in the hero the icon would inherit its
                     white and vanish on the white card. -->
                <nldd-icon :name="w.icon" size="24" color="accent"></nldd-icon>
                <nldd-title size="5">
                  <h2>{{ w.title }}</h2>
                </nldd-title>
                <nldd-rich-text size="sm" spacing="tight">
                  <p>{{ w.text }}</p>
                </nldd-rich-text>
              </nldd-container>
            </nldd-card>
          </nldd-collection>
        </div>
        <QrCode
          v-if="pageUrl"
          class="qr"
          :value="pageUrl"
          :accessible-label="t('home.hero.qr')"
        />
      </div>
    </nldd-hero>

    <!-- Verder lezen staat boven de rondleiding: wie hier landt zonder de demo
         te kennen heeft eerder iets aan de weg naar RegelRecht zelf dan aan een
         beschrijving van tabbladen die vlak boven hem al staan. -->
    <nldd-simple-section>
      <nldd-title slot="header" size="3">
        <h2>{{ t('home.links.title') }}</h2>
        <span slot="supporting-text">{{ t('home.links.subtitle') }}</span>
      </nldd-title>
      <nldd-collection layout="grid" item-width="240px">
        <nldd-card v-for="l in links" :key="l.href" :href="l.href" target="_blank">
          <nldd-container padding="16" gap="8">
            <nldd-icon :icon="l.icon" size="24"></nldd-icon>
            <nldd-title size="5">
              <h3>{{ l.title }}</h3>
            </nldd-title>
            <nldd-rich-text size="sm" spacing="tight">
              <p>{{ l.text }}</p>
            </nldd-rich-text>
          </nldd-container>
        </nldd-card>
      </nldd-collection>
    </nldd-simple-section>

    <nldd-simple-section background="tinted">
      <nldd-title slot="header" size="3">
        <!-- Een kop die met "Wat ..." begint leest als een tussenkop uit een
             gegenereerde tekst; de sectie is een lijst van onderdelen, dus zij
             heet naar wat zij toont. -->
        <h2>{{ t('home.parts.title') }}</h2>
        <span slot="supporting-text">{{ t('home.parts.subtitle') }}</span>
      </nldd-title>
      <nldd-collection layout="grid" item-width="240px">
        <nldd-card v-for="o in onderdelen" :key="o.to" button @click="router.push(o.to)">
          <nldd-container padding="16" gap="8">
            <nldd-icon :icon="o.icon" size="24"></nldd-icon>
            <nldd-title size="5">
              <h3>{{ o.title }}</h3>
            </nldd-title>
            <nldd-rich-text size="sm" spacing="tight">
              <p>{{ o.text }}</p>
            </nldd-rich-text>
          </nldd-container>
        </nldd-card>
      </nldd-collection>
    </nldd-simple-section>
  </nldd-page>
</template>

<style scoped>
/* De QR-code heeft een witte ondergrond nodig om leesbaar te blijven, ook in
 * donkere modus. De code tekent zijn eigen witte vlak; dit kadertje geeft hem
 * een maximale maat en een radius die bij de rest van het vlak past. */
/* Tekst links, QR-code rechts, over de volle hoogte van de hero. Onder de
 * md-grens (1007px) stapelt het: de code zakt dan onder de tekst. */
.hero-row {
  display: flex;
  flex-wrap: wrap;
  align-items: stretch;
  gap: var(--primitives-space-32, 32px);
}

/* De tekst houdt een leesbare minimumbreedte; wordt het krapper, dan wikkelt de
   rij en zakt de code eronder. Dat is een eigenschap van de rij zelf en niet
   van het venster, dus geen media query: die keek naar de vensterbreedte en
   liet de kolom in een smalle container tot 54px samenknijpen. */
.hero-text {
  flex: 1 1 320px;
  min-width: min(100%, 320px);
}

/* De code draagt zijn eigen witte rand (de stille zone zit in de SVG), dus er
 * is geen kadertje omheen nodig.
 *
 * De maat komt van de BREEDTE, niet van de uitgerekte hoogte. `align-self:
 * stretch` samen met `aspect-ratio` gaf een lus: de code rekte tot de hoogte
 * van de rij, die hoogte bepaalde zijn breedte, en dat duwde de rij weer hoger
 * — de hero werd 5000px hoog en de tekstkolom 0 breed. */
.qr {
  flex: 0 0 auto;
  align-self: center;
  width: clamp(144px, 14vw, 240px);
  border-radius: var(--primitives-radius-8, 8px);
  overflow: hidden;
}

</style>
