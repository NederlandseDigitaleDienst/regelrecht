<script setup>
// A picture of YAML files on the explanation page: every file as a box, in a
// column per kind, with the names in it that a line starts or ends at; the
// boxes and lines come from `keten` in schema.js, for one value. With `doel`,
// that name stands out. The lines between those names are drawn over the
// boxes in an SVG, from where the names stand once the page is laid out; they
// are drawn again when the picture changes size.
// Pointing at a name, or focusing it, shows only its own lines.
//
// The lines are a picture: a screen reader gets, per name, the names it
// points to and is pointed to from, in words.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { LIJNSOORTEN, SOORTEN } from './schema.js';

const props = defineProps({
  /** The boxes and lines, from `keten` in schema.js. */
  schema: { type: Object, required: true },
  /** What the picture shows, for a screen reader. */
  label: { type: String, required: true },
});

const s = computed(() => props.schema);
const doel = computed(() => s.value.doel && sleutel(s.value.doel.pad, s.value.doel.begrip));
const kolommen = computed(() =>
  SOORTEN.map((x) => ({ ...x, bestanden: s.value.bestanden.filter((b) => b.soort === x.soort) })).filter(
    (x) => x.bestanden.length,
  ),
);
/** The files with lines between their own names: they get a lane for them. */
const binnen = computed(() => new Set(s.value.lijnen.filter((l) => l.van.pad === l.naar.pad).map((l) => l.van.pad)));
/** The kinds of line in this picture, for the legend. */
const soorten = computed(() => LIJNSOORTEN.filter((l) => s.value.lijnen.some((x) => x.soort === l.soort)));

/** How a name's role reads. */
const ROL = {
  open_term: 'open begrip',
  output: 'uitkomst',
  input: 'invoer',
  answers: 'vraag',
  fact: 'feit',
  decisions: 'besluiten',
  artikel: 'artikel',
  cel: 'cel',
  andere_cellen: 'andere cel',
};
/** The directory a file is in: the name of the regelwerk or the cell. */
const sleutel = (pad, begrip) => `${pad}|${begrip}`;

const titels = computed(() => new Map(s.value.bestanden.map((b) => [b.pad, b.titel])));
const namen = computed(
  () => new Map(s.value.bestanden.flatMap((b) => b.begrippen.map((g) => [sleutel(b.pad, g.key), g.naam]))),
);
/** The lines of a name, in words, for a screen reader. */
function inWoorden(pad, key) {
  const k = sleutel(pad, key);
  const uit = s.value.lijnen
    .filter((l) => sleutel(l.van.pad, l.van.begrip) === k)
    .map((l) => `naar ${namen.value.get(sleutel(l.naar.pad, l.naar.begrip))} in ${titels.value.get(l.naar.pad)}`);
  const in_ = s.value.lijnen
    .filter((l) => sleutel(l.naar.pad, l.naar.begrip) === k)
    .map((l) => `van ${namen.value.get(sleutel(l.van.pad, l.van.begrip))} in ${titels.value.get(l.van.pad)}`);
  return [...uit, ...in_].join('; ');
}

const vlak = ref(null);
const maat = ref({ w: 0, h: 0 });
/** Each line with where it runs, once the page is laid out. */
const paden = ref([]);
const actief = ref(null);

/**
 * A line from (ax, ay), leaving to the side `uit` (1 right, -1 left), to
 * (bx, by), arriving from the side `in_`. It starts and ends with a straight
 * piece of `recht` pixels, so the arrowhead sits on the line however steep
 * the curve between is; `bocht` is how far the curve swings out.
 */
function boog(ax, ay, uit, bx, by, in_, bocht, recht = 10) {
  const a2 = ax + uit * recht;
  const b2 = bx + in_ * recht;
  return `M ${ax} ${ay} L ${a2} ${ay} C ${a2 + uit * bocht} ${ay}, ${b2 + in_ * bocht} ${by}, ${b2} ${by} L ${bx} ${by}`;
}

