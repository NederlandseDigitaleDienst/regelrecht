<script setup lang="ts">
// A request of the policy that another party answers, as its row in the
// timeline opens it (`metVerzoeken`): who asked whom and by what article, and
// the answer once it is there. Only what the request adds: its stages stand
// in the timeline, and the application it is about is on screen.
import { useId } from 'vue';
import Gegevens from './Gegevens.vue';
import type { Verzoekregel } from './tijdlijn';
import type { Veldtekst } from './types';

defineProps<{ rij: Verzoekregel | null; teksten?: Record<string, Veldtekst> }>();
const emit = defineEmits<{ sluit: [] }>();
const titelId = `${useId()}-titel`;
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="rij ? true : undefined" @close="emit('sluit')">
      <nldd-page :key="rij?.gram.id" sticky-header>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="rij?.tekst ?? ''"
          dismiss-text="Sluit"
          :collapse-anchor="titelId"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="rij">
          <nldd-container>
            <nldd-title :id="titelId" size="display-2" heading-level="2" :text="rij.tekst"></nldd-title>
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-text>
              <p>
                {{ rij.verzoek.aan ? `Een verzoek van ${rij.verzoek.van} aan ${rij.verzoek.aan}` : `Een verzoek van ${rij.verzoek.van}` }},
                volgens {{ rij.verzoek.artikel }}.{{ rij.verzoek.automatisch ? ' Het systeem legde het automatisch vast.' : '' }}
              </p>
            </nldd-text>
            <template v-if="rij.verzoek.antwoord">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-5" text="Antwoord"></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <Gegevens :waarden="rij.verzoek.antwoord" :teksten="teksten" label="Antwoord op het verzoek" />
            </template>
            <template v-else>
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-banner variant="neutral" text="Er is nog geen antwoord"></nldd-banner>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
