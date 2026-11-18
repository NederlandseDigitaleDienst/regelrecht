<script setup lang="ts">
import { computed, inject } from 'vue';
import { ENRICH_PROVIDER_OPTIONS } from '../lib/enrichProviders';

// De lege staat van de Machine- en YAML-panes, op één plek.
//
// Die twee panes tonen dezelfde gegevens (machine_readable, de een als
// formulier en de ander als YAML) en stonden daarom met dezelfde tekst en
// knoppen op drie plekken: MachineReadable, YamlView en de inline YAML-pane in
// EditorView. Ze liepen uit elkaar zodra er iets bijkwam. Eén component houdt
// ze gelijk.
//
// Drie toestanden, in volgorde van "wat vraagt nu je aandacht":
//   1. reviewReady - er ligt een voorstel klaar om te beoordelen
//   2. enriching   - er loopt een verrijking
//   3. leeg        - niets, dus bied de twee manieren aan om te beginnen
const props = withDefaults(defineProps<{
  /** Er staat een openstaande review-taak klaar voor deze wet. */
  reviewReady?: boolean;
  /** Het artikel waar die taak over gaat. Leeg bij een voorstel voor de hele
   *  wet. Staat in de tekst omdat de knop naar dat artikel navigeert, en dat
   *  hoeft niet het artikel te zijn dat je nu open hebt. */
  reviewArticle?: string;
  /** Er loopt een verrijking voor deze wet (pending of processing). */
  enriching?: boolean;
  /** Verrijken is hier aan te vragen (of routeert naar login/traject). */
  canEnrich?: boolean;
  /** De machine-versie kan hier ter plekke worden aangemaakt (editor). */
  canWriteHere?: boolean;
  /** Read-only context: bied een route naar de editor aan. */
  canCreate?: boolean;
  /** Doel van die route; leeg laten zolang de gebruiker niet is ingelogd, dan
   *  gaat de klik door de login-popover in plaats van door de href. */
  createHref?: string;
  /** Melding van de laatste mislukte verrijk-aanvraag. */
  enrichError?: string;
  /** Wat er nog ontbreekt voordat verrijken hier kan: '' (niets, het gebeurt
   *  hier), 'login' of 'traject'. Stuurt de uitleg boven de knoppen en de vorm
   *  van het knoplabel. */
  needs?: string;
}>(), {
  reviewReady: false,
  reviewArticle: '',
  enriching: false,
  canEnrich: false,
  canWriteHere: false,
  canCreate: false,
  createHref: undefined,
  enrichError: '',
  needs: '',
});

const emit = defineEmits<{
  /** `provider` is set only when the user picked one from the menu; without
   *  it the deployment's default runs. */
  enrich: [button: EventTarget | null, provider?: string];
  /** Machine-versie hier ter plekke aanmaken. */
  write: [];
  /** Naar de editor om hem daar aan te maken. Payload is het knopelement, zodat
   *  de ouder de login-popover eraan kan ankeren. */
  create: [button: EventTarget | null];
  'view-tasks': [];
  review: [];
}>();

const onLoginTriggerPointerdown = inject('onLoginTriggerPointerdown', () => {});

// Een voorstel is een wijziging, en wijzigingen leven op de branch van een
// traject. Buiten een traject voert de knop de actie dus niet uit maar brengt
// hij je ergens heen. Dat verschil zit in de tekst en in de vorm van het label:
// gebiedende wijs waar het nu gebeurt, heel werkwoord waar je heen gaat.
//
// Het label noemt de wet en niet het artikel, want dat is wat de actie doet: de
// aanvraag kent alleen een wet en levert een voorstel per gewijzigd artikel op.
// Wie bij artikel 5 op deze knop drukte, kreeg taken terug voor artikel 3, 7 en
// 12 zonder dat de knop dat ergens had gezegd.
const actsHere = computed(() => !props.needs);
const enrichLabel = computed(() => (actsHere.value ? 'Verrijk dit regelwerk' : 'Dit regelwerk verrijken'));
const IN_EEN_TRAJECT = 'Een voorstel komt in een traject te staan, want daar leg je wijzigingen vast.';
const emptyText = computed(() => {
  if (props.needs === 'login') return `${IN_EEN_TRAJECT} Log in en kies een traject om dit artikel daar te openen.`;
  if (props.needs === 'traject') return `${IN_EEN_TRAJECT} Kies een traject om dit artikel daar te openen.`;
  return 'Genereren verrijkt alle artikelen van deze versie en levert een voorstel per artikel op in dit traject.';
});
</script>

