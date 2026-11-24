<script setup lang="ts">
// One submission, for the decider of the cell that holds it: what was
// submitted, and under its title the button "Besluiten". That opens a sheet
// with the findings the deciding article asks of the authority
// (`GET /api/formulier`, `besluiten`), the decision as it would be taken, and
// the button that takes it, after a question whether the decider is sure: a
// decision is not undone. The preview is the server's (`voorvertoning`): it
// runs when the pane opens and again when a finding changes, the chronicles
// change (a fact recorded, an advice received) or the test clock moves. It
// may ask more: a finding a policy leaves to the authority for this case
// (`stil`). The preview and the decision taken show in one view
// (BesluitWeergave.vue), with why and the full chain. The main pane beside
// the work queue (WerkvoorraadView.vue), which hands it what it loaded.
//
// What answers the submission comes from the server (`stand` of the
// chronicle): its decision stands; another decision took it back and holds a
// new one in its place; or a later record revoked it with nothing in its
// place. Whether it waits for a decision, the server's queue says (`open`:
// the work queue of the executor, or what waits for an external party), and
// nothing else: the button and the sheet show only for a submission in it.
// When the queue names a record to decide after (`na`), such as the receipt
// of a judgment that annulled a decision on objection, the decider decides
// again after it; after a withdrawal nothing waits.
//
// With `namens` it is the pane beside the admin panel (BeheerView.vue): the
// submission waits for an external party, such as an appeal for the court,
// is one of what the panel shows of that party (`open`), and the admin
// decides on its behalf through the panel's routes.
//
// What the executor's policy asked of another party for this application,
// such as the examination by the GGD, stands in the timeline of the
// application at the bottom (gedeeld/TijdlijnLijst.vue,
// `GET /api/backoffice/tijdlijn/{id}`). A request made for an earlier
// application of the same person, which this one waits on as well, is not in
// that timeline; the pane shows it on its own (`GET /api/backoffice/verzoeken`),
// from the executor's own records, never from the party's chronicle. A
// record of the executor in the timeline opens in a sheet over the pane
// (StukSheet.vue), not on the Kroniek tab.
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { ontbrekendeBevinding, weigeringInWoorden } from '@regelrecht/frontend-shared/chronolex/weigering';
import { kleineLetter, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { geordend, herkomstenOpStuk, opVolgorde, indieningTitel, isAanvraag, kaalSchermveld, schermveld } from '../gedeeld/formulier';
import type { Schermveld } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { antwoordOp, artikelNaam, celNaam, herroeping as herroepingVan, herroepingTekst, omschrijving, rolVan, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import { voorMedewerker } from '../gedeeld/rollen';
import TabelUitKroniek from '../gedeeld/TabelUitKroniek.vue';
import ElementenPerRol from './ElementenPerRol.vue';
import TijdlijnLijst from '../gedeeld/TijdlijnLijst.vue';
import StukSheet from './StukSheet.vue';
import type { Casus, Gram, Ik, Rol, Tabellen, Tijdlijn, Veld, Voorvertoning, Vormen, Waarom } from '../gedeeld/types';
import { backofficeApi, beheerApi } from './api';
import BesluitWeergave from './BesluitWeergave.vue';
import type { EigenVerzoek, Kroniek, Wachtend } from './api';
import { feitNaam } from './feit';
import { verzoekenBij } from '../gedeeld/verzoek';
import type { VerzoekBij } from '../gedeeld/verzoek';

const props = defineProps<{
  cel: string;
  id: string;
  casus: Casus;
  vormen: Vormen | null;
  wie: Ik | null;
  kronieken: Kroniek[];
  /** Decide on behalf of an external party, on one of its waiting submissions. */
  namens?: boolean;
  /**
   * What waits for a decision, as the server's queue says: the executor's
   * work queue, or what waits for the external party. Undefined until it
   * loaded.
   */
  open?: Wachtend[];
  /** Where the back button leads, in words, such as the list the submission was opened from. */
  terug?: string;
}>();
const emit = defineEmits<{ besloten: [] }>();

const kroniek = computed(() => props.kronieken.find((k) => k.cel === props.cel));
/** The submission as the queue holds it, if it waits for a decision. */
const wachtend = computed(() => props.open?.find((w) => w.stuk.id === props.id));
const gevonden = computed(() => (props.namens ? wachtend.value?.stuk : kroniek.value?.grammen.find((g) => g.id === props.id)) ?? null);
// A submission the panel decided on leaves what waits for the party; the pane
// keeps showing it, with the decision.
const onthouden = ref<Gram | null>(null);
watch(gevonden, (g) => (onthouden.value = g ?? onthouden.value), { immediate: true });
const indiening = computed(() => gevonden.value ?? onthouden.value);
/** Per field a rule derived in the submission, the article of that rule ([`herkomstenOpStuk`]): for the case's application only. */
const herkomstVan = computed(() =>
  indiening.value?.basis === props.casus.artikel ? herkomstenOpStuk(props.vormen?.indieningen[props.casus.artikel]) : {},
);
const mag = computed(() =>
  props.namens ? (props.wie?.partijen_invoeren ?? false) : (props.wie?.cellen.find((c) => c.cel === props.cel)?.beslissen ?? false),
);
const api = props.namens ? beheerApi : backofficeApi;
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
/**
 * A record a field refers to, in words: `besluit (beschikking) van 13 oktober
 * 2026`. Searched in every chronicle the panel has: an appeal sits with the
 * court, the decision it contests with the municipality.
 */
const noemStuk = (id: string) => {
  const k = props.kronieken.find((x) => x.grammen.some((g) => g.id === id));
  const g = k?.grammen.find((x) => x.id === id);
  return g && k ? noem(g, rolVan(g, k.stand)) : undefined;
};
/** A record in words with its day: `ontvangst van het afschrift van de uitspraak van 20 november 2026`. */
const noem = (g: Gram, rol: Rol | undefined) =>
  `${omschrijving(g, props.casus, rol).toLowerCase()} van ${longDate(g.recorded_on)}`;
const titel = computed(() =>
  indiening.value
    ? indieningTitel(
        indiening.value,
        props.casus,
        props.vormen,
        props.kronieken.flatMap((k) => k.grammen),
      )
    : 'Aanvraag',
);

/** The decision form of the article the submission asks a decision of. */
const besluitvorm = computed(() => {
  const g = indiening.value;
  const artikel = g && props.vormen?.indieningen[g.basis]?.besluit;
  return artikel ? props.vormen?.besluiten[artikel] : undefined;
});

// The findings the preview found a policy leaves to the authority.
const stil = ref<Veld[]>([]);
/** The findings the UI spec has the system fill: the form does not ask them, the server fills them on the decision. */
const doorSysteem = computed(() => besluitvorm.value?.systeem ?? {});
/** Those of them the UI spec does not hide, with the value the server would record, as a row under the form. */
const systeemwaarden = computed(() => {
  const verborgen = new Set(besluitvorm.value?.verborgen ?? []);
  return Object.fromEntries(
    Object.entries(doorSysteem.value)
      .filter(([naam]) => !verborgen.has(naam))
      .map(([naam, hoe]) => [naam, hoe.waarde]),
  );
});
const velden = computed<Schermveld[]>(() => {
  const verborgen = new Set(besluitvorm.value?.verborgen ?? []);
  const eigen = (besluitvorm.value?.bevindingen ?? []).filter((v) => !(v.naam in doorSysteem.value) && !verborgen.has(v.naam));
  const extra = stil.value.filter((s) => !eigen.some((e) => e.naam === s.naam));
  return [...eigen, ...extra].map((v) => schermveld(v, props.vormen?.teksten));
});
const bevindingen = ref<Record<string, unknown>>({});
/**
 * The order the findings stand in on the sheet: those there from the start in
 * the order of the UI spec, and each one the preview asks for later under
 * them, so a new question never pushes in between answers already given.
 */
const verschenen = ref<string[]>([]);
let verschenenVoor = '';
watch(
  [velden, () => props.id],
  ([nu, id]) => {
    if (id !== verschenenVoor) [verschenenVoor, verschenen.value] = [id, []];
    const nieuw = nu.filter((v) => !verschenen.value.includes(v.veld.naam));
    if (nieuw.length) verschenen.value = [...verschenen.value, ...opVolgorde(nieuw, props.vormen?.volgorde, (v) => v.veld.naam).map((v) => v.veld.naam)];
  },
  { immediate: true },
);
/**
 * Per article, the findings it leaves to the executor, for the page that says
 * where they come from: the articles in the order of their first finding on
 * the form, and under each its findings in that order. Apart from those, per
 * article the findings a policy may still leave to it, which only a later
 * answer can bring onto the form.
 */
const herkomstPerArtikel = computed(() => {
  const per = new Map<string, { punten: string[]; mogelijk: string[] }>();
  const van = (basis: string | undefined) => {
    const artikel = basis ? artikelNaam(basis) : 'Zonder artikel';
    const bij = per.get(artikel) ?? { punten: [], mogelijk: [] };
    per.set(artikel, bij);
    return bij;
  };
  for (const v of geordend(velden.value, verschenen.value).flatMap((g) => g.velden)) van(v.veld.basis).punten.push(v.label);
  const nogNietGevraagd = (voorvertoning.value?.mogelijk ?? []).filter((m) => !velden.value.some((v) => v.veld.naam === m.naam));
  for (const m of opVolgorde(nogNietGevraagd, props.vormen?.volgorde)) van(m.basis).mogelijk.push(schermveld(m, props.vormen?.teksten).label);
  return [...per].map(([artikel, bij]) => ({ artikel, ...bij }));
});

const voorvertoning = ref<Voorvertoning | null>(null);
const genomen = ref<{ besluit: Gram; waarom: Waarom } | null>(null);
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();

/** What answers the submission now, as the server says; for the executor's own submissions. */
const antwoord = computed(() => (!props.namens && kroniek.value ? antwoordOp(props.id, kroniek.value.stand) : undefined));
/** The last decision on the submission, taken before this screen opened, whether it still stands or not. */
const laatste = computed(() => {
  const a = antwoord.value;
  if (!a || a.soort === 'open') return undefined;
  return kroniek.value?.grammen.find((g) => g.id === a.besluit);
});
/** What revoked the last decision, in a word and in a sentence. */
const herroeping = computed(() => {
  const k = kroniek.value;
  const h = laatste.value && k ? herroepingVan(laatste.value.id, k.stand, k.grammen) : undefined;
  return h && k ? { woord: h.woord, tekst: herroepingTekst(h, props.casus, k.stand), gram: h.gram } : null;
});
/** The decision another record took in the place of the revoked one, which answers the submission now. */
const inDePlaats = computed(() => (antwoord.value?.soort === 'in_de_plaats' ? antwoord.value.nieuw : null));
/** The record to decide after, when the queue says to decide again. */
const na = computed(() => wachtend.value?.na);
/** Whether the submission waits for a decision: when the server's queue holds it, and only then. */
const wacht = computed(() => !!wachtend.value);
/** A decision already on the submission that stands, taken before this screen opened. */
const eerder = computed(() => (antwoord.value?.soort === 'besloten' ? laatste.value : undefined));
const eerderWaarom = ref<Waarom | null>(null);
const besluit = computed(() => genomen.value?.besluit ?? eerder.value ?? null);
const waarom = computed(() => genomen.value?.waarom ?? eerderWaarom.value);
const stand = (g: Pick<Gram, 'outputs'>) => toegewezen(g, props.casus);

async function laadEerder() {
  if (eerder.value && !genomen.value) eerderWaarom.value = await backofficeApi.waarom(eerder.value.id).catch(() => null);
}
onMounted(laadEerder);

// The requests of the executor's policy, for the executor's own submissions.
const alleVerzoeken = ref<EigenVerzoek[]>([]);
const verzoekenFout = ref('');
async function laadVerzoeken() {
  if (props.namens) return;
  try {
    alleVerzoeken.value = (await backofficeApi.verzoeken()).verzoeken;
    verzoekenFout.value = '';
  } catch (e) {
    verzoekenFout.value = foutTekst(e);
  }
}
onMounted(laadVerzoeken);

/**
 * Whom this submission waits on and why: its open requests, each in words with
 * the day and the article it was asked by. Nothing once it is decided here.
 */
const wachtOp = computed(() => {
  if (!indiening.value || genomen.value) return null;
  const open = verzoekenBij(indiening.value, alleVerzoeken.value).filter((v) => v.open);
  if (!open.length) return null;
  const wie = [...new Set(open.map((v) => v.aan_naam))];
  const waarom = open.map(
    (v) =>
      `${omschrijving(v.verzoek, props.casus, 'vastlegging')} op ${longDate(v.verzoek.recorded_on)}${v.eerder ? ', bij een eerdere aanvraag' : ''}, volgens ${artikelNaam(v.verzoek.basis)}.`,
  );
  return { tekst: `Wacht op ${wie.join(' en ')}`, waarom: waarom.join(' ') };
});

/** The requests made for an earlier submission that this one waits on as well ([`verzoekenBij`]); the timeline holds the rest. */
const eerdereVerzoeken = computed(() => (indiening.value ? verzoekenBij(indiening.value, alleVerzoeken.value).filter((v) => v.eerder) : []));

/** What became of a request, in words. */
function standVan(v: VerzoekBij): string {
  const gevraagd = `Aan ${v.aan_naam}, op ${longDate(v.verzoek.recorded_on)}`;
  const antwoord = v.antwoord_ontvangen;
  if (!antwoord) return `${gevraagd}. Het antwoord is nog niet binnen.`;
  return `${gevraagd}. ${omschrijving(antwoord, props.casus, 'vastlegging')} op ${longDate(antwoord.recorded_on)}.`;
}

// The preview follows what it depends on. Only the answer to the last
// request counts: an earlier one that arrives late is dropped.
const voorvertoningFout = ref('');
const rekent = ref(false);
let vraag = 0;
async function voorvertonen() {
  const deze = ++vraag;
  rekent.value = true;
  try {
    const v = await api.voorvertoning({ op: props.id, na: na.value, bevindingen: zonderLeeg(bevindingen.value) });
    if (deze !== vraag) return;
    voorvertoning.value = v;
    voorvertoningFout.value = '';
    for (const s of v.stil) if (!stil.value.some((x) => x.naam === s.naam)) stil.value.push(s);
  } catch (e) {
    if (deze === vraag) voorvertoningFout.value = foutTekst(e);
  } finally {
    if (deze === vraag) rekent.value = false;
  }
}
/**
 * Why no decision can be computed yet, in Dutch: what the decision misses,
 * by the label of the UI spec; for a refusal it has no words for, the cell's
 * own message.
 */
const weigering = computed(() => {
  const fout = voorvertoning.value?.geweigerd;
  if (!fout) return '';
  const woorden = {
    veld: (naam: string) => kaalSchermveld(naam, props.vormen?.teksten).label,
    rol: props.casus.rol ?? 'het bestuursorgaan',
  };
  return weigeringInWoorden(fout, woorden) ?? `Het besluit is nog niet te berekenen. De melding van het systeem, in het Engels: ${fout}`;
});
/** Whether this pane may decide now, and so shows the preview. */
const kanBeslissen = computed(() => !!indiening.value && !genomen.value && !eerder.value && wacht.value && mag.value);
let wachttijd: ReturnType<typeof setTimeout> | undefined;
watch(
  () => [
    kanBeslissen.value,
    props.id,
    na.value,
    JSON.stringify(bevindingen.value),
    props.kronieken.map((k) => k.kop).join(','),
    props.open?.length,
    props.casus.vandaag,
  ],
  () => {
    clearTimeout(wachttijd);
    if (!kanBeslissen.value) return;
    // What shows is stale from now until the new answer is in.
    rekent.value = true;
    // A finding typed in: ask once it settles.
    wachttijd = setTimeout(voorvertonen, voorvertoning.value ? 300 : 0);
  },
  { immediate: true },
);
onUnmounted(() => clearTimeout(wachttijd));

// The tables of the UI spec beside the submission (`table`), such as the
// numbers it holds next to the records of another cell they come from; for
// every employee, the reader included. Again when a chronicle changes.
const tabellen = ref<Tabellen | null>(null);
const tabellenFout = ref('');
async function laadTabellen() {
  if (props.namens || !props.vormen?.tabellen?.length) return;
  try {
    tabellen.value = await backofficeApi.tabellen(props.id);
    tabellenFout.value = '';
  } catch (e) {
    tabellenFout.value = foutTekst(e);
  }
}
watch(() => [props.id, props.vormen?.tabellen?.length, props.kronieken.map((k) => k.kop).join(',')], laadTabellen, { immediate: true });

/** The tables for this employee's role: a table without a role is for every employee. */
const mijnTabellen = computed(() =>
  (tabellen.value?.tabellen ?? []).filter((t) => !t.tabel.rol || voorMedewerker(t.tabel.rol, props.wie?.niveau, t.tabel.rol_generiek)),
);
/** Whether an element of the UI spec is for this employee's role, so its block shows. */
const mijnElementen = computed(() => (props.vormen?.elementen ?? []).some((e) => voorMedewerker(e.rol, props.wie?.niveau, e.rol_generiek)));
/** What is known of the submission for the elements of the UI spec: its fields and what its decision gives. */
const bekend = computed(() => ({
  ...(indiening.value?.fields ?? {}),
  ...(voorvertoning.value?.voorlopig?.besluit.outputs ?? {}),
  ...(besluit.value?.outputs ?? {}),
}));

// The timeline of the application, for the executor's own submissions:
// again when a chronicle changes.
const tijdlijn = ref<Tijdlijn | null>(null);
const tijdlijnFout = ref('');
let tijdlijnVraag = 0;
async function laadTijdlijn() {
  if (props.namens) return;
  const deze = ++tijdlijnVraag;
  try {
    const t = await backofficeApi.tijdlijn(props.id);
    if (deze !== tijdlijnVraag) return;
    tijdlijn.value = t;
    tijdlijnFout.value = '';
  } catch (e) {
    if (deze === tijdlijnVraag) tijdlijnFout.value = foutTekst(e);
  }
}
watch(() => props.kronieken.map((k) => k.kop).join(','), laadTijdlijn, { immediate: true });
/** The record of the timeline open in a sheet over this panel, looked up in the chronicles as they now stand. */
const stukId = ref<string | null>(null);
const stukKroniek = computed(() => props.kronieken.find((k) => k.cel === props.casus.cel));
const stuk = computed(() => stukKroniek.value?.grammen.find((g) => g.id === stukId.value) ?? null);
/** A record of the executor opens in that sheet; a record of another cell has no screen on this side. */
function opent(g: Gram) {
  if (g.recording_actor !== props.casus.cel || !stukKroniek.value?.grammen.some((x) => x.id === g.id)) return undefined;
  return () => (stukId.value = g.id);
}

/** The submission decided on, with the day it came in to tell it from another of the same person. */
const besluitOver = computed(() => (indiening.value ? `${titel.value} · ${longDate(indiening.value.recorded_on)}` : titel.value));

/**
 * The addresses of the sheet in which the decider decides: a hash on the
 * address of the submission, as the citizen's new application has one, so a
 * link opens it and the back button closes it. It has two pages: the findings
 * first, then the summary with the decision as it would be taken and the
 * button that takes it. Beside the findings, as on the citizen's new
 * application, a page says which article leaves each to the executor.
 */
const BESLUITEN = '#besluiten';
const SAMENVATTING = '#besluiten-samenvatting';
const HERKOMST = '#besluiten-herkomst';
const route = useRoute();
const router = useRouter();
/** Whether the sheet in which the decider decides is open. */
const besluiten = computed(() => [BESLUITEN, SAMENVATTING, HERKOMST].includes(route.hash));
/**
 * The page of that sheet: the findings, where they come from, or the summary.
 * A decision that asks no findings has only the summary.
 */
const pagina = computed(() =>
  route.hash === SAMENVATTING || !velden.value.length ? 'samenvatting' : route.hash === HERKOMST ? 'herkomst' : 'bevindingen',
);
const paginaTitel = computed(() =>
  pagina.value === 'samenvatting' ? `Samenvatting: ${kleineLetter(besluitOver.value)}` : pagina.value === 'herkomst' ? 'Waar deze punten vandaan komen' : `Besluiten: ${kleineLetter(besluitOver.value)}`,
);
/** The finding on the sheet the preview still needs a judgement on, if the refusal names one. */
const ontbreekt = computed(() => {
  const naam = ontbrekendeBevinding(voorvertoning.value?.geweigerd);
  return naam && velden.value.some((v) => v.veld.naam === naam) ? naam : '';
});
/** Whether the decider tried to go on without it; from then on that question says what it lacks. */
const verderGeprobeerd = ref(false);
watch([besluiten, () => props.id], () => (verderGeprobeerd.value = false));
const fouten = computed(() =>
  verderGeprobeerd.value && ontbreekt.value ? { [ontbreekt.value]: 'Een oordeel: het besluit kan niet zonder deze bevinding.' } : {},
);
const bevindingenEl = ref<HTMLElement>();
async function verder() {
  if (!ontbreekt.value) return naarSamenvatting();
  verderGeprobeerd.value = true;
  await nextTick();
  bevindingenEl.value?.querySelector('[invalid]')?.closest('nldd-form-field')?.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
}
/** Whether the question if the decider is sure is open. */
const bevestigen = ref(false);
/** Why the button did not ask yet; it goes once the preview changes or the sheet opens again. */
const nogNiet = ref('');
watch([voorvertoning, voorvertoningFout, besluiten, rekent], () => (nogNiet.value = ''));
/**
 * Whether the decision as previewed grants the application. Undefined when
 * the submission is no application, such as an objection, or when the preview
 * on screen does not say: it failed or computes no decision.
 */
const toewijzing = computed(() => {
  const v = voorvertoning.value?.voorlopig;
  if (!v || voorvertoningFout.value || !indiening.value || !isAanvraag(indiening.value, props.casus)) return undefined;
  return toegewezen(v.besluit, props.casus);
});
/** The button that takes the decision names what it does, as an order: a direct action. A refusal is a destructive one. */
const beslisKnop = computed(() => (toewijzing.value === true ? 'Wijs aanvraag toe' : toewijzing.value === false ? 'Wijs aanvraag af' : 'Neem besluit'));
/** What going on does, above the button: the decision as previewed, with what is known now. */
const gevolg = computed(() => {
  const wat = toewijzing.value === true ? 'wordt de aanvraag toegewezen' : toewijzing.value === false ? 'wordt de aanvraag afgewezen' : 'wordt het besluit genomen';
  return `Als u verdergaat, ${wat} zoals hierboven staat, met de bevindingen en de kroniek van dit moment.`;
});
/** On the findings page, above the way on: how the application would be decided on these findings. */
const uitkomst = computed(() =>
  toewijzing.value === undefined
    ? undefined
    : `Op basis van deze bevindingen wordt de aanvraag ${toewijzing.value ? 'toegewezen' : 'afgewezen'}.`,
);
const bevestigVraag = computed(() =>
  toewijzing.value === true ? 'Aanvraag toewijzen?' : toewijzing.value === false ? 'Aanvraag afwijzen?' : 'Besluit nemen?',
);

/**
 * Ask whether the decider is sure, since a decision is not undone. While the
 * preview is being updated, say so instead: the button names the outcome of
 * the preview, and a stale one may name the wrong one.
 */
function bevestig() {
  melding.value = '';
  if (rekent.value) nogNiet.value = 'De voorvertoning wordt nog bijgewerkt. Kijk of de uitkomst klopt en probeer het dan opnieuw.';
  else bevestigen.value = true;
}

function openBesluiten() {
  melding.value = '';
  if (!besluiten.value) void router.push({ path: route.path, query: route.query, hash: BESLUITEN });
}
function naarSamenvatting() {
  void router.push({ path: route.path, query: route.query, hash: SAMENVATTING });
}
function sluitBesluiten() {
  if (besluiten.value) void router.push({ path: route.path, query: route.query });
}

async function beslissen() {
  bezig.value = true;
  melding.value = '';
  try {
    genomen.value = await api.beslissen({ op: props.id, na: na.value, bevindingen: zonderLeeg(bevindingen.value) }, sleutel);
    sleutel = nieuweSleutel();
    sluitBesluiten();
    emit('besloten');
    await laadVerzoeken();
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
    bevestigen.value = false;
  }
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" :back-text="terug ?? (namens ? 'Beheerpaneel' : 'Aanvragen')" collapse-anchor="indiening-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-inline-dialog
        v-if="!indiening"
        :text="namens ? 'Deze aanvraag wacht niet op een externe partij' : 'Deze aanvraag staat niet in een kroniek die u mag lezen'"
      ></nldd-inline-dialog>
      <nldd-container v-else>
        <template v-if="namens">
          <nldd-banner
            variant="neutral"
            :text="`U voert dit besluit in namens ${naamVanCel(cel)}`"
            supporting-text="De kroniek legt vast dat het via het beheerpaneel is ingevoerd, door uw account."
          ></nldd-banner>
          <nldd-spacer size="24"></nldd-spacer>
        </template>
        <template v-if="wachtOp">
          <nldd-banner variant="neutral" icon="time" :text="wachtOp.tekst" :supporting-text="wachtOp.waarom"></nldd-banner>
          <nldd-spacer size="24"></nldd-spacer>
        </template>
        <nldd-title
          id="indiening-titel"
          size="display-3"
          heading-level="3"
          :text="titel"
          :supporting-text="`Ontvangen ${longDate(indiening.recorded_on)} door ${naamVanCel(indiening.recording_actor)}`"
        ></nldd-title>
        <template v-if="kanBeslissen">
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-button-group orientation="horizontal">
            <nldd-button appearance="primary" text="Besluiten" @click="openBesluiten"></nldd-button>
          </nldd-button-group>
        </template>
        <nldd-spacer size="24"></nldd-spacer>
        <Gegevens
          :waarden="indiening.fields"
          :teksten="vormen?.teksten"
          :volgorde="vormen?.volgorde"
          :noem="noemStuk"
          :herkomst="herkomstVan"
          :bij="[{ naam: 'grondslag', label: 'Op grond van', waarde: artikelNaam(indiening.basis) }]"
          label="Gegevens van de aanvraag"
        />

        <template v-if="tabellenFout">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-inline-dialog icon="warning" icon-color="critical" text="De tabellen laden niet" :supporting-text="tabellenFout"></nldd-inline-dialog>
        </template>
        <template v-for="t in mijnTabellen" :key="t.tabel.naam">
          <nldd-spacer size="24"></nldd-spacer>
          <TabelUitKroniek
            :tabel="t.tabel"
            :rijen="t.rijen"
            :leeg="t.leeg"
            :cel-naam="(cel: string) => tabellen?.cel_namen[cel] ?? naamVanCel(cel)"
            :teksten="vormen?.teksten"
          />
        </template>

        <template v-if="!namens && mijnElementen">
          <nldd-spacer size="24"></nldd-spacer>
          <ElementenPerRol :elementen="vormen?.elementen" :niveau="wie?.niveau" :waarden="bekend" :teksten="vormen?.teksten" />
        </template>

        <template v-if="verzoekenFout">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-inline-dialog icon="warning" icon-color="critical" text="Wat aan anderen is gevraagd, laadt niet" :supporting-text="verzoekenFout"></nldd-inline-dialog>
        </template>
        <template v-if="eerdereVerzoeken.length">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-title
            heading-level="4"
            size="display-5"
            text="Gevraagd bij een eerdere aanvraag"
            supporting-text="Deze aanvraag wacht ook op het antwoord."
          ></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list appearance="box-base" accessible-label="Gevraagd bij een eerdere aanvraag">
            <nldd-list-item v-for="v in eerdereVerzoeken" :key="v.verzoek.id" size="md">
              <nldd-text-cell :text="omschrijving(v.verzoek, casus, 'vastlegging')" :supporting-text="standVan(v)"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag :text="v.open ? 'Loopt' : 'Antwoord binnen'" :color="v.open ? 'warning' : 'success'" size="sm"></nldd-tag>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </template>

        <template v-if="laatste && herroeping && !genomen">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-title heading-level="4" size="display-5" text="Het eerdere besluit">
            <nldd-badge slot="end" :text="herroeping.woord" color="warning" size="sm"></nldd-badge>
          </nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-banner
            variant="warning"
            :text="`Het besluit van ${longDate(laatste.recorded_on)} geldt niet meer`"
            :supporting-text="`${herroeping.tekst}.`"
          ></nldd-banner>
          <nldd-spacer size="16"></nldd-spacer>
          <Gegevens :waarden="laatste.outputs" :teksten="vormen?.teksten" label="Uitkomst van het eerdere besluit" />
        </template>

        <template v-if="inDePlaats && herroeping && !genomen">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-title
            heading-level="4"
            size="display-5"
            text="Het geldende besluit"
            :supporting-text="`Genomen in de plaats van het eerdere besluit. ${herroeping.tekst}.`"
          >
            <nldd-tag
              v-if="stand(inDePlaats) !== undefined"
              slot="end"
              :text="stand(inDePlaats) ? 'Toegewezen' : 'Afgewezen'"
              :color="stand(inDePlaats) ? 'success' : 'critical'"
            ></nldd-tag>
          </nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <Gegevens :waarden="inDePlaats.outputs" :teksten="vormen?.teksten" label="Uitkomst van het geldende besluit" />
        </template>

        <template v-if="besluit">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-banner
            :variant="genomen ? 'success' : 'neutral'"
            :text="genomen ? 'Het besluit is genomen' : `Op deze aanvraag is besloten op ${longDate(besluit.recorded_on)}`"
          ></nldd-banner>
          <nldd-spacer size="24"></nldd-spacer>
          <BesluitWeergave
            :besluit="besluit"
            :waarom="waarom"
            :casus="casus"
            :vormen="vormen"
            :cel-naam="naamVanCel"
            :grammen="kroniek?.grammen"
            @verzonden="emit('besloten')"
          />
        </template>
        <template v-else-if="wacht && !mag">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-inline-dialog icon="info" text="Een beslisser van deze cel neemt het besluit"></nldd-inline-dialog>
        </template>

        <template v-if="!namens">
          <nldd-spacer size="24"></nldd-spacer>
          <TijdlijnLijst
            :tijdlijn="tijdlijn"
            :fout="tijdlijnFout"
            :casus="casus"
            kant="medewerker"
            :teksten="vormen?.teksten"
            :huidig="id"
            :opent="opent"
          />
        </template>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="besluiten && kanBeslissen ? true : undefined" accessible-label="Besluiten" @close="sluitBesluiten">
      <nldd-page :key="pagina" sticky-header>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="paginaTitel"
          :back-text="pagina !== 'bevindingen' && velden.length ? 'Besluiten' : ''"
          :back-href="pagina !== 'bevindingen' && velden.length ? BESLUITEN : ''"
          dismiss-text="Annuleer"
          :collapse-anchor="`${pagina}-titel`"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="kanBeslissen">
          <nldd-container v-if="pagina === 'bevindingen'" ref="bevindingenEl">
            <nldd-title id="bevindingen-titel" size="display-2" heading-level="2" text="Besluiten" :supporting-text="besluitOver"></nldd-title>
            <nldd-spacer size="24"></nldd-spacer>
            <template v-if="na">
              <nldd-banner
                variant="neutral"
                text="Opnieuw beslissen"
                :supporting-text="`Het nieuwe besluit volgt op ${noemStuk(na) ?? 'het stuk dat het eerdere besluit vernietigde'}.`"
              ></nldd-banner>
              <nldd-spacer size="24"></nldd-spacer>
            </template>
            <nldd-text>
              <p>
                Hieronder staan de
                <nldd-link :href="HERKOMST" :text="`punten die de wet aan ${naamVanCel(cel)} overlaat`"></nldd-link>.
              </p>
            </nldd-text>
            <nldd-spacer size="16"></nldd-spacer>
            <SpecFormulier
              :velden="velden"
              :waarden="bevindingen"
              :volgorde="verschenen"
              :artikelen="false"
              :fouten="fouten"
              oordeel
              @zet="(n, v) => (bevindingen[n] = v)"
            />
            <template v-if="Object.keys(systeemwaarden).length">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-6" text="Vult het systeem in" supporting-text="Volgens de UI-beschrijving; het systeem legt ze vast bij het besluit."></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <Gegevens :waarden="systeemwaarden" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" label="Vult het systeem in" />
            </template>
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-banner v-if="voorvertoningFout" variant="critical" text="De voorvertoning lukt niet" :supporting-text="voorvertoningFout"></nldd-banner>
            <nldd-activity-indicator v-else-if="!voorvertoning" text="Het besluit wordt berekend" show-text></nldd-activity-indicator>
            <nldd-banner
              v-else-if="voorvertoning.geweigerd && !ontbreekt"
              variant="warning"
              text="Er is nog geen besluit te berekenen"
              :supporting-text="weigering"
            ></nldd-banner>
            <template v-else-if="voorvertoning.voorlopig || ontbreekt">
              <template v-if="rekent || uitkomst">
                <nldd-text :color="rekent ? 'secondary' : undefined"><p>{{ rekent ? 'De uitkomst wordt bijgewerkt.' : uitkomst }}</p></nldd-text>
                <nldd-spacer size="16"></nldd-spacer>
              </template>
              <nldd-button-group orientation="vertical">
                <nldd-button appearance="primary" text="Verder naar samenvatting" @click="verder"></nldd-button>
              </nldd-button-group>
            </template>
          </nldd-container>

          <nldd-container v-else-if="pagina === 'herkomst'">
            <nldd-title id="herkomst-titel" size="display-2" heading-level="2" text="Waar deze punten vandaan komen"></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-text>
              <p>Elk punt komt uit de wet. Hieronder staat per artikel welke punten het aan {{ naamVanCel(cel) }} overlaat.</p>
            </nldd-text>
            <template v-for="a in herkomstPerArtikel" :key="a.artikel">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-6" :text="a.artikel"></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-rich-text v-if="a.punten.length">
                <ul>
                  <li v-for="p in a.punten" :key="p">{{ p }}</li>
                </ul>
              </nldd-rich-text>
              <template v-if="a.mogelijk.length">
                <nldd-spacer v-if="a.punten.length" size="16"></nldd-spacer>
                <nldd-text><p>Kan nog gevraagd worden, afhankelijk van eerdere antwoorden:</p></nldd-text>
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-rich-text>
                  <ul>
                    <li v-for="p in a.mogelijk" :key="p">{{ p }}</li>
                  </ul>
                </nldd-rich-text>
              </template>
            </template>
          </nldd-container>

          <nldd-container v-else>
            <nldd-title id="samenvatting-titel" size="display-2" heading-level="2" text="Samenvatting" :supporting-text="besluitOver"></nldd-title>
            <nldd-spacer size="24"></nldd-spacer>
            <template v-if="na && !velden.length">
              <nldd-banner
                variant="neutral"
                text="Opnieuw beslissen"
                :supporting-text="`Het nieuwe besluit volgt op ${noemStuk(na) ?? 'het stuk dat het eerdere besluit vernietigde'}.`"
              ></nldd-banner>
              <nldd-spacer size="24"></nldd-spacer>
            </template>
            <template v-if="!velden.length">
              <nldd-text color="secondary"><p>Dit besluit vraagt {{ naamVanCel(cel) }} geen bevindingen.</p></nldd-text>
              <nldd-spacer size="24"></nldd-spacer>
            </template>
            <nldd-banner v-if="voorvertoningFout" variant="critical" text="De voorvertoning lukt niet" :supporting-text="voorvertoningFout"></nldd-banner>
            <nldd-activity-indicator v-else-if="!voorvertoning" text="Het besluit wordt berekend" show-text></nldd-activity-indicator>
            <nldd-banner
              v-else-if="voorvertoning.geweigerd"
              variant="warning"
              text="Er is nog geen besluit te berekenen"
              :supporting-text="weigering"
            ></nldd-banner>
            <BesluitWeergave
              v-else-if="voorvertoning.voorlopig"
              :besluit="voorvertoning.voorlopig.besluit"
              :waarom="voorvertoning.voorlopig.waarom"
              :casus="casus"
              :vormen="vormen"
              :cel-naam="naamVanCel"
              :grammen="kroniek?.grammen"
              :niveau="3"
              voorlopig
            />
            <template v-if="voorvertoning?.voorlopig && voorvertoning.automatisch?.length">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-5" text="Daarna doet het systeem zelf"></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-list appearance="box-base" accessible-label="Wat het systeem daarna zelf vastlegt">
                <nldd-list-item v-for="(a, i) in voorvertoning.automatisch" :key="i" size="md">
                  <nldd-text-cell :text="feitNaam(a.feit)" :supporting-text="`Volgens ${artikelNaam(a.artikel)}`"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
            </template>
            <template v-if="rekent && voorvertoning">
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-text size="sm" color="secondary"><p>De voorvertoning wordt bijgewerkt.</p></nldd-text>
            </template>

            <template v-if="melding">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-banner variant="critical" text="Dat lukt niet" :supporting-text="melding"></nldd-banner>
            </template>
            <template v-if="voorvertoning?.voorlopig && !voorvertoningFout">
              <template v-if="nogNiet && !melding">
                <nldd-spacer size="24"></nldd-spacer>
                <nldd-banner variant="neutral" text="Nog niet" :supporting-text="nogNiet"></nldd-banner>
              </template>
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-text><p>{{ gevolg }}</p></nldd-text>
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-button-group orientation="vertical">
                <nldd-button :appearance="toewijzing === false ? 'destructive' : 'primary'" :text="beslisKnop" @click="bevestig"></nldd-button>
              </nldd-button-group>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

  <Teleport to="body">
    <nldd-modal-dialog
      v-if="kanBeslissen"
      variant="alert"
      :open="bevestigen || undefined"
      :text="bevestigVraag"
      :supporting-text="`Het besluit wordt vastgelegd in de kroniek van ${naamVanCel(cel)}. Dat is niet terug te draaien: alleen een later besluit kan het herroepen.`"
      @close="bevestigen = false"
    >
      <nldd-button slot="actions" appearance="primary" text="Controleer opnieuw" @click="bevestigen = false"></nldd-button>
      <nldd-button
        slot="actions"
        :appearance="toewijzing === false ? 'destructive' : 'secondary'"
        :text="beslisKnop"
        :loading="bezig || undefined"
        @click="beslissen"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
  <StukSheet
    :stuk="stuk"
    :kroniek="stukKroniek"
    :casus="casus"
    :vormen="vormen"
    :huidig="id"
    @sluit="stukId = null"
  />
</template>
