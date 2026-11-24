<script setup lang="ts">
// A request of the policy that another party answers, as its row in the
// timeline opens it (`metVerzoeken`): who asked whom and by what article, and
// the answer once it is there. Only what the request adds: its stages stand
// in the timeline, and the application it is about is on screen.
import Gegevens from './Gegevens.vue';
import type { Verzoekregel } from './tijdlijn';
import type { Veldtekst } from './types';

defineProps<{ rij: Verzoekregel | null; teksten?: Record<string, Veldtekst> }>();
const emit = defineEmits<{ sluit: [] }>();
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="rij ? true : undefined" @close="emit('sluit')">
      <nldd-page>
        <nldd-top-title-bar slot="header" heading-level="2" :text="rij?.tekst ?? ''" dismiss-text="Sluit"></nldd-top-title-bar>
        <nldd-simple-section v-if="rij">
          <nldd-container gap="16">
            <nldd-text>
              <p>
                {{ rij.verzoek.aan ? `Een verzoek van ${rij.verzoek.van} aan ${rij.verzoek.aan}` : `Een verzoek van ${rij.verzoek.van}` }},
                volgens {{ rij.verzoek.artikel }}.{{ rij.verzoek.automatisch ? ' Het systeem legde het automatisch vast.' : '' }}
              </p>
            </nldd-text>
            <template v-if="rij.verzoek.antwoord">
              <nldd-title size="display-6"><h3>Antwoord</h3></nldd-title>
              <nldd-container>
                <Gegevens :waarden="rij.verzoek.antwoord" :teksten="teksten" label="Antwoord op het verzoek" />
              </nldd-container>
            </template>
            <nldd-text v-else color="secondary"><p>Er is nog geen antwoord.</p></nldd-text>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
