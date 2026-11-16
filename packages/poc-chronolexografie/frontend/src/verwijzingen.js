/**
 * What a chronolexogram refers to, and what refers to it, in words a visitor
 * can follow. A gram names another by its id: `refers_to` (the application a
 * decision is taken on, the decision an objection contests), `after` (the
 * receipt a decision taken again follows), `received.id` (the gram of another
 * cell a copy is of, RFC-053), and a field that holds an id, such as the
 * contested decision the cell fills in an objection. The Kroniek tab shows
 * every chronicle the app has, so each of those it can find is a link there;
 * one it cannot find says so.
 */
import { nextTick } from 'vue';
import { celNaam } from './logboek.js';
import { hoofdletter, isGramId, kort, leesbaar, state } from './state.js';

/** Every chronicle the app has: that of the cell of the case, then the others (RFC-048). */
const kronieken = () => [{ cel: state.casus?.cel, grammen: state.grammen }, ...(state.andere ?? [])];

/** The chronolexogram `id` and the cell whose chronicle holds it, or null. */
export function zoekGram(id) {
  for (const k of kronieken()) {
    const gram = k.grammen.find((g) => g.id === id);
    if (gram) return { gram, cel: k.cel };
  }
  return null;
}

/** `BESLUIT 476c88d3`: the kind of a chronolexogram and the end of its id. */
export const gramKop = (g) => `${g.kind} ${kort(g.id)}`;

/**
 * Where the id `id` leads, seen from a gram in the chronicle of `vanCel`:
 * `tekst` names it (`BESLUIT 476c88d3 · <fact or basis>`), `gevonden` says
 * whether a chronicle of the app holds it, and `kroniek` names that
 * chronicle when it is another one than the gram's own.
 */
export function doel(id, vanCel) {
  const hit = zoekGram(id);
  if (!hit) return { id, gevonden: false, tekst: `chronolexogram ${kort(id)}`, kroniek: null };
  const { gram, cel } = hit;
  return {
    id,
    gevonden: true,
    tekst: [gramKop(gram), gram.fact ?? gram.basis].filter(Boolean).join(' · '),
    kroniek: cel === vanCel ? null : celNaam(cel),
  };
}

/**
 * The ids gram `g` names, each once, with what the reference is: the first
 * of `refers_to`, `after`, `received` and an id-valued field or output that
 * names it.
 */
function genoemd(g) {
  const uit = [];
  const voeg = (soort, id) => {
    if (id && !uit.some((r) => r.id === id)) uit.push({ soort, id });
  };
  voeg('Verwijst naar', g.refers_to);
  voeg('Genomen na', g.after);
  if (g.received) voeg(`Ontvangen van ${celNaam(g.received.from)}`, g.received.id);
  for (const [veld, w] of [...Object.entries(g.fields ?? {}), ...Object.entries(g.outputs ?? {})]) {
    if (isGramId(w)) voeg(hoofdletter(leesbaar(veld)), w);
  }
  return uit;
}

/** What gram `g` refers to, each with its `doel`. */
export function verwijzingen(g) {
  const cel = zoekGram(g.id)?.cel ?? g.recording_actor;
  return genoemd(g).map((r) => ({ ...r, doel: doel(r.id, cel) }));
}

/** The grams in any chronicle of the app that refer to `g`, each as its `doel`. */
export function verwezenDoor(g) {
  const cel = zoekGram(g.id)?.cel ?? g.recording_actor;
  return kronieken()
    .flatMap((k) => k.grammen)
    .filter((x) => x.id !== g.id && genoemd(x).some((r) => r.id === g.id))
    .map((x) => doel(x.id, cel));
}

/** Show chronolexogram `id` in the Kroniek tab; App.vue switches to it (KroniekTab.vue). */
export const toonInKroniek = (id) => {
  state.inKroniek = id;
};

/**
 * Wait until no sheet is closing. A link in the sheet (HerkomstSheet.vue)
 * closes it, but the sheet's modal dialog stays open during its closing
 * animation: the page behind it is inert and takes no focus, and on closing
 * the dialog returns focus to what opened it. The dialog sits in the
 * sheet's shadow DOM, and the sheet marks it `is-closing` while it animates
 * (sheet.js of @nldd/design-system). A sheet that stays open is no reason to
 * wait. Without a closing sheet the wait ends at once; it ends in any case
 * after 1.2 s, just after the sheet's own fallback of one second closes it.
 */
function sheetDicht() {
  const dialog = [...document.querySelectorAll('nldd-sheet')]
    .map((s) => s.shadowRoot?.querySelector('dialog'))
    .find((d) => d?.open && d.classList.contains('is-closing'));
  if (!dialog) return Promise.resolve();
  return new Promise((klaar) => {
    dialog.addEventListener('close', klaar, { once: true });
    setTimeout(klaar, 1200);
  });
}

/**
 * Bring the element `elementId` into view and give it focus, so a keyboard
 * user continues from there. The element needs `tabindex="-1"` to take it.
 */
export async function inBeeld(elementId) {
  await nextTick();
  await sheetDicht();
  const el = document.getElementById(elementId);
  if (!el) return;
  el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  el.focus?.({ preventScroll: true });
}
