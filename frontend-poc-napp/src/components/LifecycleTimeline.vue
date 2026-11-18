<script setup>
import { computed } from 'vue';
import { datum, BESLISSING_LABELS } from '../format.js';

const props = defineProps({
  aanvraag: { type: Object, required: true },
  besluit: { type: Object, default: null },
  bezwaar: { type: Object, default: null },
});


// AWB-procedure (RFC-008): AANVRAAG → BEHANDELING → BESLUIT → BEKENDMAKING
// → BEZWAAR; is er een bezwaarschrift, dan loopt de bezwaarprocedure
// (BEZWAARSCHRIFT → HERSTEL → BEHANDELING → BESLISSING) als vervolg in
// dezelfde tijdlijn door.
const stappen = computed(() => {
  const status = props.aanvraag.status;
  const b = props.besluit;
  const bereikt = {
    AANVRAAG: true,
    BEHANDELING: true,
    BESLUIT: status === 'BESLUIT' || status === 'BEZWAAR',
    BEKENDMAKING: status === 'BEZWAAR',
    BEZWAAR: status === 'BEZWAAR',
  };
  const basis = [
    {
      key: 'AANVRAAG',
      titel: 'Aanvraag ingediend',
      detail: `${datum(props.aanvraag.aanvraag_datum)} (Awb 4:1)`,
      bereikt: bereikt.AANVRAAG,
    },
    {
      key: 'BEHANDELING',
      titel: 'In behandeling bij de Napp',
      detail: 'De Napp toetst de aanvraag aan de wet (Awb 3:2)',
      bereikt: bereikt.BEHANDELING,
    },
    {
      key: 'BESLUIT',
      titel: b
        ? b.subsidie_toegekend
          ? 'Besluit: subsidie toegekend'
          : 'Besluit: aanvraag afgewezen'
        : 'Besluit',
      detail: b ? `${datum(b.besluit_datum)} (Awb 1:3)` : 'Volgt na de behandeling',
      bereikt: bereikt.BESLUIT,
    },
    {
      key: 'BEKENDMAKING',
      titel: 'Bekendmaking',
      detail: b?.bekendmaking_datum
        ? `${datum(b.bekendmaking_datum)} (Awb 3:41)`
        : 'Het besluit wordt aan u bekendgemaakt',
      bereikt: bereikt.BEKENDMAKING,
    },
    {
      key: 'BEZWAAR',
      titel: 'Bezwaartermijn',
      detail: b?.bezwaartermijn_einddatum
        ? `Zes weken, tot en met ${datum(b.bezwaartermijn_einddatum)} (Awb 6:7)`
        : 'Zes weken vanaf de dag na bekendmaking (Awb 6:7)',
      bereikt: bereikt.BEZWAAR,
    },
  ];

  const bz = props.bezwaar;
  if (!bz) return basis;

  const beslist = !!bz.beslissing;
  const inBehandeling = bz.status === 'BEHANDELING' || beslist;
  const vervolg = [
    {
      key: 'BEZWAARSCHRIFT',
      titel: 'Bezwaar ingediend',
      detail: `${datum(bz.ontvangen_op)} (Awb 6:4)`,
      bereikt: true,
    },
  ];
  // Herstel alleen tonen als die fase loopt of werkelijk is doorlopen.
  const herstelDoorlopen = bz.toets?.outputs?.herstelgelegenheid_geboden === true;
  if (bz.status === 'HERSTEL' || herstelDoorlopen) {
    vervolg.push({
      key: 'HERSTEL',
      titel: 'Herstel van het bezwaarschrift',
      detail:
        bz.status === 'HERSTEL'
          ? 'Vul de ontbrekende onderdelen aan (Awb 6:6)'
          : 'Het verzuim is hersteld (Awb 6:6)',
      bereikt: true,
    });
  }
  vervolg.push(
    {
      key: 'BEZWAAR_BEHANDELING',
      titel: 'Behandeling van het bezwaar',
      detail:
        bz.gehoord === true
          ? 'U bent gehoord (Awb 7:2)'
          : bz.gehoord === false
            ? 'Van het horen is afgezien (Awb 7:3)'
            : 'U wordt gehoord, tenzij daarvan mag worden afgezien (Awb 7:2/7:3)',
      bereikt: inBehandeling,
    },
    {
      key: 'BESLISSING',
      titel: beslist
        ? `Beslissing op bezwaar: ${(BESLISSING_LABELS[bz.beslissing] ?? bz.beslissing).toLowerCase()}`
        : 'Beslissing op bezwaar',
      detail: beslist
        ? `${datum(bz.beslissing_datum)} (Awb 7:11)`
        : bz.beslistermijn_einddatum
          ? `Uiterlijk ${datum(bz.beslistermijn_einddatum)} (Awb 7:10)`
          : 'Na de behandeling (Awb 7:10)',
      bereikt: beslist,
    },
  );
  return [...basis, ...vervolg];
});
</script>

<template>
  <nldd-list appearance="simple" dividers="never">
    <nldd-list-item v-for="(stap, i) in stappen" :key="stap.key" size="md">
      <nldd-step-cell
        :status="stap.bereikt ? 'past' : 'future'"
        :position="stappen.length === 1 ? 'only' : i === 0 ? 'first' : i === stappen.length - 1 ? 'last' : 'between'"
      ></nldd-step-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-text-cell
        :text="stap.titel"
        :supporting-text="stap.detail"
        :color="stap.bereikt ? 'content' : 'secondary'"
      ></nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
</template>
