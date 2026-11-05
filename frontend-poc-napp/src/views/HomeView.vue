<script setup>
import { useRouter } from 'vue-router';
import PortalHeader from '../components/PortalHeader.vue';
import { b } from '../basePad.js';

const router = useRouter();

// Inloggen rechtsboven: het aanvragersportaal regelt de (gesimuleerde)
// eHerkenning-login zelf.
function naarInloggen(item) {
  if (item.key === 'login') window.location.href = b('/aanvrager/');
}

const navItems = [
  { text: 'Home', to: '/' },
  { text: 'Openbaar register', to: '/register' },
];

const stappen = [
  {
    titel: 'U dient een aanvraag in',
    tekst:
      'Log in met eHerkenning namens uw partij en vul uw ledental en transparantieverklaringen in. Uw zetels komen uit de officiële verkiezingsuitslag.',
  },
  {
    titel: 'De Napp beoordeelt uw aanvraag',
    tekst:
      'Wij toetsen uw aanvraag aan de Wet op de politieke partijen en stellen de hoogte van de subsidie vast.',
  },
  {
    titel: 'U ontvangt een besluit',
    tekst:
      'Bij toekenning betalen wij de subsidie uit. Na bekendmaking kunt u zes weken bezwaar maken.',
  },
];
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      portal="publiek"
      :items="navItems"
      :utility-items="[{ text: 'Inloggen', icon: 'person', key: 'login' }]"
      @utility="naarInloggen"
    />

    <nldd-simple-section>
      <nldd-title size="1">
        <span slot="overline">Nederlandse autoriteit politieke partijen</span>
        <h1>Subsidie voor politieke partijen</h1>
      </nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-rich-text>
        <p>
          De Napp verstrekt subsidie aan landelijke en decentrale politieke partijen
          en houdt toezicht op de financiering van partijen. Op grond van de Wet op
          de politieke partijen hebben partijen met vertegenwoordiging in de Eerste
          of Tweede Kamer, gemeenteraden of provinciale staten recht op subsidie.
        </p>
      </nldd-rich-text>
      <nldd-spacer size="20"></nldd-spacer>
      <nldd-button-group orientation="horizontal">
        <nldd-button
          appearance="primary"
          text="Subsidie aanvragen"
          end-icon="arrow-right"
          :href="b('/aanvrager/')"
        ></nldd-button>
        <nldd-button
          appearance="secondary"
          text="Inloggen"
          start-icon="person"
          :href="b('/aanvrager/')"
        ></nldd-button>
      </nldd-button-group>
    </nldd-simple-section>

    <nldd-simple-section background="tinted">
      <nldd-title size="2" slot="header">
        <h2>Hoe werkt het?</h2>
      </nldd-title>
      <nldd-list appearance="simple" dividers="never">
        <nldd-list-item v-for="(stap, i) in stappen" :key="stap.titel" size="md">
          <nldd-step-cell
            status="past"
            :position="stappen.length === 1 ? 'only' : i === 0 ? 'first' : i === stappen.length - 1 ? 'last' : 'between'"
          ></nldd-step-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-text-cell :text="stap.titel" :supporting-text="stap.tekst"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>

    <nldd-simple-section>
      <nldd-title size="2" slot="header">
        <h2>Direct regelen</h2>
      </nldd-title>
      <nldd-collection layout="grid" item-min-width="300px">
        <nldd-card accessible-label="Subsidie aanvragen">
          <nldd-container padding="20">
            <nldd-icon icon="apartment-building" size="32" color="lintblauw"></nldd-icon>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-title size="4"><h3>Voor politieke partijen</h3></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-rich-text>
              <p>
                Vraag subsidie aan, volg de behandeling van uw aanvraag en bekijk
                uw besluiten in het subsidieportaal.
              </p>
            </nldd-rich-text>
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-button
              appearance="primary"
              text="Naar het subsidieportaal"
              end-icon="arrow-right"
              :href="b('/aanvrager/')"
            ></nldd-button>
          </nldd-container>
        </nldd-card>
        <nldd-card accessible-label="Openbaar register">
          <nldd-container padding="20">
            <nldd-icon icon="books-vertical" size="32" color="lintblauw"></nldd-icon>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-title size="4"><h3>Voor iedereen</h3></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-rich-text>
              <p>
                In het openbare register vindt u alle bekendgemaakte
                subsidiebesluiten en statistieken over aanvragen en toekenningen.
              </p>
            </nldd-rich-text>
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-button
              appearance="secondary"
              text="Bekijk het register"
              end-icon="arrow-right"
              @click="router.push('/register')"
            ></nldd-button>
          </nldd-container>
        </nldd-card>
      </nldd-collection>
    </nldd-simple-section>

    <nldd-page-footer slot="footer">
      <nldd-container padding="24" gap="8">
        <nldd-title size="5"><p>Nederlandse autoriteit politieke partijen</p></nldd-title>
        <nldd-rich-text>
          <p>
            De Napp is een onafhankelijke autoriteit, ingesteld bij de Wet op de
            politieke partijen. Zij verstrekt subsidies aan politieke partijen en
            houdt toezicht op de naleving van de financierings- en
            transparantieregels.
          </p>
        </nldd-rich-text>
      </nldd-container>
      <nldd-page-footer-legal-bar slot="legal-bar">
        <nldd-page-footer-legal-bar-item :href="b('/register')" text="Openbaar register"></nldd-page-footer-legal-bar-item>
        <nldd-page-footer-legal-bar-item href="#" text="Contact"></nldd-page-footer-legal-bar-item>
        <nldd-page-footer-legal-bar-item href="#" text="Toegankelijkheid"></nldd-page-footer-legal-bar-item>
        <nldd-page-footer-legal-bar-item href="#" text="Privacy"></nldd-page-footer-legal-bar-item>
      </nldd-page-footer-legal-bar>
    </nldd-page-footer>
  </nldd-page>
</template>
