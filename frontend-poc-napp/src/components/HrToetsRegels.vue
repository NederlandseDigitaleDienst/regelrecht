<script setup>
/**
 * Result rows of the (mocked) Handelsregister check stored with a claim,
 * judged against Kieswet G 1 by the law engine (`wettelijke_toets` on the
 * stored toets). The legal criteria live in the law YAML; this component
 * only renders the per-eis verdicts. Shared between the aanvrager claim
 * block and the beoordelaar claims section.
 */
import { computed } from 'vue';

const props = defineProps({
  toets: { type: Object, default: () => ({}) },
});

const regels = computed(() => {
  const t = props.toets ?? {};
  // Claims van vóór de wettelijke toets vallen terug op de oude
  // client-side afleiding van dezelfde eisen.
  const wet = t.wettelijke_toets ?? {
    voldoet_eis_inschrijving: !!t.gevonden,
    voldoet_eis_rechtsvorm: t.rechtsvorm === 'Vereniging met volledige rechtsbevoegdheid',
    voldoet_eis_naam: !!t.naam_match,
  };
  return [
    {
      label: 'Inschrijving Handelsregister (Kieswet G 1)',
      waarde: t.gevonden
        ? (t.statutaire_naam ?? 'Gevonden')
        : 'Geen inschrijving gevonden',
      ok: !!wet.voldoet_eis_inschrijving,
    },
    {
      label: 'Rechtsvorm (Kieswet G 1: vereniging met volledige rechtsbevoegdheid)',
      waarde: t.rechtsvorm ?? 'Onbekend',
      ok: !!wet.voldoet_eis_rechtsvorm,
    },
    {
      label: 'SBI-code (politieke organisaties: 94.92)',
      waarde: t.sbi_code
        ? `${t.sbi_code}${t.sbi_omschrijving ? ` · ${t.sbi_omschrijving}` : ''}`
        : 'Onbekend',
      ok: t.sbi_code === '94.92',
    },
    {
      label: 'Statutaire naam en aanduiding (Kieswet G 1)',
      waarde: t.naam_match ? 'Komen overeen' : 'Wijken af',
      ok: !!wet.voldoet_eis_naam,
    },
  ];
});

// Het totaaloordeel van Kieswet G 1, alleen aanwezig op claims die door de
// engine zijn getoetst.
const oordeel = computed(() => props.toets?.wettelijke_toets ?? null);
</script>

<template>
  <nldd-list appearance="box-tinted">
    <nldd-list-item v-for="regel in regels" :key="regel.label" size="sm">
      <nldd-text-cell :text="regel.label" :supporting-text="regel.waarde"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-cell width="fit-content">
        <nldd-tag
          :color="regel.ok ? 'success' : 'critical'"
          :text="regel.ok ? 'In orde' : 'Afwijkend'"
        ></nldd-tag>
      </nldd-cell>
    </nldd-list-item>
    <nldd-list-item v-if="oordeel" size="sm">
      <nldd-text-cell
        text="Oordeel volgens de wet (Kieswet G 1)"
        supporting-text="Getoetst door de wet-engine; de bevestiging blijft een beslissing van de Napp."
      ></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-cell width="fit-content">
        <nldd-tag
          :color="oordeel.voldoet_aan_registratie_eisen ? 'success' : 'critical'"
          :text="oordeel.voldoet_aan_registratie_eisen ? 'Voldoet' : 'Voldoet niet'"
        ></nldd-tag>
      </nldd-cell>
    </nldd-list-item>
  </nldd-list>
</template>
