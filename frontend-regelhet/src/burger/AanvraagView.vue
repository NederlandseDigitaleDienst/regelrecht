<script setup lang="ts">
// A new application under the case's own article, or, with `artikel` and
// `op` in the address, one the case offers about a grant
// (`na_toekenning.aanvragen`), such as a duplicate of a card that is lost;
// that one refers to the record that granted. The fields come from the
// law (`GET /api/formulier`); the UI spec says in what order and under which
// headings they stand, how they are named, and where each value comes from.
// What the system fills (the login, the clock, a fixed value, a rule) shows
// read-only, with the value the server will record and its origin; the
// server fills it itself, so the form does not send it. What the UI spec
// hides stays off the form. Once submitted, the sheet says so and offers to
// open the application in the dossier or to start another.
import { hoofdletter, kleineLetter, leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { aanvraagformulier, aanvraagVelden, beginwaarden, herkomsten, schermveld, voorgevuldeWaarden } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram } from '../gedeeld/types';
import { burgerApi } from './api';
import { AANVRAAG, AANVRAAG_WETTEN } from './routes';
import { useBurger } from './useBurger';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ sluit: []; ingediend: [] }>();

const router = useRouter();
const route = useRoute();
/** The application about a grant the address asks for, if the case offers it. */
const overToekenning = computed(() => {
  const artikel = route.query.artikel;
  return typeof artikel === 'string' ? casus.value?.na_toekenning?.aanvragen?.find((a) => a.artikel === artikel) : undefined;
});
const op = computed(() => (typeof route.query.op === 'string' ? route.query.op : undefined));
const artikel = computed(() => overToekenning.value?.artikel ?? casus.value?.artikel);
const { casus, vormen, wie, fout, laden, laad } = useBurger();
const waarden = ref<Record<string, unknown>>({});
/** The prefilled fields the applicant chose to change: those are inputs, holding the prefill to start with. */
const bewerkt = ref<Set<string>>(new Set());
// The sheet has two pages: the form, and behind it where each value comes
// from. The article a question rests on is not what you need while answering
// it, and it is worth little under three inputs of one address; it stands on
// a page of its own, which the introduction links to. Which page shows follows
// from the address, so that link is a link and the browser's back button
// returns from it.
// After submitting, the sheet shows what was submitted until it closes or
// another application begins.
const ingediend = ref<Gram | null>(null);
const pagina = computed(() => (ingediend.value ? 'ingediend' : route.hash === AANVRAAG_WETTEN ? 'herkomst' : 'formulier'));
/** What was submitted, in words: `Aanvraag`. */
const soortIngediend = computed(() => hoofdletter(leesbaar((vorm.value?.soort ?? 'aanvraag').toLowerCase())));
const bezig = ref(false);
const melding = ref('');
// One key per filled-in form: a click sent twice records one application.
let sleutel = nieuweSleutel();

// What this application is about, in a sentence: the thing the case is named
// after, and the cell it is asked from. The title names a thing you apply for
// ("Gehandicaptenparkeerkaart"), so it takes an article here; a case whose
// title does not take "een" needs its own words. An application about a grant
// says it in the case's own words (`overToekenning.uitleg`), so it gets none.
/** Every value the law asks for this application, per article that asks for
 *  it, in the order of the first value of each: what it is called and who
 *  fills it. The ones the system fills are in it too: those are the ones a
 *  reader wonders about most. */
const herkomstPerArtikel = computed(() => {
  const v = vorm.value;
  if (!v) return [];
  const uitSysteem = herkomsten(v);
  const per = new Map<string, { naam: string; tekst: string }[]>();
  for (const veld of v.velden) {
    const scherm = schermveld(veld, vormen.value?.teksten, true);
    const basis = scherm.veld.basis;
    const artikel = basis ? artikelNaam(basis) : 'Zonder artikel';
    const bij = per.get(artikel) ?? [];
    per.set(artikel, bij);
    const wie = uitSysteem[veld.naam];
    bij.push({ naam: veld.naam, tekst: `${scherm.label}: ${wie ? kleineLetter(wie) : 'u vult dit zelf in.'}` });
  }
  return [...per].map(([artikel, gegevens]) => ({ artikel, gegevens }));
});

