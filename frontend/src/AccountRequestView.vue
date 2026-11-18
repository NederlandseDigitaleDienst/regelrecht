<script setup lang="ts">
import { watchEffect } from 'vue';
import { useRouter } from 'vue-router';
import { SUPPORT_EMAIL } from './constants';

// Account-aanvraagpagina — bereikbaar vanaf de login-warning-popover (de
// "Account aanvragen"-knop). Legt uit wie nu een account kan krijgen en hoe.
// Accounts zijn (ook op termijn) voorbehouden aan de overheid; wie er niet
// onder valt, kan straks de demo bekijken.
//
// Top-level route (geen AppShell-child): geen app-chrome. De pagina draagt z'n
// eigen top-title-bar met terugknop, net als de trajecten- en nieuw-traject-
// pagina's.
const router = useRouter();

const mailtoHref = `mailto:${SUPPORT_EMAIL}?subject=${encodeURIComponent('Accountaanvraag RegelRecht')}`;

watchEffect(() => {
  document.title = 'Account aanvragen · RegelRecht';
});

// Terug naar waar je vandaan kwam (de bibliotheek); bij een directe link valt
// het terug op de bibliotheek-home.
function goBack() {
  if (window.history.length > 1) router.back();
  else router.push({ name: 'home' });
}
</script>

<template>
  <nldd-shell>
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Account aanvragen"
        back-text="Terug"
        collapse-anchor="account-aanvragen-titel"
        @back="goBack"
      ></nldd-top-title-bar>

      <nldd-simple-section width="720px">
        <nldd-title id="account-aanvragen-titel" size="display-2"><h1>Account aanvragen</h1></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>

        <nldd-rich-text>
          <p>Met RegelRecht maak en valideer je wetgeving als uitvoerbare code. Accounts zijn daarom voorbehouden aan de overheid.</p>

          <h2>Voor wie</h2>
          <p>Ben je jurist bij de overheid en spraken we elkaar al eerder over RegelRecht? Dan kun je een account aanvragen. We werken op dit moment met een kleine groep waarmee we RegelRecht samen vormgeven.</p>
          <p>Val je hier nu nog niet onder, of wil je RegelRecht eerst zelf ervaren? Binnenkort kun je vrijblijvend rondkijken in de demo, zonder account. Wil je op de hoogte blijven? <a href="https://regelrecht.rijks.app/aanmelden" target="_blank" rel="noopener">Meld je aan voor updates</a>.</p>

          <h2>Hoe vraag je het aan</h2>
          <p>Stuur een korte e-mail naar <a :href="mailtoHref">{{ SUPPORT_EMAIL }}</a>. Fijn als je er even bij zet waar je ons hebt gezien of wie je hebt gesproken, dan pakken we het snel op.</p>
        </nldd-rich-text>

        <nldd-spacer size="24"></nldd-spacer>
        <nldd-button appearance="primary" text="E-mail RegelRecht" start-icon="mail" :href="mailtoHref"></nldd-button>
      </nldd-simple-section>

      <nldd-page-footer slot="footer"></nldd-page-footer>
    </nldd-page>
  </nldd-shell>
</template>
