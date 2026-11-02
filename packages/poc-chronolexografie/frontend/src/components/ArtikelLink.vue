<script setup>
// A link to the article a chronolexogram rests on (its `basis`): for an
// application the article that establishes it, for a decision the article
// that took it. It reads as the chain names an article (`artikelNaam`), and a
// click opens the article in the sheet of the chain (HerkomstSheet.vue), in
// the version valid on `datum`.
import { computed } from 'vue';
import { herkomstVanBasis, toonHerkomst } from '../herkomst.js';
import { artikelNaam } from '../logboek.js';

const props = defineProps({
  basis: { type: String, required: true },
  // The day of the chronolexogram; without it, the play date.
  datum: { type: String, default: null },
});

const herkomst = computed(() => herkomstVanBasis(props.basis));
</script>

<template>
  <nldd-link
    v-if="herkomst"
    href="#"
    :text="artikelNaam(herkomst.wet, herkomst.artikel)"
    @click.prevent="toonHerkomst({ ...herkomst, datum })"
  ></nldd-link>
</template>
