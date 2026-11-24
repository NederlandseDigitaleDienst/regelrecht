<script setup lang="ts">
// The way in of the citizen's own app: the RegelHets of its list, each with
// whether the app is activated there. Choosing an activated one opens its
// dossier; choosing another asks for a made-up citizen first, and the app
// activates itself at that RegelHet's test Activatiedienst. Activation is
// per RegelHet, so the app has another identity at each, until a shared
// Stelsel exists.
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { BURGER, terugNaar } from '../../gedeeld/terug';
import { alsFout } from '../tauri/kern';
import type { AppStatus } from '../tauri/kern';
import { opslagTekst } from '../tauri/status';
import { burgerapp, kies } from './app';
import type { Regelhet } from './kern';

const route = useRoute();
const router = useRouter();
const regelhets = ref<Regelhet[]>([]);
const standen = ref<Record<string, AppStatus>>({});
/** The RegelHet the citizen is activating the app at, if any. */
const activeren = ref<string | null>(null);
const bezig = ref('');
const fout = ref('');
const laden = ref(true);

const reden: Record<string, string> = {
  credential_expired: 'De activering van deze app is verlopen. Activeer de app opnieuw om verder te gaan.',
  credential_refused: 'De regeling neemt de activering van deze app niet meer aan. Activeer de app opnieuw om verder te gaan.',
};

async function laad() {
  laden.value = true;
  try {
    const app = await burgerapp();
    regelhets.value = app.regelhets;
    const lijst = await Promise.all(app.regelhets.map(async (r) => [r.id, await app.status(r.id)] as const));
    standen.value = Object.fromEntries(lijst);
    fout.value = '';
  } catch (e) {
    fout.value = alsFout(e).message;
  } finally {
    laden.value = false;
  }
}
onMounted(laad);

const ondertitel = (r: Regelhet): string => {
  const h = standen.value[r.id]?.holder;
  if (h) return `Geactiveerd voor ${h.name ?? `BSN ${h.bsn}`}`;
  return standen.value[r.id]?.credential_expired ? 'Activering verlopen' : 'Nog niet geactiveerd';
};

async function open(r: Regelhet) {
  if (!standen.value[r.id]?.holder) {
    activeren.value = r.id;
    return;
  }
  kies(r.id);
  await router.push(terugNaar(route.query.terug, BURGER, '/burger'));
}

async function activeer(bsn: string) {
  const id = activeren.value;
  if (!id) return;
  bezig.value = bsn;
  fout.value = '';
  try {
    const app = await burgerapp();
    standen.value = { ...standen.value, [id]: await app.activeren(id, bsn) };
    kies(id);
    activeren.value = null;
    await router.push(terugNaar(route.query.terug, BURGER, '/burger'));
  } catch (e) {
    fout.value = alsFout(e).message;
  } finally {
    bezig.value = '';
  }
}

async function intrekken(r: Regelhet) {
  try {
    const app = await burgerapp();
    standen.value = { ...standen.value, [r.id]: await app.intrekken(r.id) };
  } catch (e) {
    fout.value = alsFout(e).message;
  }
}

/** Where the key lives, for the RegelHet being activated. */
const opslag = computed(() => {
  const s = activeren.value ? standen.value[activeren.value] : undefined;
  return s ? opslagTekst(s) : '';
});

const gekozenRegelhet = () => regelhets.value.find((r) => r.id === activeren.value);
</script>

<template>
  <nldd-page landmarks="page" sticky-header accessible-label="Regelingen">
    <nldd-top-title-bar slot="header" text="Regelingen" collapse-anchor="keuze-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="keuze-titel" size="display-3"><h3>Regelingen</h3></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-container gap="16">
        <nldd-inline-dialog
          v-if="typeof route.query.reden === 'string' && reden[route.query.reden]"
          icon="warning"
          text="Opnieuw activeren"
          :supporting-text="reden[route.query.reden]"
        ></nldd-inline-dialog>
        <nldd-text>
          <p>
            Deze app is van u en draait alleen in uw browser. Hij praat met elke regeling hieronder via FSC voor
            Burgers. Wat u in een formulier invult, blijft op dit apparaat tot u de aanvraag indient.
          </p>
        </nldd-text>
        <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
        <template v-else-if="!activeren">
          <nldd-list appearance="box-base" accessible-label="Regelingen">
            <nldd-list-item v-for="r in regelhets" :key="r.id" size="md" button @click="open(r)">
              <nldd-text-cell :text="r.naam" :supporting-text="ondertitel(r)"></nldd-text-cell>
              <nldd-cell v-if="standen[r.id]?.holder">
                <nldd-tag text="Geactiveerd" color="success" size="sm"></nldd-tag>
              </nldd-cell>
              <nldd-icon-cell icon="chevron-right" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-button-group orientation="horizontal">
            <template v-for="r in regelhets" :key="r.id">
              <nldd-button
                v-if="standen[r.id]?.holder"
                appearance="critical-transparent"
                size="sm"
                :text="`Activering bij ${r.naam} intrekken`"
                @click="intrekken(r)"
              ></nldd-button>
            </template>
          </nldd-button-group>
        </template>
        <template v-else>
          <nldd-title size="display-5"><h4>App activeren bij {{ gekozenRegelhet()?.naam }}</h4></nldd-title>
          <nldd-banner
            variant="accent"
            text="Testomgeving: kies een verzonnen burger"
            supporting-text="In de testmodus geeft de demo-Activatiedienst van de regeling deze app een bewijs voor een fictief BSN uit de testreeks. Niemand controleert wie u bent. Een echte app krijgt dat bewijs pas nadat u zich bij een Activatiedienst hebt geïdentificeerd."
          ></nldd-banner>
          <nldd-list appearance="box-base" accessible-label="Fictieve burgers">
            <nldd-list-item v-for="bsn in standen[activeren]?.test_bsns ?? []" :key="bsn" button @click="activeer(bsn)">
              <nldd-text-cell :text="`Burger met BSN ${bsn}`" supporting-text="Het BSN is fictief."></nldd-text-cell>
              <nldd-icon-cell icon="chevron-right" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-activity-indicator v-if="bezig"></nldd-activity-indicator>
          <nldd-text v-if="opslag" color="secondary"><p>{{ opslag }}</p></nldd-text>
          <nldd-button-group orientation="horizontal">
            <nldd-button appearance="secondary" text="Terug naar de regelingen" @click="activeren = null"></nldd-button>
          </nldd-button-group>
        </template>
        <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Dit lukt niet" :supporting-text="fout"></nldd-inline-dialog>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
