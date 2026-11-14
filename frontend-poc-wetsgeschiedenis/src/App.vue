<template>
  <Bekijker />
  <OphaalVenster />
</template>

<script setup>
// De bekijker is de pagina van het sjabloon (gemaakt door de generator in het
// dossier); het venster om een wet op te halen is van deze app.
import Bekijker from './Bekijker.vue';
import OphaalVenster from './OphaalVenster.vue';
</script>

<style>
/* De opmaak van de pagina rekent op de body als flexcontainer: de schil van
   de app telt in de lay-out niet mee. */
#app,
.bekijker {
  display: contents;
}
</style>
