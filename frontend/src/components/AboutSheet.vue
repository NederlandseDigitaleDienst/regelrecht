<script setup lang="ts">
// "Over RegelRecht" — a short about sheet opened from the account menu. Static
// content plus a link to the public site. Exposes show()/hide() so the shell
// can drive it from the menu item.
import { ref } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';

const sheetEl = ref<NLDDSheet | null>(null);

function show(): void {
  sheetEl.value?.show?.();
}
function hide(): void {
  sheetEl.value?.hide?.();
}

defineExpose({ show, hide });
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="right" width="480px" @close="hide">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" text="Over RegelRecht" dismiss-text="Sluit" @dismiss="hide"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-rich-text>
            <p>
              RegelRecht is een verkenning van het Ministerie van Binnenlandse Zaken naar wetgeving als
              uitvoerbare code. Het onderzoekt of verschillende organisaties dezelfde wet ook hetzelfde
              toepassen, en of burgers kunnen volgen hoe overheidsbesluiten tot stand komen, door de kloof
              tussen wettekst en digitale uitvoering te overbruggen.
            </p>
            <p>
              Wetten worden vastgelegd in YAML, met machine-uitvoerbare regels naast de originele tekst. Een
              deterministische regel-engine voert die regels uit, scenario's toetsen ze, en een Git-gebaseerd
              corpus houdt alles versievast. Juristen, beleidsmakers en programmeurs werken in een omgeving
              samen aan het formuleren en testen van regels.
            </p>
            <p>
              Zo kunnen organisaties wetten consistenter toepassen, kan beleid voor invoering worden
              gesimuleerd, en kunnen burgers besluiten zoals toeslagen herleiden tot de onderliggende wet.
              RegelRecht is geen bindend juridisch instrument, maar een hulpmiddel voor controleerbaarheid en
              consistentie.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-link
            size="md"
            href="https://regelrecht.rijks.app/"
            target="_blank"
            rel="noopener noreferrer"
            end-icon="external-link"
            text="regelrecht.rijks.app"
          ></nldd-link>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
