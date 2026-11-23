<script setup>
// Een blok dat je in- en uitklapt. Eén vorm
// voor secties (groot) en groepen (klein). De kop is één knop met aria-expanded; de inhoud
// blijft bewaard als hij dicht is. NLDD heeft geen accordeon, dus dit is een eigen element
// met NLDD-tekst en -iconen erin.
const open = defineModel('open', { type: Boolean, default: false });
const props = defineProps({
  titel: { type: String, required: true },
  sub: { type: String, default: '' },
  // 'sectie': hoofdblok met kader; 'groep': kleiner blok erbinnen.
  soort: { type: String, default: 'sectie' },
});
const id = `uitklap-${Math.random().toString(36).slice(2, 10)}`;
</script>

<template>
  <section class="uitklap" :class="[props.soort, { open }]">
    <button type="button" class="kop" :aria-expanded="open ? 'true' : 'false'" :aria-controls="id" @click="open = !open">
      <nldd-icon class="pijl" :icon="open ? 'chevron-down' : 'chevron-right'" size="20"></nldd-icon>
      <span class="titels">
        <nldd-text :size="props.soort === 'sectie' ? 'md' : 'sm'"><p><strong>{{ props.titel }}</strong></p></nldd-text>
        <nldd-text v-if="props.sub" size="xs" color="secondary"><p>{{ props.sub }}</p></nldd-text>
      </span>
      <span class="extra"><slot name="kop" /></span>
    </button>
    <div v-show="open" :id="id" class="inhoud">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.uitklap {
  border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 40%, transparent);
  border-radius: 8px;
}
.uitklap.sectie { border-width: 1px; background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 60%, transparent); }
.uitklap.groep { border-color: color-mix(in srgb, var(--primitives-color-neutral-400, #999) 30%, transparent); }
.kop {
  all: unset; box-sizing: border-box; cursor: pointer; width: 100%;
  display: flex; align-items: center; gap: 8px;
  padding: 10px 12px; border-radius: 8px;
}
.groep .kop { padding: 6px 10px; }
.kop:hover { background: color-mix(in srgb, var(--primitives-color-neutral-400, #999) 12%, transparent); }
.kop:focus-visible { outline: 2px solid #154273; outline-offset: 2px; }
.pijl { flex: none; width: 20px; height: 20px; }
.titels { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.extra { flex: none; display: inline-flex; align-items: center; gap: 6px; }
.inhoud { padding: 4px 12px 12px; display: flex; flex-direction: column; gap: 10px; }
.groep .inhoud { padding: 0 10px 8px; gap: 0; }
</style>
