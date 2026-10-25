<!-- Gemaakt door werkbank/generatoren/bouw-wetsgeschiedenis.py --poc --vue uit
     wetsgeschiedenis-sjabloon.html in het Wpp-dossier; wijzig het daar. -->
<template>
  <div class="bekijker">
<div class="app">
  <div class="kop">
    <nldd-toolbar size="md" label="Wetsgeschiedenis">
      <nldd-toolbar-title slot="start" id="titel" text="Concept Wetsgeschiedenis"
        supporting-text=""></nldd-toolbar-title>
      <nldd-toolbar-item slot="end" label="Weergave">
        <nldd-segmented-control id="modus" type="radio" variant="text" size="md" value="tekst" accessible-label="Weergave">
          <nldd-segmented-control-item value="tekst" text="Tekst"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="verschil" text="Vergelijk"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="doorwerking" text="Doorwerking" id="modus-doorwerking"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="controle" text="Controle"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="uitleg" text="Uitleg"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="end" label="Ophalen" id="ophalen-plek" class="verborgen">
        <nldd-button id="ophalen" appearance="secondary" size="md" text="Wet ophalen"></nldd-button>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="end" label="Dossier">
        <select id="dossierkeuze" class="dossierkeuze" aria-label="Kamerdossier"></select>
      </nldd-toolbar-item>
    </nldd-toolbar>
  </div>

  <div class="speler">
    <div class="speler-regel">
      <nldd-button-bar size="md">
        <nldd-icon-button id="k-begin" icon="media-backward-end" text="Naar het begin"></nldd-icon-button>
        <nldd-icon-button id="k-stuk-terug" icon="chevron-double-left" text="Vorig stuk"></nldd-icon-button>
        <nldd-icon-button id="k-terug" icon="media-backward-frame" text="Stap terug"></nldd-icon-button>
        <nldd-button-bar-divider></nldd-button-bar-divider>
        <nldd-icon-button id="k-speel" icon="media-play" text="Afspelen"></nldd-icon-button>
        <nldd-button-bar-divider></nldd-button-bar-divider>
        <nldd-icon-button id="k-vooruit" icon="media-forward-frame" text="Stap vooruit"></nldd-icon-button>
        <nldd-icon-button id="k-stuk-vooruit" icon="chevron-double-right" text="Volgend stuk"></nldd-icon-button>
        <nldd-icon-button id="k-eind" icon="media-forward-end" text="Naar de laatste stand"></nldd-icon-button>
      </nldd-button-bar>
      <label class="keuze">Tempo
        <select id="tempo">
          <option value="1800">langzaam</option>
          <option value="800" selected>gewoon</option>
          <option value="220">snel</option>
        </select></label>
      <label class="keuze">Stop bij
        <select id="sprong">
          <option value="alles">elke handeling en elk stuk</option>
          <option value="stukken">alleen de stukken</option>
          <option value="raak">wat niet of met aanname afspeelt</option>
          <option value="focus">wat dit artikel raakt</option>
        </select></label>
      <div class="stapnaam" id="stapnaam"></div>
    </div>
    <div class="kalender" id="kalender" aria-label="De stukken op hun datum"></div>
    <div class="strook" id="strook" role="slider" tabindex="0" aria-label="Moment in de tijdlijn"></div>
    <div class="legenda" id="legenda"></div>
  </div>

  <div class="werk">
    <aside class="zij">
      <div class="blok">
        <h3>Dossier</h3>
        <ul class="stukken" id="stukken"></ul>
        <h3>Artikelen</h3>
        <nldd-search-field id="zoek" size="md" placeholder="Zoek een artikel of woord" accessible-label="Zoek"></nldd-search-field>
        <label class="filter"><input type="checkbox" id="alleen-gewijzigd"> alleen gewijzigde artikelen</label>
        <ul class="artlijst" id="artlijst"></ul>
      </div>
    </aside>
    <main class="document" id="document"></main>
    <aside class="rail" id="rail"></aside>
  </div>
</div>
  </div>
</template>

<script setup>
// De bekijker is de pagina van het sjabloon; pagina.js tekent erin zodra
// deze opmaak in het document staat. Vue raakt haar daarna niet meer aan.
import { onMounted } from 'vue';

onMounted(() => import('./pagina.js'));
</script>