/** Each line as a curve, from the side of one name to the side of the other. */
function teken() {
  const root = vlak.value;
  if (!root) return;
  const o = root.getBoundingClientRect();
  maat.value = { w: root.scrollWidth, h: root.scrollHeight };
  const plek = (pad, begrip) => {
    const el = root.querySelector(`[data-begrip="${CSS.escape(sleutel(pad, begrip))}"]`);
    return el && Object.assign(el.getBoundingClientRect().toJSON(), { kolom: el.closest('.kolom') });
  };
  paden.value = s.value.lijnen.flatMap((l, i) => {
    const a = plek(l.van.pad, l.van.begrip);
    const b = plek(l.naar.pad, l.naar.begrip);
    if (!a || !b) return [];
    const ay = a.top + a.height / 2 - o.top;
    const by = b.top + b.height / 2 - o.top;
    let d;
    if (l.van.pad === l.naar.pad) {
      // Same file: in the lane on the left inside the box, the longer the
      // further out, so the arcs nest instead of lying on top of each other.
      const x = a.left - o.left - 2;
      d = boog(x, ay, -1, x, by, -1, 4 + Math.min(14, Math.abs(by - ay) / 30), 6);
    } else if (a.kolom === b.kolom) {
      // Same column: out on the right and back in.
      const x = Math.max(a.right, b.right) - o.left;
      d = boog(a.right - o.left, ay, 1, b.right - o.left, by, 1, x - (a.right - o.left) + 16 + Math.min(24, Math.abs(by - ay) / 10));
    } else {
      const rechts = b.left > a.left;
      const ax = (rechts ? a.right : a.left) - o.left;
      const bx = (rechts ? b.left : b.right) - o.left;
      d = boog(ax, ay, rechts ? 1 : -1, bx, by, rechts ? -1 : 1, Math.max(30, Math.abs(bx - ax) / 2 - 10));
    }
    return [{ i, d, soort: l.soort, binnen: l.van.pad === l.naar.pad, van: sleutel(l.van.pad, l.van.begrip), naar: sleutel(l.naar.pad, l.naar.begrip) }];
  });
}

const zichtbaar = (p) => !actief.value || p.van === actief.value || p.naar === actief.value;
/** The lines drawn over the boxes: those of the name pointed at. */
// The lines within a file run inside its box, so they are always on top.
const boven = computed(() => paden.value.filter((p) => p.binnen || (actief.value && zichtbaar(p))));
const verbonden = computed(() => {
  if (!actief.value) return null;
  const set = new Set([actief.value]);
  for (const p of paden.value) if (zichtbaar(p)) set.add(p.van).add(p.naar);
  return set;
});

let waarnemer = null;
onMounted(async () => {
  await nextTick();
  teken();
  waarnemer = new ResizeObserver(() => teken());
  waarnemer.observe(vlak.value);
  // The names change width once the web components and fonts are there.
  document.fonts?.ready.then(teken);
});
onBeforeUnmount(() => waarnemer?.disconnect());
watch(s, async () => {
  await nextTick();
  teken();
});
</script>

