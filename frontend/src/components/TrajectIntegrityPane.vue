<script setup lang="ts">
// Structuurrapport van een traject: wat er mis is met de inrichting van het
// traject-corpus, en per bevinding wat je eraan doet.
//
// De aanleiding: de wettenindex leest een wet-id af van de mapnaam, terwijl de
// editor na het laden overgaat op het `$id` uit de YAML. Wijken die af, dan
// mislukt alles wat daarna komt met "niet gevonden" - zonder dat ergens staat
// waarom. Deze pagina maakt dat zichtbaar, met de remedie erbij.
//
// De pane doet zelf de netwerkpoot (zoals TrajectDetailsPane) zodat hij zowel
// als losse pagina als in een paneel te gebruiken is. Laden gebeurt bij mount
// en bij de verversknop - niet periodiek: het rapport verandert alleen als er
// gepusht wordt, en dan weet de gebruiker dat zelf.
import { onMounted, watch } from 'vue';
import { SEVERITY_ICONS, useTrajectIntegrity } from '../composables/useTrajectIntegrity';
import type { IntegrityReport } from '../composables/useTrajectIntegrity';
import { paneChromeVisible } from '../constants';

const props = withDefaults(
  defineProps<{
    /** Traject-ref uit de URL (`{slug}-{8hex}`). */
    trajectRef?: string | null;
  }>(),
  { trajectRef: null },
);

const { report, groups, summary, hasFindings, loading, error, load } = useTrajectIntegrity();

function reload(): void {
  load(props.trajectRef);
}
onMounted(reload);
watch(() => props.trajectRef, reload);

/**
 * Samenvattende regel onder de titel: hoeveel is er nagekeken. Zonder dit
 * leest "Geen problemen gevonden" als "er is niets gecontroleerd".
 */
function scopeSummary(r: IntegrityReport | null): string | null {
  if (!r) return null;
  const laws = `${r.checked_laws} ${r.checked_laws === 1 ? 'versie' : 'versies'}`;
  const scenarios = `${r.checked_scenarios} ${r.checked_scenarios === 1 ? 'scenario' : "scenario's"}`;
  return `${laws} en ${scenarios} nagekeken in de eigen repo van dit traject.`;
}

// `error` holds whatever load() caught; read its message the way the template did.
function errorMessage(e: unknown): string | undefined {
  return (e as { message?: string } | null)?.message;
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title v-if="paneChromeVisible(loading)" id="structuur-controle-titel" size="display-3"><h3>Traject-structuur controleren</h3></nldd-title>
    <nldd-spacer v-if="paneChromeVisible(loading)" size="8"></nldd-spacer>
    <nldd-rich-text v-if="paneChromeVisible(loading)">
      <p>
        Controle op de configuratie van het traject-corpus: mapnamen,
        bestandsnamen, dubbele id's en verwijzingen die nergens uitkomen.
      </p>
    </nldd-rich-text>
    <nldd-spacer v-if="paneChromeVisible(loading)" size="16"></nldd-spacer>
    <nldd-toolbar v-if="paneChromeVisible(loading)" accessible-label="Integriteitsacties">
      <nldd-toolbar-item slot="start">
        <nldd-button
          appearance="secondary"
          size="md"
          start-icon="refresh"
          text="Opnieuw controleren"
          :disabled="loading || undefined"
          @click="reload"
        ></nldd-button>
        <nldd-menu-item
          slot="overflow"
          icon="refresh"
          text="Opnieuw controleren"
          :disabled="loading || undefined"
          @select="reload"
        ></nldd-menu-item>
      </nldd-toolbar-item>
    </nldd-toolbar>
    <nldd-spacer v-if="paneChromeVisible(loading)" size="16"></nldd-spacer>

    <nldd-activity-indicator
      v-if="loading"
      text="Structuur controleren"
      show-text
    ></nldd-activity-indicator>

    <nldd-inline-dialog
      v-else-if="error"
      variant="alert"
      text="Structuurcontrole niet gelukt"
      :supporting-text="errorMessage(error) || 'De gegevens konden niet worden opgehaald.'"
    >
      <nldd-button slot="actions" appearance="primary" text="Probeer opnieuw" @click="reload"></nldd-button>
    </nldd-inline-dialog>

    <template v-else-if="report">
      <!-- Lege staat: bevestigend, en met de omvang van de controle erbij. -->
      <nldd-inline-dialog
        v-if="!hasFindings"
        variant="success"
        icon="certified"
        text="Geen problemen gevonden"
        :supporting-text="scopeSummary(report)"
      ></nldd-inline-dialog>

      <template v-else>
        <nldd-rich-text>
          <p>{{ scopeSummary(report) }} {{ summary }}</p>
        </nldd-rich-text>
        <nldd-spacer size="16"></nldd-spacer>

        <!-- Per wet, zwaarst getroffen eerst; het severity-icoon staat per
             bevinding, want binnen één wet mengen fouten en waarschuwingen. -->
        <template v-for="group in groups" :key="group.key">
          <nldd-title size="display-5"><h4>{{ group.title }} ({{ group.counts }})</h4></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list appearance="box-tinted">
            <nldd-list-item
              v-for="(finding, i) in group.findings"
              :key="`${group.key}-${i}`"
              size="md"
            >
              <nldd-icon-cell size="20" vertical-alignment="top">
                <nldd-icon :icon="SEVERITY_ICONS[finding.severity] ?? 'info'"></nldd-icon>
              </nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell
                vertical-alignment="top"
                :overline="finding.path || finding.law_id || undefined"
                :text="finding.message"
                :supporting-text="finding.remedy"
              ></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-spacer size="24"></nldd-spacer>
        </template>
      </template>
    </template>
  </nldd-simple-section>
</template>
