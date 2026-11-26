<script setup lang="ts">
// A record as a message to the citizen (bericht.ts): the one way a decision,
// an acknowledgement or an invitation to complete is shown, in the form of a
// letter of an authority. On top a band with the sender as text (the text
// layer's `briefhoofd.lint`; never a logo: the design system has none for a
// sender other than the Rijksoverheid); under it the return address and the
// address block, and on the right the column of references (date, our
// reference, your letter of, subject, the channel). Then the text, the
// signature, an annex on a page of its own, and the footer. Printed, it is an
// sheet of 210 by 297 mm with "Blad x van y" at the foot of each page.
//
// Built of NLDD components: running text, the decision and the remedy in a
// box, a specification or an annex as a table (BerichtTabel.vue, a block of
// its own that a generic table can replace), the status and the channel as
// tags. A part the law requires that has nothing to say shows as missing,
// with the article. A message not yet laid down (`concept`) carries a stamp.
//
// A message that went out is a fact in the chronicle of the sender, with its
// content frozen (`verzending` in the message spec). Given that fact
// (`verzonden`, or `gram` itself when it is one), this view shows the frozen
// message and does not build it again: a later change of the spec, the text
// layer or the record does not change a message sent.
//
// Two views: `burger`, the message as it reaches the citizen; `medewerker`,
// with above it whether every invariant of the template holds and what does
// not, and whether and when it went. A violation of the message shows in
// both: to the citizen as a notice that the message is not in order. A gap of
// the spec itself is for the employee: it is no fault of this message.
import { computed } from 'vue';
import { berichtsoortVoor, isVerzending, maakBericht, verzondenBericht } from './bericht';
import type { Berichtbronnen, Berichtstatus } from './bericht';
import BerichtTabel from './BerichtTabel.vue';
import { longDate } from './datum';
import type { Berichtenspec, Gram } from './types';

const props = defineProps<
  Berichtbronnen & {
    spec?: Berichtenspec | null;
    weergave?: 'burger' | 'medewerker';
    /** The fact by which the message went out, with its frozen content. */
    verzonden?: Gram | null;
  }
>();

const spec = computed(() => props.spec ?? props.vormen?.berichten ?? null);
/** The fact of sending this view shows, if any: then the message is read from it, not built. */
const vastgelegd = computed(() => {
  if (props.verzonden) return props.verzonden;
  const g = props.gram;
  return isVerzending(g, spec.value) || (typeof g.fields?.inhoud === 'string' && typeof g.fields?.hash === 'string') ? g : null;
});
const bericht = computed(() => {
  if (vastgelegd.value) return verzondenBericht(vastgelegd.value);
  const soort = berichtsoortVoor(props.gram, props.rol, spec.value);
  return soort ? maakBericht(soort, props, spec.value) : null;
});
const medewerker = computed(() => props.weergave === 'medewerker');
const hoofd = computed(() => bericht.value?.onderdelen.filter((o) => o.plek !== 'bijlage') ?? []);
const bijlagen = computed(() => bericht.value?.onderdelen.filter((o) => o.plek === 'bijlage') ?? []);
const kenmerken = computed(() => {
  const b = bericht.value;
  if (!b) return [];
  const r: { titel: string; tekst: string }[] = [
    { titel: 'Datum', tekst: b.verzonden ? longDate(b.verzonden.op) : b.datum },
    { titel: 'Onze referentie', tekst: b.kenmerk },
  ];
  if (b.uwBriefVan) r.push({ titel: 'Uw brief van', tekst: b.uwBriefVan });
  r.push({ titel: 'Betreft', tekst: b.onderwerp.replace(/^Concept: /, '') });
  if (b.kanaal) r.push({ titel: 'Verzonden via', tekst: b.kanaal.naam });
  if (bijlagen.value.length) r.push({ titel: 'Bijlage', tekst: bijlagen.value.map((o) => o.bijlage?.titel ?? o.titel ?? o.id).join(', ') });
  return r;
});

const STATUS: Record<Berichtstatus, { tekst: string; kleur: string }> = {
  concept: { tekst: 'Concept', kleur: 'warning' },
  vastgesteld: { tekst: 'Vastgesteld', kleur: 'accent' },
  bekendgemaakt: { tekst: 'Bekendgemaakt', kleur: 'success' },
};
</script>