// The rest of the introduction names the values the law asks, and links them
// to the page that says where each comes from.
const aanhef = computed(() => {
  const c = casus.value;
  if (!c?.titel || overToekenning.value) return '';
  // With the space before the next sentence: a template trims its own.
  return `U vraagt een ${c.titel.toLowerCase()} aan bij ${c.cel_naam}. `;
});
/** What the system fills, asked again on what the form holds now (`POST /api/formulier/systeem`), per field. */
const systeemNu = ref<Record<string, unknown>>({});
const vorm = computed(() => {
  const v = artikel.value && vormen.value ? vormen.value.indieningen[artikel.value] : undefined;
  if (!v || !Object.keys(systeemNu.value).length) return v;
  const systeem = Object.fromEntries(
    Object.entries(v.systeem ?? {}).map(([naam, hoe]) => [naam, naam in systeemNu.value ? { ...hoe, waarde: systeemNu.value[naam] } : hoe]),
  );
  return { ...v, systeem };
});
// A source of what the system fills may read what the applicant fills in, such
// as the year an amount depends on: ask again once the form settles.
let systeemVraag: ReturnType<typeof setTimeout> | undefined;
watch(
  () => JSON.stringify(waarden.value),
  () => {
    clearTimeout(systeemVraag);
    const a = artikel.value;
    const v = artikel.value && vormen.value ? vormen.value.indieningen[artikel.value] : undefined;
    if (!a || !v?.systeem || !Object.values(v.systeem).some((h) => h.bron)) return;
    systeemVraag = setTimeout(async () => {
      try {
        systeemNu.value = await burgerApi.systeemwaarden(a, aanvraagVelden(v, waarden.value, vormen.value?.delen));
      } catch {
        // The form keeps what the server gave when it opened; the submission fills anew.
      }
    }, 300);
  },
);
const formulier = computed(() =>
  vorm.value
    ? aanvraagformulier(
        vorm.value,
        vormen.value?.teksten,
        wie.value?.identiteit ?? null,
        vormen.value?.volgorde,
        vormen.value?.delen,
        vormen.value?.verplicht,
        bewerkt.value,
      )
    : null,
);

/** The fields of the form in their groups. What the system fills shows as a list, which asks nothing. */
const velden = computed(() => (formulier.value?.groepen ?? []).flatMap((g) => g.velden));

/** An empty form, with a key of its own. */
async function begin() {
  ingediend.value = null;
  melding.value = '';
  sleutel = nieuweSleutel();
  await laad();
  bewerkt.value = new Set();
  waarden.value = formulier.value ? { ...beginwaarden(formulier.value.invullen), ...voorgevuldeWaarden(vorm.value) } : {};
}

// The sheet stays in the DOM so it animates; it loads the form when it opens,
// and an application left half-filled does not come back on the next open.
watch(
  () => props.open,
  (open) => open && begin(),
  { immediate: true },
);

// The day of submission is the server's when it records the application, so
// it is named once it is known, not on the form.
const bevestiging = (g: Gram) =>
  `Ingediend${casus.value ? ` bij ${casus.value.cel_naam}` : ''} op ${longDate(g.recorded_on)}. ` +
  `Dat is de dagtekening van uw ${soortIngediend.value.toLowerCase()}. ` +
  'Het besluit verschijnt in uw dossier zodra een medewerker klaar is.';

/** Open what was just submitted in the dossier, which closes the sheet. */
const bekijk = (g: Gram) => router.push({ name: 'stuk', params: { id: g.id } });
/** Another application under the case's own article, on an empty form. */
async function nogEen() {
  if (route.query.artikel || route.query.op) await router.replace({ path: route.path, hash: AANVRAAG });
  await begin();
}

