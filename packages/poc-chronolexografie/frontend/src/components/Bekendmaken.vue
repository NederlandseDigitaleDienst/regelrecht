<script setup>
// Bekendmaken: een stap na het besluit. Waar een artikel in de fase die het
// besluit bekendmaakt zegt dat een feit een van zijn gegevens houdt, zoals
// de dag van verzending, vraagt het besluitformulier dat niet. Het
// bestuursorgaan legt dat feit vast nadat het besluit is genomen, met een
// verwijzing naar het besluit (FeitVastleggen.vue). Wat de bekendmaking
// geeft, zoals de bezwaartermijn (Awb 6:8), volgt dan uit de kroniek op de
// speeldatum (`api.bekendmaking`). Bij een besluit waarvan de bekendmaking
// geen feit leest, maakt de cel het bekend op de dag van het besluit; dan is
// er geen stap en toont deze sectie niets.
import { computed, reactive, watch } from 'vue';
import { api } from '../api.js';
import { aanvragen, alsJson, besluitOp, kort, state } from '../state.js';
import FeitVastleggen from './FeitVastleggen.vue';

const vorm = () => state.vormen?.besluit?.bekendmaking ?? null;

const besluiten = computed(() =>
  vorm()
    ? aanvragen()
        .map((a) => besluitOp(a.id))
        .filter(Boolean)
    : [],
);

// Per besluit hoe het op de speeldatum is bekendgemaakt, na elke stap opnieuw.
// Een antwoord op een eerdere vraag, zoals voor een andere speeldatum, telt
// niet meer zodra er een nieuwere is gesteld.
const stand = reactive({});
let vraag = 0;
watch(
  () => [besluiten.value.map((b) => b.id).join(), state.grammen.length, state.datum],
  async () => {
    const deze = ++vraag;
    for (const b of besluiten.value) {
      let antwoord;
      try {
        antwoord = await api.bekendmaking(b.id, state.datum);
      } catch (e) {
        antwoord = { fout: e.message };
      }
      if (deze === vraag) stand[b.id] = antwoord;
    }
  },
  { immediate: true },
);

const status = (id) => {
  const s = stand[id];
  if (!s) return { text: 'Bekendmaking wordt gelezen', variant: 'loading' };
  if (s.fout) return { text: 'De bekendmaking is niet te lezen', supporting: s.fout, icon: 'warning' };
  if (!s.bekendgemaakt)
    return {
      text: 'Nog niet bekendgemaakt',
      supporting:
        'De kroniek houdt op de speeldatum geen vastlegging van de bekendmaking. Wat daaruit volgt, zoals de bezwaartermijn, ligt nog niet vast.',
      icon: 'info',
    };
  return {
    text: 'Bekendgemaakt',
    supporting: `Uit ${s.vastgelegd.length === 1 ? 'de vastlegging' : `${s.vastgelegd.length} vastleggingen`} in de kroniek.`,
    icon: 'check-mark-circle',
    color: 'success',
  };
};
</script>

<template>
  <nldd-simple-section v-if="vorm()">
    <nldd-title heading-level="2" slot="header" size="display-3" text="Bekendmaken">
      <nldd-text slot="supporting-text">
        Een besluit is pas bekendgemaakt als het is verzonden of uitgereikt. Het bestuursorgaan legt dat vast
        nadat het besluit is genomen, met een verwijzing naar het besluit. Wat uit de bekendmaking volgt, zoals de
        bezwaartermijn, leest de cel uit die vastleggingen.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="besluiten.length === 0"
      icon="info"
      text="Nog geen besluit om bekend te maken"
      supporting-text="Een besluit neem je hierboven, op een open aanvraag."
    ></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="b in besluiten" :key="b.id">
        <nldd-container padding="16" gap="12">
          <nldd-title heading-level="3"
            size="display-5"
            :text="`Besluit ${kort(b.id)}`"
            :supporting-text="`genomen ${b.recorded_on}`"
          ></nldd-title>
          <nldd-inline-dialog
            :variant="status(b.id).variant"
            :icon="status(b.id).icon"
            :icon-color="status(b.id).color"
            :text="status(b.id).text"
            :supporting-text="status(b.id).supporting"
          ></nldd-inline-dialog>
          <nldd-code-viewer
            v-if="stand[b.id]?.bekendgemaakt"
            language="json"
            appearance="box-tinted"
          >{{ alsJson(stand[b.id].uitvoer) }}</nldd-code-viewer>
          <FeitVastleggen :cel="state.vormen.besluit.cel" :handelingen="vorm().feiten" :verwijst-naar="b.id" />
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>
</template>