<template>
  <nldd-container v-if="bericht" gap="16" class="bericht-scherm">
    <!-- For the employee: whether the message may go out as it stands, and whether it went. -->
    <template v-if="medewerker">
      <nldd-banner
        v-if="bericht.verzonden"
        variant="info"
        size="sm"
        icon="check-circle-filled"
        :text="`Verzonden op ${longDate(bericht.verzonden.op)}${bericht.kanaal ? `, ${bericht.kanaal.naam.charAt(0).toLowerCase()}${bericht.kanaal.naam.slice(1)}` : ''}`"
        :supporting-text="`Dit is het bericht zoals het is vastgelegd en verzonden; het wordt niet opnieuw opgebouwd. Hash ${bericht.verzonden.hash.slice(0, 12)}.${bericht.verzonden.vervangt ? ' Het vervangt een eerder verzonden bericht.' : ''}`"
      ></nldd-banner>
      <nldd-banner
        v-else-if="bericht.status === 'concept'"
        variant="warning"
        size="sm"
        icon="warning"
        text="Concept: nog niet vastgesteld"
        supporting-text="Zo luidt het bericht als u het besluit nu neemt. Er is niets vastgelegd en niets verzonden."
      ></nldd-banner>
      <nldd-banner
        v-if="bericht.schendingen.length"
        variant="critical"
        size="sm"
        icon="warning"
        :text="`${bericht.schendingen.length} ${bericht.schendingen.length === 1 ? 'invariant geschonden' : 'invarianten geschonden'}`"
        supporting-text="Het bericht klopt niet met de gegevens. Herstel dit voordat het de deur uitgaat."
      ></nldd-banner>
      <nldd-banner
        v-else
        variant="success"
        size="sm"
        icon="check-circle-filled"
        text="Alle invarianten gelden"
        supporting-text="Elk deel dat de wet eist is gevuld, elke sleutel heeft een waarde, en de bedragen en het oordeel kloppen met de gegevens."
      ></nldd-banner>
      <nldd-list v-if="bericht.schendingen.length" appearance="box-base" accessible-label="Geschonden invarianten">
        <nldd-list-item v-for="(s, i) in bericht.schendingen" :key="i" size="md">
          <nldd-text-cell :text="s.tekst" :supporting-text="s.soort.replace(/_/g, ' ')"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
    <nldd-banner
      v-else-if="bericht.schendingen.some((s) => s.soort !== 'spec')"
      variant="critical"
      size="sm"
      icon="warning"
      text="Dit bericht is niet in orde"
      supporting-text="Een deel ervan klopt niet met de gegevens van uw dossier. Wij nemen contact met u op."
    ></nldd-banner>

    <nldd-box class="brief" :data-status="bericht.status">
      <div v-if="bericht.status === 'concept' && !bericht.verzonden" class="brief-stempel" aria-hidden="true">Concept</div>
      <!-- The band on top: the sender as text, no logo. -->
      <div class="brief-lint">
        <span class="brief-lint-balk" aria-hidden="true"></span>
        <span class="brief-lint-tekst">{{ bericht.lint ?? bericht.afzender }}</span>
      </div>
      <nldd-container padding="24" gap="24" class="brief-blad">
        <div class="brief-kop">
          <div class="brief-adres">
            <p v-if="bericht.retouradres" class="brief-retour">{{ bericht.retouradres }}</p>
            <p v-for="(r, i) in bericht.geadresseerde" :key="i">{{ r }}</p>
          </div>
          <dl class="brief-kenmerken" aria-label="Kenmerken van het bericht">
            <template v-for="k in kenmerken" :key="k.titel">
              <dt>{{ k.titel }}</dt>
              <dd>{{ k.tekst }}</dd>
            </template>
          </dl>
        </div>
        <nldd-container gap="8" layout="row" class="brief-tags">
          <nldd-tag :text="STATUS[bericht.status].tekst" :color="STATUS[bericht.status].kleur" size="sm"></nldd-tag>
          <nldd-tag
            v-if="bericht.kanaal"
            :text="bericht.kanaal.naam"
            :color="bericht.kanaal.code === 'onbekend' ? 'critical' : 'neutral'"
            size="sm"
            :accessible-label="bericht.kanaal.grond.join(', ') || undefined"
          ></nldd-tag>
        </nldd-container>
        <nldd-title heading-level="4" size="display-5" :text="bericht.onderwerp"></nldd-title>
        <nldd-text v-if="bericht.aanhef"><p>{{ bericht.aanhef }}</p></nldd-text>

        <template v-for="o in hoofd" :key="o.id">
          <nldd-container v-if="o.plek === 'ondertekening'" gap="4">
            <nldd-text v-if="bericht.slot"><p>{{ bericht.slot }}</p></nldd-text>
            <nldd-text v-for="(a, i) in o.alineas" :key="i"><p>{{ a.tekst }}</p></nldd-text>
          </nldd-container>
          <component :is="o.nldd === 'nldd-box' ? 'nldd-box' : 'nldd-container'" v-else :background="o.nldd === 'nldd-box' ? 'tinted' : undefined">
            <nldd-container :padding="o.nldd === 'nldd-box' ? '16' : undefined" gap="8">
              <nldd-title v-if="o.titel" heading-level="5" size="display-6" :text="o.titel" :supporting-text="o.vereist_door"></nldd-title>
              <template v-for="(a, i) in o.alineas" :key="i">
                <nldd-title v-if="a.kop && (i > 0 || a.kop !== o.titel)" heading-level="6" size="display-6" :text="a.kop"></nldd-title>
                <nldd-rich-text><p>{{ a.tekst }}</p><ul v-if="a.punten.length"><li v-for="(p, j) in a.punten" :key="j">{{ p }}</li></ul></nldd-rich-text>
              </template>
              <nldd-list v-if="o.opsomming.length" appearance="box-base" :accessible-label="o.titel || o.id">
                <nldd-list-item v-for="(s, i) in o.opsomming" :key="i" size="sm">
                  <nldd-text-cell :text="s"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-list v-if="o.regels.length" appearance="box-base" :accessible-label="o.titel || o.id">
                <nldd-list-item v-for="r in o.regels" :key="r.naam" size="md">
                  <nldd-text-cell :text="r.label" :supporting-text="medewerker ? r.toelichting : undefined"></nldd-text-cell>
                  <nldd-spacer-cell size="16"></nldd-spacer-cell>
                  <nldd-text-cell :text="r.waarde" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <BerichtTabel v-if="o.bijlage || o.tabel" :tabel="o.tabel" :bijlage="o.bijlage" :label="o.titel || o.id" />
              <nldd-inline-dialog
                v-if="o.ontbreekt"
                icon="warning"
                icon-color="critical"
                text="Dit onderdeel ontbreekt"
                :supporting-text="`De wet eist het: ${o.vereist_door}.`"
              ></nldd-inline-dialog>
            </nldd-container>
          </component>
        </template>

        <!-- An annex: a page of its own when printed. -->
        <nldd-container v-for="o in bijlagen" :key="o.id" gap="8" class="brief-bijlage">
          <nldd-divider></nldd-divider>
          <nldd-title heading-level="5" size="display-6" :text="`Bijlage: ${o.bijlage?.titel ?? o.titel ?? o.id}`" :supporting-text="o.vereist_door"></nldd-title>
          <template v-for="(a, i) in o.alineas" :key="i">
            <nldd-rich-text><p>{{ a.tekst }}</p></nldd-rich-text>
          </template>
          <BerichtTabel v-if="o.bijlage || o.tabel" :tabel="o.tabel" :bijlage="o.bijlage" :label="o.titel || o.id" />
          <nldd-inline-dialog
            v-if="o.ontbreekt"
            icon="warning"
            icon-color="critical"
            text="Dit onderdeel ontbreekt"
            :supporting-text="`De wet eist het: ${o.vereist_door}.`"
          ></nldd-inline-dialog>
        </nldd-container>

        <footer v-if="bericht.voettekst?.length" class="brief-voet">
          <span v-for="(r, i) in bericht.voettekst" :key="i">{{ r }}</span>
        </footer>
      </nldd-container>
    </nldd-box>
  </nldd-container>
