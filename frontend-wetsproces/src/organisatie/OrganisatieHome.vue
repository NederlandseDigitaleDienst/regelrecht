<script setup lang="ts">
// The page of an organisation's front: a tile per process of the organisation.
// A citizen picks what to apply for, and the tile opens that process's
// application form; an employee picks a process, and the tile opens its work
// queue. Either way the front hands the browser over with a signed statement,
// so the process opens without a second login. A process that did not answer
// keeps its tile, marked as such and without a link, so the others stay
// usable.
import { computed, onMounted, ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import { bestemming, openAdres, organisatieApi, tegelNaam } from './api';
import type { Organisatie, Proces } from './api';

const props = defineProps<{ kant: Organisatie['kant'] }>();
const organisatie = ref<Organisatie | null>(null);
const fout = ref('');

onMounted(async () => {
  try {
    organisatie.value = await organisatieApi.overzicht();
  } catch (e) {
    fout.value = foutTekst(e);
  }
});

const kop = computed(() => (props.kant === 'burger' ? 'Wat wilt u aanvragen?' : 'Werkvoorraden'));
const uitleg = computed(() =>
  props.kant === 'burger'
    ? 'Kies wat u wilt aanvragen. U bent al ingelogd en hoeft dat daar niet opnieuw te doen.'
    : 'Kies een regeling om de aanvragen te behandelen. U bent al ingelogd en hoeft dat daar niet opnieuw te doen.',
);
const actie = computed(() => (props.kant === 'burger' ? 'Aanvragen' : 'Naar de werkvoorraad'));
const href = (p: Proces): string | undefined => (p.bereikbaar ? openAdres(p.id, bestemming(props.kant)) : undefined);
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="kop">
    <nldd-top-title-bar slot="header" :text="kop" collapse-anchor="organisatie-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-container gap="16">
        <nldd-identity v-if="organisatie?.logo" :text="organisatie.naam" :avatar-src="organisatie.logo" avatar-alt=""></nldd-identity>
        <nldd-title id="organisatie-titel" size="display-3" heading-level="1" :text="kop" :supporting-text="uitleg"></nldd-title>
        <nldd-activity-indicator v-if="!organisatie && !fout" text="De regelingen worden geladen" show-text></nldd-activity-indicator>
        <nldd-inline-dialog
          v-if="fout"
          icon="warning"
          icon-color="critical"
          text="De regelingen zijn niet te laden"
          :supporting-text="fout"
        ></nldd-inline-dialog>
        <nldd-inline-dialog
          v-else-if="organisatie && !organisatie.processen.length"
          icon="info"
          text="Er is hier nog niets"
          supporting-text="Bij deze organisatie is nog geen regeling aangemeld."
        ></nldd-inline-dialog>
        <nldd-collection v-else-if="organisatie" layout="grid" item-min-width="280px">
          <nldd-card
            v-for="p in organisatie.processen"
            :key="p.id"
            :href="href(p)"
            :accessible-label="p.bereikbaar ? `${tegelNaam(p)}: ${actie}` : `${tegelNaam(p)}: nu niet bereikbaar`"
          >
            <nldd-container padding="20" gap="8">
              <nldd-title size="display-6" :text="tegelNaam(p)" heading-level="2"></nldd-title>
              <nldd-text v-if="p.bereikbaar" color="secondary">{{ actie }}</nldd-text>
              <nldd-tag v-else color="warning" icon="warning" text="Nu niet bereikbaar" size="sm"></nldd-tag>
            </nldd-container>
          </nldd-card>
        </nldd-collection>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