<template>
  <nldd-inline-dialog
    v-if="reviewReady"
    data-testid="review-ready"
    icon="ai"
    text="Er ligt een voorstel klaar"
    :supporting-text="reviewArticle
      ? `Beoordeel het voorstel voor artikel ${reviewArticle} en sla het op, of verwerp het.`
      : 'Beoordeel de wijzigingen en sla ze op, of verwerp ze.'"
  >
    <nldd-button
      slot="actions"
      appearance="secondary"
      size="md"
      data-testid="review-btn"
      text="Beoordeel voorstel"
      @click="emit('review')"
    ></nldd-button>
  </nldd-inline-dialog>

  <nldd-inline-dialog
    v-else-if="enriching"
    data-testid="enriching"
    icon="ai"
    text="We genereren een voorstel"
    supporting-text="Er staat een wacht-taak klaar. Zodra het voorstel er is, kun je het beoordelen."
  >
    <nldd-button
      slot="actions"
      appearance="secondary"
      size="md"
      data-testid="view-tasks-btn"
      text="Bekijk taken"
      @click="emit('view-tasks')"
    ></nldd-button>
  </nldd-inline-dialog>

  <nldd-inline-dialog
    v-else
    data-testid="no-machine-readable"
    text="Geen machine-leesbare gegevens voor dit artikel"
    :variant="enrichError ? 'alert' : undefined"
    :supporting-text="enrichError || (canEnrich ? emptyText : undefined)"
  >
    <!-- Where the enrichment runs here, the chevron offers a choice of model
         as an advanced option; the button itself runs the default. Where it
         routes to login or a traject first there is nothing to choose yet. -->
    <nldd-split-button
      v-if="canEnrich && actsHere"
      slot="actions"
      appearance="secondary"
      size="md"
      icon="ai"
      width="full"
      data-testid="enrich-btn"
      :text="enrichLabel"
      @action-click="emit('enrich', $event.currentTarget)"
    >
      <nldd-menu slot="popup">
        <nldd-menu-group text="Verrijk met">
          <nldd-menu-item
            v-for="option in ENRICH_PROVIDER_OPTIONS"
            :key="option.id"
            :text="option.label"
            :details="option.model"
            :data-testid="`enrich-with-${option.id}`"
            @select="emit('enrich', null, option.id)"
          ></nldd-menu-item>
        </nldd-menu-group>
      </nldd-menu>
    </nldd-split-button>
    <nldd-button
      v-else-if="canEnrich"
      slot="actions"
      appearance="secondary"
      size="md"
      start-icon="ai"
      data-testid="enrich-btn"
      :text="enrichLabel"
      @click="emit('enrich', $event.currentTarget)"
      @pointerdown.capture="onLoginTriggerPointerdown"
    ></nldd-button>
    <nldd-button
      v-if="canWriteHere"
      slot="actions"
      appearance="secondary"
      size="md"
      start-icon="write"
      data-testid="init-mr-btn"
      text="Stel handmatig op"
      @click="emit('write')"
    ></nldd-button>
    <nldd-button
      v-else-if="canCreate"
      slot="actions"
      appearance="secondary"
      size="md"
      start-icon="write"
      data-testid="create-mr-btn"
      text="Handmatig opstellen"
      :href="createHref"
      @click.prevent="emit('create', $event.currentTarget)"
      @pointerdown.capture="onLoginTriggerPointerdown"
    ></nldd-button>
  </nldd-inline-dialog>
</template>