<template>
  <nldd-container gap="12">
    <!-- The legend: what each kind of line means, with its YAML key. -->
    <ul class="legenda">
      <li v-for="l in soorten" :key="l.soort">
        <svg width="40" height="10" aria-hidden="true"><line x1="0" y1="5" x2="40" y2="5" :class="['lijn', l.soort]" /></svg>
        <nldd-text size="sm">{{ l.label }} <code>{{ l.yaml }}</code></nldd-text>
      </li>
    </ul>
    <!-- The overview is wider than a phone; it scrolls on its own, not the page. -->
    <div class="rol" tabindex="0" role="group" :aria-label="label">
      <div ref="vlak" class="vlak">
        <div v-for="k in kolommen" :key="k.soort" class="kolom">
          <nldd-text size="sm" color="secondary">{{ k.label }}</nldd-text>
          <nldd-box v-for="b in k.bestanden" :key="b.pad">
            <nldd-container padding="12" gap="8">
              <nldd-text size="sm" weight="bold">{{ b.titel }}</nldd-text>
              <ul :class="['begrippen', { baan: binnen.has(b.pad) }]">
                <li
                  v-for="g in b.begrippen"
                  :key="g.key"
                  :data-begrip="sleutel(b.pad, g.key)"
                  :class="{ uit: verbonden && !verbonden.has(sleutel(b.pad, g.key)), aan: actief === sleutel(b.pad, g.key), doel: doel === sleutel(b.pad, g.key) }"
                  tabindex="0"
                  @mouseenter="actief = sleutel(b.pad, g.key)"
                  @mouseleave="actief = null"
                  @focus="actief = sleutel(b.pad, g.key)"
                  @blur="actief = null"
                >
                  <nldd-text size="sm" color="secondary">{{ g.artikel && !['artikel', 'decisions'].includes(g.rol) ? `art. ${g.artikel} · ` : '' }}{{ ROL[g.rol] ?? g.rol }}</nldd-text>
                  <code>{{ g.naam }}</code>
                  <nldd-visually-hidden>{{ inWoorden(b.pad, g.key) }}</nldd-visually-hidden>
                </li>
              </ul>
            </nldd-container>
          </nldd-box>
        </div>
        <!-- Twice: every line under the boxes, and the lines of the name
             pointed at over them, so they stay whole. -->
        <svg class="lijnen onder" :width="maat.w" :height="maat.h" aria-hidden="true">
          <defs>
            <marker v-for="l in LIJNSOORTEN" :id="`pijl-onder-${l.soort}`" :key="l.soort" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" :class="['punt', l.soort]" />
            </marker>
          </defs>
          <path v-for="p in paden" :key="p.i" :d="p.d" :class="['lijn', p.soort, { uit: !zichtbaar(p) }]" :marker-end="`url(#pijl-onder-${p.soort})`" />
        </svg>
        <svg class="lijnen boven" :width="maat.w" :height="maat.h" aria-hidden="true">
          <defs>
            <marker v-for="l in LIJNSOORTEN" :id="`pijl-boven-${l.soort}`" :key="l.soort" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" :class="['punt', l.soort]" />
            </marker>
          </defs>
          <path v-for="p in boven" :key="p.i" :d="p.d" :class="['lijn', p.soort, { uit: !zichtbaar(p), dik: actief && zichtbaar(p) }]" :marker-end="`url(#pijl-boven-${p.soort})`" />
        </svg>
      </div>
    </div>
  </nldd-container>
</template>

<style scoped>
/* The design system has no diagram; this is the least that draws one: a
 * grid of columns, lists without bullets, and the lines over them. */
.rol {
  overflow-x: auto;
}
.vlak {
  position: relative;
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: minmax(180px, 1fr);
  column-gap: 56px;
  min-width: 1100px;
  padding: 4px 48px 4px 4px;
}
.kolom {
  display: flex;
  flex-direction: column;
  gap: 24px;
  align-self: start;
}
.begrippen,
.legenda {
  list-style: none;
  margin: 0;
  padding: 0;
}
.begrippen {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.begrippen.baan {
  padding-left: 28px;
}
.begrippen li {
  display: flex;
  flex-direction: column;
  padding: 2px 6px;
  margin: 0 -6px;
  border-radius: 4px;
  cursor: default;
  transition: opacity 0.15s;
}
.begrippen li.aan {
  background: var(--semantics-surfaces-base-background-color);
}
.begrippen li.doel {
  outline: 2px solid var(--semantics-content-accent-color);
}
.begrippen li.uit {
  opacity: 0.35;
}
.legenda {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 24px;
}
.legenda li {
  display: flex;
  align-items: center;
  gap: 8px;
}
code {
  font-size: 0.8125rem;
  overflow-wrap: anywhere;
}
.kolom {
  position: relative;
  z-index: 1;
}
.lijnen {
  position: absolute;
  inset: 0;
  pointer-events: none;
  overflow: visible;
}
.lijnen.boven {
  z-index: 2;
}
.lijn {
  fill: none;
  stroke: var(--kleur);
  stroke-width: 1.5;
  transition: opacity 0.15s;
}
.boven .lijn.dik {
  stroke-width: 2.5;
}
.lijn.uit {
  opacity: 0.15;
}
.punt {
  fill: var(--kleur);
}
.vult_in,
.rekent {
  --kleur: var(--primitives-color-coolgray-700);
}
.vult_in {
  --kleur: var(--primitives-color-paars-600);
}
.leest,
.leest {
  --kleur: var(--primitives-color-lintblauw-600);
}
.vraagt,
.vraagt {
  --kleur: var(--primitives-color-oranje-600);
}
.kroniek,
.kroniek {
  --kleur: var(--primitives-color-donkergroen-600);
}
.casus,
.casus {
  --kleur: var(--primitives-color-coolgray-500);
}
.lijn.vraagt {
  stroke-dasharray: 6 4;
}
.lijn.kroniek {
  stroke-dasharray: 2 3;
}
.lijn.casus {
  stroke-dasharray: 1 4;
}
</style>