</template>

<style scoped>
.brief {
  position: relative;
  overflow: hidden;
}
.brief-lint {
  display: flex;
  align-items: stretch;
  gap: 12px;
  min-height: 56px;
  padding: 0 24px;
  border-bottom: 1px solid var(--primitives-color-neutral-200, currentColor);
}
.brief-lint-balk {
  inline-size: 28px;
  background: var(--primitives-color-accent-700, currentColor);
}
.brief-lint-tekst {
  align-self: center;
  font-weight: 600;
}
.brief-kop {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(12rem, 16rem);
  gap: 24px;
}
.brief-adres p {
  margin: 0;
}
.brief-retour {
  font-size: 0.75em;
  text-decoration: underline;
  margin-block-end: 8px !important;
}
.brief-kenmerken {
  margin: 0;
  font-size: 0.85em;
}
.brief-kenmerken dt {
  font-weight: 600;
}
.brief-kenmerken dd {
  margin: 0 0 8px;
}
.brief-voet {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  font-size: 0.75em;
  border-top: 1px solid var(--primitives-color-neutral-200, currentColor);
  padding-top: 8px;
}
.brief-stempel {
  position: absolute;
  top: 40%;
  left: 50%;
  transform: translate(-50%, -50%) rotate(-24deg);
  font-size: 5rem;
  font-weight: 700;
  letter-spacing: 0.2em;
  text-transform: uppercase;
  border: 6px solid currentColor;
  padding: 0 24px;
  color: var(--components-banner-warning-icon-color, currentColor);
  opacity: 0.25;
  pointer-events: none;
  z-index: 1;
}
@media (max-width: 640px) {
  .brief-kop {
    grid-template-columns: 1fr;
  }
}
@media print {
  .brief-tags {
    display: none;
  }
  .brief-bijlage {
    break-before: page;
  }
  .brief-stempel {
    position: fixed;
    opacity: 0.18;
  }
}
</style>

<style>
/* Printed, a message is a letter on sheets of 210 by 297 mm, each with its number; what
   stands around it on the screen is not printed. */
@page {
  size: 210mm 297mm;
  margin: 20mm 18mm 22mm 22mm;
  @bottom-right {
    content: 'Blad ' counter(page) ' van ' counter(pages);
    font-size: 8pt;
  }
}
@media print {
  body:has(.brief) * {
    visibility: hidden;
  }
  body:has(.brief) .brief,
  body:has(.brief) .brief * {
    visibility: visible;
  }
  body:has(.brief) .brief {
    position: absolute;
    inset: 0 auto auto 0;
    width: 100%;
  }
}
</style>
