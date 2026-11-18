<template>
  <nldd-shell>
    <div class="app-frame">
      <nldd-skip-link text="Direct naar de inhoud">
      <nldd-top-navigation-bar
        logo-title="Nieuwkomersbekostiging"
        logo-subtitle="OCW/DUO-casus · knelpuntenbrief DUO 2025"
        :logo-href="b('/beleid')"
        :website-href="b('/beleid')"
        @click.prevent
      >
        <nldd-menu-bar slot="global">
          <nldd-menu-bar-item
            text="Beleid"
            icon="settings"
            :current="route.name === 'beleid' ? true : undefined"
            @click="router.push('/beleid')"
          ></nldd-menu-bar-item>
          <nldd-menu-bar-item
            text="Casussen"
            icon="user"
            :current="route.name === 'casussen' ? true : undefined"
            @click="router.push('/casussen')"
          ></nldd-menu-bar-item>
          <nldd-menu-bar-item
            text="School"
            icon="building"
            :current="route.name === 'school' ? true : undefined"
            @click="router.push('/school')"
          ></nldd-menu-bar-item>
        </nldd-menu-bar>
        <nldd-menu-bar slot="utility">
          <nldd-menu-bar-item
            :text="themaTekst"
            :icon="themaIcoon"
            @click="volgende"
          ></nldd-menu-bar-item>
        </nldd-menu-bar>
      </nldd-top-navigation-bar>
      <werkversie-balk />
      </nldd-skip-link>
      <!-- Meldingen van de assistent: hier en niet in het paneel, want op de
           andere pagina's is dat paneel niet gemonteerd. -->
      <assistant-notices
        :notices="meldingen"
        :on-assistant-page="route.name === 'beleid'"
        go-to-label="Naar de assistent"
        @dismiss="wisMelding"
        @go="router.push('/beleid')"
      />
      <main id="hoofdinhoud" class="app-main" tabindex="-1">
        <router-view />
      </main>
    </div>
  </nldd-shell>
</template>

<script setup>
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useThema } from './composables/useThema.js';
import { b } from './basePad.js';
import WerkversieBalk from './components/WerkversieBalk.vue';
import AssistantNotices from '@regelrecht/frontend-shared/components/AssistantNotices.vue';
import { useAssistent } from './composables/useAssistent.js';

const route = useRoute();
const router = useRouter();
const { thema, volgende } = useThema();
const { meldingen, wisMelding } = useAssistent();

// Het icoon toont wat je krijgt als je klikt, niet waar je nu staat: een
// knop die zijn eigen huidige stand afbeeldt, leest als een statuslampje.
const themaIcoon = computed(() => (thema.value === 'donker' ? 'light-mode' : 'dark-mode'));
const themaTekst = computed(() => (thema.value === 'donker' ? 'Licht' : 'Donker'));
</script>

<style scoped>
.app-frame {
  display: flex;
  flex-direction: column;
  /* nldd-shell clipt zijn inhoud (overflow hidden in de shadow DOM);
     het kind moet zelf scrollen. Vaste hoogte + scrollende main. */
  height: 100vh;
}
.app-main {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
</style>
