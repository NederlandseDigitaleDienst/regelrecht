<script setup lang="ts">
import { computed } from 'vue';
import qrcode from 'qrcode-generator';

// Een QR-code als inline SVG. Geen canvas en geen data-URL: als vector schaalt
// hij mee met de lay-out en volgt hij `currentColor`, zodat dezelfde code in
// donkere modus klopt zonder een tweede afbeelding.
//
// Geen design-system-component: het systeem heeft geen QR-component, en een
// QR-code is geen stijlkeuze maar een tekening met een vaste betekenis.

const props = withDefaults(defineProps<{
  /** De tekst die gecodeerd wordt; hier altijd een absolute URL. */
  value: string;
  /** Foutcorrectie. 'M' verdraagt ~15% schade en houdt de code klein genoeg. */
  errorCorrection?: 'L' | 'M' | 'Q' | 'H';
  /** Stille marge in modules. De specificatie schrijft er vier voor. */
  quietZone?: number;
  accessibleLabel?: string;
}>(), {
  errorCorrection: 'M',
  quietZone: 4,
  accessibleLabel: '',
});

// `0` laat de generator zelf het kleinste type kiezen dat de tekst aankan.
const qr = computed(() => {
  const code = qrcode(0, props.errorCorrection);
  code.addData(props.value);
  code.make();
  return code;
});

const size = computed(() => qr.value.getModuleCount() + props.quietZone * 2);

/**
 * Eén `<path>` voor alle donkere modules in plaats van een rechthoek per
 * module: dat scheelt bij 29×29 modules ruim vijfhonderd elementen. Aangrenzende
 * donkere modules in een rij worden tot één horizontale streep samengevoegd,
 * zodat er geen haarlijn tussen twee vakjes valt bij niet-hele pixelgroottes.
 */
const path = computed(() => {
  const code = qr.value;
  const n = code.getModuleCount();
  const offset = props.quietZone;
  const parts: string[] = [];
  for (let row = 0; row < n; row++) {
    let runStart = -1;
    for (let col = 0; col <= n; col++) {
      const dark = col < n && code.isDark(row, col);
      if (dark && runStart < 0) runStart = col;
      if (!dark && runStart >= 0) {
        parts.push(`M${runStart + offset} ${row + offset}h${col - runStart}v1h-${col - runStart}z`);
        runStart = -1;
      }
    }
  }
  return parts.join('');
});
</script>

<template>
  <svg
    class="qr"
    :viewBox="`0 0 ${size} ${size}`"
    :role="accessibleLabel ? 'img' : 'presentation'"
    :aria-label="accessibleLabel || undefined"
    :aria-hidden="accessibleLabel ? undefined : 'true'"
    shape-rendering="crispEdges"
    xmlns="http://www.w3.org/2000/svg"
  >
    <!-- Zwart op wit, en bewust géén design-system-tokens. Dit is geen
         vormgeving maar de code zelf: een scanner heeft maximaal contrast
         nodig, en een themakleur (of een donkere modus) maakt hem onleesbaar.
         Daarom tekent hij ook zijn eigen witte vlak voor de stille zone in
         plaats van de achtergrond van de pagina te erven. -->
    <rect :width="size" :height="size" fill="#fff" />
    <path :d="path" fill="#000" />
  </svg>
</template>

<style scoped>
.qr {
  display: block;
  width: 100%;
  height: auto;
}
</style>