async function indienen() {
  if (!vorm.value || !casus.value) return;
  if (overToekenning.value && !op.value) {
    melding.value = 'Begin bij het besluit in uw dossier: deze aanvraag gaat over een besluit.';
    return;
  }
  bezig.value = true;
  melding.value = '';
  try {
    ingediend.value = await burgerApi.indienen(
      { artikel: artikel.value, ...(overToekenning.value && op.value ? { op: op.value } : {}), velden: aanvraagVelden(vorm.value, waarden.value, vormen.value?.delen) },
      sleutel,
    );
    sleutel = nieuweSleutel();
    // The dossier beside the sheet shows the new record.
    emit('ingediend');
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="open || undefined" @close="emit('sluit')">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="pagina === 'herkomst' ? 'Waar deze gegevens vandaan komen' : 'Nieuwe aanvraag'"
          :back-text="pagina === 'herkomst' ? 'Nieuwe aanvraag' : ''"
          :back-href="pagina === 'herkomst' ? AANVRAAG : ''"
          :collapse-anchor="pagina === 'herkomst' ? 'aanvraag-wetten-titel' : pagina === 'formulier' ? 'aanvraag-titel' : undefined"
          :dismiss-text="pagina === 'ingediend' ? 'Sluit' : 'Annuleer'"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="ingediend">
          <nldd-inline-dialog
            variant="success"
            heading-level="3"
            :text="`${soortIngediend} ingediend`"
            :supporting-text="bevestiging(ingediend)"
          >
            <nldd-button slot="actions" appearance="primary" :text="`Bekijk ${soortIngediend.toLowerCase()}`" @click="bekijk(ingediend)"></nldd-button>
            <nldd-button slot="actions" appearance="secondary" :text="`Doe nog een ${soortIngediend.toLowerCase()}`" @click="nogEen"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="pagina === 'herkomst'">
          <nldd-container>
            <nldd-title id="aanvraag-wetten-titel" size="display-2" heading-level="2" text="Waar deze gegevens vandaan komen"></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-text>
              <p>Elk gegeven komt uit de wet. Hieronder staat per artikel welke gegevens het vraagt, en wie elk gegeven invult.</p>
            </nldd-text>
            <template v-for="a in herkomstPerArtikel" :key="a.artikel">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-6" :text="a.artikel"></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-rich-text>
                <ul>
                  <li v-for="g in a.gegevens" :key="g.naam">{{ g.tekst }}</li>
                </ul>
              </nldd-rich-text>
            </template>
          </nldd-container>
        </nldd-simple-section>
        <nldd-simple-section v-else>
          <nldd-container>
            <nldd-title id="aanvraag-titel" size="display-2"><h2>Nieuwe aanvraag</h2></nldd-title>
            <template v-if="formulier">
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-text>
                <p>
                  {{ aanhef }}Hieronder staan de
                  <nldd-link :href="AANVRAAG_WETTEN" text="gegevens die de wet voor deze beslissing nodig heeft"></nldd-link>.
                  Wat al bekend is, hoeft u niet in te vullen.
                </p>
              </nldd-text>
            </template>
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
            <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het formulier laadt niet" :supporting-text="fout"></nldd-inline-dialog>
            <template v-else-if="formulier">
              <template v-if="overToekenning">
                <nldd-text><p>{{ overToekenning.uitleg }}</p></nldd-text>
                <nldd-spacer size="16"></nldd-spacer>
              </template>
              <template v-if="overToekenning && !op">
                <nldd-inline-dialog
                  icon="warning"
                  icon-color="critical"
                  text="Het is niet duidelijk waar deze aanvraag over gaat"
                  supporting-text="Begin bij het besluit in uw dossier."
                ></nldd-inline-dialog>
                <nldd-spacer size="16"></nldd-spacer>
              </template>
              <template v-if="vorm?.systeem_fout">
                <nldd-inline-dialog
                  icon="warning"
                  icon-color="critical"
                  text="Niet alles wat de gemeente invult, is te tonen"
                  :supporting-text="vorm.systeem_fout"
                ></nldd-inline-dialog>
                <nldd-spacer size="16"></nldd-spacer>
              </template>
              <!-- The browser judges the fields on submit: a field the UI spec
                   requires that is empty stops it, and nldd-form marks it. -->
              <nldd-form>
                <form @submit.prevent="indienen">
                  <SpecFormulier
                    :velden="velden"
                    :waarden="waarden"
                    :artikelen="false"
                    bekend-kop="We gebruiken de volgende gegevens voor uw aanvraag. Deze zijn al bij ons bekend."
                    :tabellen="vormen?.tabellen"
                    @bewerk="(n) => (bewerkt = new Set([...bewerkt, n]))"
                    @zet="(n, v) => (waarden[n] = v)" />
                  <template v-if="melding">
                    <nldd-spacer size="16"></nldd-spacer>
                    <nldd-inline-dialog icon="warning" icon-color="critical" text="De aanvraag is niet ingediend" :supporting-text="melding"></nldd-inline-dialog>
                  </template>
                  <nldd-form-actions>
                    <nldd-button-group orientation="vertical">
                      <nldd-button appearance="primary" type="submit" text="Dien aanvraag in" :loading="bezig || undefined"></nldd-button>
                    </nldd-button-group>
                  </nldd-form-actions>
                </form>
              </nldd-form>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
