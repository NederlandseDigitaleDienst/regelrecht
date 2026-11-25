// A submission whose decision a later record revoked: with nothing in its
// place the decider decides again, after that record; with a new decision in
// its place that one holds and nothing waits. What answers the submission
// comes from the server's `stand`; whether it waits for a decision, and so
// shows the button, from the server's queue (`open`) alone.
import { artikelNaam } from '../gedeeld/keten';
import { longDate } from '../gedeeld/datum';
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Ik, Stand, Tijdlijn, Voorvertoning } from '../gedeeld/types';
import type { Beslissing, EigenVerzoek, Kroniek, Wachtend } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';

const beslissen = vi.fn(async (b: Beslissing) => ({
  besluit: { ...gram('e', 'BESCHIKKING', 'awb#7:11', b.op), after: b.na, outputs: { verleend: true } },
  waarom: { ingevuld: [], antwoorden: [], bevindingen: [] },
}));
const voorvertoning = vi.fn(async (_b: Beslissing): Promise<Voorvertoning> => ({ stil: [] }));
const verzoeken = vi.fn(async (): Promise<{ verzoeken: EigenVerzoek[] }> => ({ verzoeken: [] }));
const tijdlijn = vi.fn(
  async (): Promise<Tijdlijn> => ({ stuk: 'c', grammen: [], stand: { rollen: {}, herroepen: {}, antwoorden: {} }, door_het_systeem: {}, volgende: [] }),
);
vi.mock('./api', () => ({
  backofficeApi: {
    beslissen: (b: Beslissing) => beslissen(b),
    voorvertoning: (b: Beslissing) => voorvertoning(b),
    verzoeken: () => verzoeken(),
    waarom: async () => ({ ingevuld: [], antwoorden: [], bevindingen: [] }),
    tijdlijn: () => tijdlijn(),
  },
  beheerApi: {},
}));

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [{ cel: 'rechtbank', cel_naam: 'Rechtbank Voorbeeld' }],
  indieningen: [],
  vandaag: '2026-12-01',
};
function gram(id: string, kind: string, basis: string, refers_to?: string): Gram {
  return { id, kind, basis, refers_to, recorded_on: '2026-11-15', recording_actor: 'gemeente' };
}
const bezwaar = gram('c', 'BEZWAAR', 'awb#7:11', 'b');
const opBezwaar = { ...gram('d', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: { bestreden_besluit_herroepen: false } };
const ontvangst = { ...gram('r', 'EXECUTOGRAM', 'awb#8:79', 'd'), fact: 'afschrift_van_de_uitspraak', recorded_on: '2026-11-20' };
const rollen: Stand['rollen'] = { c: 'indiening', d: 'besluit', r: 'vastlegging' };
const beslisser: Ik = {
  sub: 'x',
  naam: null,
  burger: false,
  identiteit: null,
  niveau: 'beslisser',
  cellen: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }],
  partijen_invoeren: false,
  klok_zetten: false,
};

// A pane left mounted keeps its timer, which would call the shared mocks in the next test.
enableAutoUnmount(afterEach);

/**
 * Let the pane settle. It asks for its preview on a timer, and
 * `flushPromises` alone resolves through `setImmediate`, which Node may run
 * before that timer: first let a timer pass, then the answer.
 */
async function rond() {
  await new Promise((r) => setTimeout(r, 0));
  await flushPromises();
}

/** The pane on the submission `c`, with `open` as the server's work queue. */
function toon(stand: Stand, grammen: Gram[], open: Wachtend[] = [], r = router()) {
  const kroniek: Kroniek = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: grammen.length, grammen, stand };
  return mount(BehandelenPaneel, {
    props: { cel: 'gemeente', id: 'c', casus, vormen: null, wie: beslisser, kronieken: [kroniek], open },
    global: { stubs: { teleport: true }, plugins: [r] },
  });
}
/** A router with the screens a sheet over the pane leads to. */
function router() {
  const leeg = { template: '<div />' };
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: leeg },
      { path: '/behandelen/:cel/:id', name: 'behandelen', component: leeg },
    ],
  });
}
/** The button under the title that opens the sheet to decide in. */
const knop = (w: ReturnType<typeof toon>) => w.findAll('nldd-button').find((b) => b.attributes('text') === 'Besluiten');
/** The button in the sheet that asks whether the decider is sure, and the one in that question that decides. */
const inSheet = (w: ReturnType<typeof toon>) => w.find('nldd-sheet nldd-button-group nldd-button');
const inModal = (w: ReturnType<typeof toon>) => w.findAll('nldd-modal-dialog nldd-button').at(-1);
describe('the decision button', () => {
  const besloten: Stand = { rollen, herroepen: {}, antwoorden: { c: { soort: 'besloten', besluit: 'd' } } };

  it('does not show on a submission whose decision stands', async () => {
    const w = toon(besloten, [bezwaar, opBezwaar]);
    await rond();
    expect(knop(w)).toBeUndefined();
    expect(w.findAll('nldd-banner').map((b) => b.attributes('text'))).toContain('Op deze aanvraag is besloten op 15 november 2026');
  });

  it('does not show when the queue no longer holds it, also while the chronicle the screen loaded still says it is open', async () => {
    // Another employee decided after this screen loaded its chronicle.
    const verouderd: Stand = { rollen, herroepen: {}, antwoorden: { c: { soort: 'open' } } };
    const w = toon(verouderd, [bezwaar], []);
    await rond();
    expect(knop(w)).toBeUndefined();
  });

  it('shows after an annulment with an order for a new decision, to decide after the receipt', async () => {
    const vernietigd: Stand = { rollen, herroepen: { d: 'r' }, antwoorden: { c: { soort: 'vervallen', besluit: 'd', door: 'r', na: 'r' } } };
    const w = toon(vernietigd, [bezwaar, opBezwaar, ontvangst], [{ stuk: bezwaar, na: 'r' }]);
    await rond();
    expect(knop(w)).toBeDefined();
  });
});

describe('a submission whose decision was revoked', () => {
  it('waits for nothing when a decision revoked it without one in its place, such as a withdrawal', async () => {
    const intrekking = { ...gram('i', 'BESCHIKKING', 'wet#3', 'd'), recorded_on: '2026-11-20' };
    const w = toon(
      { rollen: { ...rollen, i: 'besluit' }, herroepen: { d: 'i' }, antwoorden: { c: { soort: 'vervallen', besluit: 'd', door: 'i' } } },
      [bezwaar, opBezwaar, intrekking],
    );
    await rond();
    expect(w.find('nldd-badge').attributes('text')).toBe('Herroepen');
    expect(knop(w)).toBeUndefined();
    expect(w.findAll('nldd-inline-dialog').map((d) => d.attributes('text'))).not.toContain('Een beslisser van deze cel neemt het besluit');
  });

  it('is decided again after the receipt that annulled its decision', async () => {
    voorvertoning.mockResolvedValueOnce({ stil: [], voorlopig: { besluit: { ...gram('p', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: {} } } });
    const w = toon(
      { rollen, herroepen: { d: 'r' }, antwoorden: { c: { soort: 'vervallen', besluit: 'd', door: 'r', na: 'r' } } },
      [bezwaar, opBezwaar, ontvangst],
      [{ stuk: bezwaar, na: 'r' }],
    );
    await rond();
    // The earlier decision, marked as annulled, and the form to decide again.
    expect(w.find('nldd-badge').attributes('text')).toBe('Vernietigd');
    const banners = w.findAll('nldd-banner').map((b) => b.attributes('text'));
    expect(banners).toContain('Het besluit van 15 november 2026 geldt niet meer');
    expect(banners).toContain('Opnieuw beslissen');
    expect(knop(w)).toBeDefined();
    await knop(w)?.trigger('click');
    await rond();
    await inSheet(w).trigger('click');
    await inModal(w)?.trigger('click');
    await rond();
    expect(beslissen).toHaveBeenCalledWith(expect.objectContaining({ op: 'c', na: 'r' }));
    expect(w.findAll('nldd-banner').map((b) => b.attributes('text'))).toContain('Het besluit is genomen');
  });

  it('shows the new decision taken in its place as the one that holds, and offers no form', async () => {
    const opnieuw = { ...gram('e', 'BESCHIKKING', 'awb#7:11', 'c'), recorded_on: '2026-12-01' };
    const aanvraag = gram('a', 'AANVRAAG', 'wet#1');
    const geweigerd = { ...gram('b', 'BESCHIKKING', 'wet#1', 'a'), outputs: { verleend: false } };
    const w = mount(BehandelenPaneel, {
      props: {
        cel: 'gemeente',
        id: 'a',
        casus,
        vormen: null,
        wie: beslisser,
        kronieken: [
          {
            cel: 'gemeente',
            cel_naam: 'gemeente Voorbeeld',
            kop: 3,
            grammen: [aanvraag, geweigerd, opnieuw],
            stand: {
              rollen: { a: 'indiening', b: 'besluit', e: 'besluit' },
              herroepen: { b: 'e' },
              antwoorden: {
                a: {
                  soort: 'in_de_plaats',
                  besluit: 'b',
                  door: 'e',
                  uitvoer: 'nieuw_besluit',
                  nieuw: { outputs: { verleend: true }, recorded_on: '2026-12-01' },
                },
              },
            },
          },
        ],
      },
      global: { plugins: [router()] },
    });
    await rond();
    const titels = w.findAll('nldd-title').map((t) => t.attributes('text'));
    expect(titels).toContain('Het eerdere besluit');
    expect(titels).toContain('Het geldende besluit');
    expect(w.find('nldd-badge').attributes('text')).toBe('Herroepen');
    expect(knop(w)).toBeUndefined();
  });
});

describe('the decision as it would be taken', () => {
  const wacht: Stand = { rollen, herroepen: {}, antwoorden: { c: { soort: 'open' } } };
  const keten = [
    {
      soort: 'artikel',
      wet: 'awb',
      artikel: '7:11',
      stappen: [{ soort: 'uitkomst', naam: 'verleend', waarde: true, ontbreekt: [], stappen: [] }],
    },
  ];

  it('shows without a click, as a preview, with its outcome, why and the full chain beside the button', async () => {
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [],
      voorlopig: {
        besluit: { ...gram('p', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: { verleend: true } },
        waarom: { ingevuld: [], antwoorden: [], bevindingen: [], keten },
      },
    });
    const w = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    expect(voorvertoning).toHaveBeenCalledWith(expect.objectContaining({ op: 'c' }));
    const titels = w.findAll('nldd-title').map((t) => t.attributes('text'));
    expect(titels).toContain('Voorvertoning van het besluit');
    // The outcome is the output in the list, with no tag beside the title.
    const uitkomst = w.findAll('nldd-list[accessible-label="Uitkomst van de voorvertoning"] nldd-list-item');
    expect(uitkomst.map((r) => r.findAll('nldd-text-cell').map((c) => c.attributes('text')))).toEqual([['Verleend', 'Ja']]);
    expect(w.find('nldd-title nldd-tag').exists()).toBe(false);
    // Not inline: a link in a sentence opens a sheet of its own with the steps.
    const meer = w.find('nldd-link a[href="#volledige-keten"]');
    expect(meer.exists()).toBe(true);
    const ketenSheet = () => w.findAll('nldd-sheet').find((x) => x.attributes('accessible-label') === 'Volledige keten')!;
    expect(ketenSheet().attributes('open')).toBeUndefined();
    await meer.trigger('click');
    expect(ketenSheet().attributes('open')).toBe('true');
    expect(ketenSheet().find('nldd-list[type="tree"]').findAll('nldd-list-item').length).toBeGreaterThan(0);
    await ketenSheet().trigger('close');
    expect(ketenSheet().attributes('open')).toBeUndefined();
    // An objection is no application: its button does not speak of granting one.
    expect(w.findAll('nldd-button').map((b) => b.attributes('text'))).toEqual(['Besluiten', 'Neem besluit', 'Controleer opnieuw', 'Neem besluit']);
  });

  const aanvraag = gram('c', 'AANVRAAG', 'wet#1');
  const afgewezen = { stil: [], voorlopig: { besluit: { ...gram('p', 'BESCHIKKING', 'wet#1', 'c'), outputs: { verleend: false } } } };

  it('asks whether the decider is sure before it decides, by the outcome it would have', async () => {
    beslissen.mockClear();
    voorvertoning.mockClear().mockResolvedValue(afgewezen);
    const r = router();
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }], r);
    await rond();
    await knop(w)?.trigger('click');
    await rond();
    expect(w.find('nldd-sheet').attributes('open')).toBeDefined();
    // The sheet has an address of its own, so the back button closes it.
    expect(r.currentRoute.value.hash).toBe('#besluiten');
    expect(inSheet(w).attributes('text')).toBe('Wijs aanvraag af');
    expect(inSheet(w).attributes('appearance')).toBe('destructive');
    // The bar sticks and takes over the title once it scrolls away.
    expect(w.find('nldd-sheet nldd-page').attributes('sticky-header')).toBeDefined();
    // It names the submission decided on, as the title of the pane does.
    const balk = w.find('nldd-sheet nldd-top-title-bar');
    const kop = w.find(`nldd-sheet nldd-title[id="${balk.attributes('collapse-anchor')}"]`);
    const titel = `${w.find('nldd-title#indiening-titel').attributes('text')} · ${longDate(aanvraag.recorded_on)}`;
    // A decision that asks no findings opens on its summary, with no way back to them.
    expect([kop.attributes('text'), kop.attributes('supporting-text')]).toEqual(['Samenvatting', titel]);
    expect(balk.attributes('text')).toBe(`Samenvatting: ${titel.charAt(0).toLowerCase()}${titel.slice(1)}`);
    expect(balk.attributes('back-text')).toBeFalsy();
    // Above the button, in words, what going on does.
    expect(w.findAll('nldd-sheet nldd-text').map((t) => t.text())).toContain(
      'Als u verdergaat, wordt de aanvraag afgewezen zoals hierboven staat, met de bevindingen en de kroniek van dit moment.',
    );
    expect(w.find('nldd-modal-dialog').attributes('open')).toBeUndefined();
    await inSheet(w).trigger('click');
    const modal = w.find('nldd-modal-dialog');
    expect(modal.attributes('open')).toBeDefined();
    expect(modal.attributes('text')).toBe('Aanvraag afwijzen?');
    // The way out is the primary button and comes first.
    expect(modal.findAll('nldd-button').map((b) => [b.attributes('text'), b.attributes('appearance')])).toEqual([
      ['Controleer opnieuw', 'primary'],
      ['Wijs aanvraag af', 'destructive'],
    ]);
    expect(beslissen).not.toHaveBeenCalled();
    await inModal(w)?.trigger('click');
    await rond();
    expect(beslissen).toHaveBeenCalledTimes(1);
    expect(w.find('nldd-sheet').attributes('open')).toBeUndefined();
    expect(r.currentRoute.value.hash).toBe('');
    voorvertoning.mockReset().mockResolvedValue({ stil: [] });
  });

  it('keeps the sheet open with the reason when deciding fails', async () => {
    voorvertoning.mockResolvedValueOnce(afgewezen);
    beslissen.mockRejectedValueOnce(new Error('De kroniek is intussen veranderd'));
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }]);
    await rond();
    await knop(w)?.trigger('click');
    await rond();
    await inSheet(w).trigger('click');
    await inModal(w)?.trigger('click');
    await rond();
    expect(w.find('nldd-modal-dialog').attributes('open')).toBeUndefined();
    expect(w.find('nldd-sheet').attributes('open')).toBeDefined();
    const fout = w.findAll('nldd-sheet nldd-banner').find((d) => d.attributes('text') === 'Dat lukt niet');
    expect(fout?.attributes('variant')).toBe('critical');
    expect(fout?.attributes('supporting-text')).toContain('De kroniek is intussen veranderd');
  });

  it('does not ask while the preview is being updated, and says so until it is', async () => {
    voorvertoning.mockResolvedValue(afgewezen);
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }]);
    await rond();
    await knop(w)?.trigger('click');
    await rond();
    // The chronicle changes: the preview runs again after a moment.
    const advies = { ...gram('a', 'EXECUTOGRAM', 'beleid#5'), fact: 'advies_ontvangen' };
    await w.setProps({ kronieken: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: 2, grammen: [aanvraag, advies], stand: wacht }] });
    await inSheet(w).trigger('click');
    expect(w.find('nldd-modal-dialog').attributes('open')).toBeUndefined();
    const nogNiet = () => w.findAll('nldd-sheet nldd-banner').find((d) => d.attributes('text') === 'Nog niet');
    expect(nogNiet()?.attributes('supporting-text')).toContain('wordt nog bijgewerkt');
    await new Promise((r) => setTimeout(r, 350));
    await rond();
    expect(nogNiet()).toBeUndefined();
    voorvertoning.mockReset().mockResolvedValue({ stil: [] });
  });

  it('offers no decision when the preview failed, and says so in a banner', async () => {
    voorvertoning.mockResolvedValueOnce(afgewezen).mockRejectedValueOnce(new Error('Geen verbinding'));
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }]);
    await rond();
    await knop(w)?.trigger('click');
    await rond();
    expect(inSheet(w).attributes('text')).toBe('Wijs aanvraag af');
    const advies = { ...gram('a', 'EXECUTOGRAM', 'beleid#5'), fact: 'advies_ontvangen' };
    await w.setProps({ kronieken: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: 2, grammen: [aanvraag, advies], stand: wacht }] });
    await new Promise((r) => setTimeout(r, 350));
    await rond();
    // Without a preview there is no outcome to name, so no button that would take it.
    expect(inSheet(w).exists()).toBe(false);
    const banner = w.findAll('nldd-sheet nldd-banner').find((b) => b.attributes('text') === 'De voorvertoning lukt niet');
    expect([banner?.attributes('variant'), banner?.attributes('supporting-text')]).toEqual(['critical', 'Geen verbinding']);
    expect(w.findAll('nldd-sheet nldd-inline-dialog')).toHaveLength(0);
  });

  it('offers no decision while none can be computed yet', async () => {
    voorvertoning.mockClear().mockResolvedValueOnce({ stil: [], geweigerd: 'missing' });
    const w = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    expect(inSheet(w).exists()).toBe(false);
    expect(w.findAll('nldd-sheet nldd-text').map((t) => t.text()).filter((t) => t.startsWith('Als u verdergaat'))).toEqual([]);
    const banner = w.findAll('nldd-sheet nldd-banner').find((b) => b.attributes('text') === 'Er is nog geen besluit te berekenen');
    expect(banner?.attributes('variant')).toBe('warning');
  });

  it('says what the system would then record itself, such as ordering the card', async () => {
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [],
      voorlopig: {
        besluit: { ...gram('p', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: { verleend: true } },
        waarom: { ingevuld: [], antwoorden: [], bevindingen: [] },
      },
      automatisch: [{ feit: 'beleid#7#kaart_besteld_bij_drukker', artikel: 'beleid#7', verwijst_naar: null }],
    });
    const w = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    expect(w.findAll('nldd-title').map((t) => t.attributes('text'))).toContain('Daarna doet het systeem zelf');
    // Under the page's title (h2), the preview, what it rests on and what
    // follows it are each h3.
    const niveaus = Object.fromEntries(w.findAll('nldd-sheet nldd-title').map((t) => [t.attributes('text'), t.attributes('heading-level')]));
    expect(niveaus).toMatchObject({
      'Voorvertoning van het besluit': '3',
      'Waarop het besluit rust': '3',
      'Daarna doet het systeem zelf': '3',
    });
    const rij = w.find('nldd-list[accessible-label="Wat het systeem daarna zelf vastlegt"] nldd-text-cell');
    expect(rij.attributes('text')).toBe('Kaart besteld bij drukker');
    expect(rij.attributes('supporting-text')).toBe(`Volgens ${artikelNaam('beleid#7')}`);
    // A refusal says what is missing; what would follow a decision does not show.
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [],
      geweigerd: 'missing',
      automatisch: [{ feit: 'beleid#7#kaart_besteld_bij_drukker', artikel: 'beleid#7', verwijst_naar: null }],
    });
    const geweigerd = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    expect(geweigerd.findAll('nldd-title').map((t) => t.attributes('text'))).not.toContain('Daarna doet het systeem zelf');
    // Nothing to tell: no heading.
    voorvertoning.mockClear().mockResolvedValueOnce({ stil: [], voorlopig: { besluit: { ...gram('p', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: {} } } });
    const leeg = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    expect(leeg.findAll('nldd-title').map((t) => t.attributes('text'))).not.toContain('Daarna doet het systeem zelf');
  });

  it('says what is missing instead of an outcome when no decision can be computed yet', async () => {
    // As kern::voorvertoning words it: a policy leaves the finding to the authority.
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [],
      geweigerd:
        "the policy leaves 'langdurige_loopbeperking_volgens_college' to the authority for this case: Required open term " +
        "'langdurige_loopbeperking_volgens_college' on regeling#3 is decided per case by the authority, but no judgement was given",
    });
    const w = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    const dialoog = w.findAll('nldd-banner').find((d) => d.attributes('text') === 'Er is nog geen besluit te berekenen');
    expect(dialoog?.attributes('supporting-text')?.toLowerCase()).toContain('"langdurige loopbeperking volgens college"');
    expect(dialoog?.attributes('supporting-text')).toContain('Geef die bevinding.');
    expect(w.findAll('nldd-title').map((t) => t.attributes('text'))).not.toContain('Voorvertoning van het besluit');
  });

  it('asks the findings first, and shows the decision and its button on the summary after them', async () => {
    const vraag = { naam: 'loopbeperking', type: 'boolean', basis: 'wet#1', beleid: true };
    voorvertoning.mockClear().mockResolvedValue({ ...afgewezen, stil: [vraag] });
    const r = router();
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }], r);
    await rond();
    await knop(w)?.trigger('click');
    await rond();
    const balk = () => w.find('nldd-sheet nldd-top-title-bar');
    const knoppen = () => w.findAll('nldd-sheet nldd-button').map((b) => b.attributes('text'));
    // The findings, without the decision: the page leads on to the summary.
    expect(w.find('nldd-sheet nldd-title#bevindingen-titel').attributes('text')).toBe('Besluiten');
    expect(w.findAll('nldd-sheet nldd-title').map((t) => t.attributes('text'))).not.toContain('Voorvertoning van het besluit');
    expect(knoppen()).toEqual(['Verder naar samenvatting']);
    // Above it, what these findings come to.
    expect(w.findAll('nldd-sheet nldd-text').map((t) => t.text())).toContain('Op basis van deze bevindingen wordt de aanvraag afgewezen.');
    await w.findAll('nldd-sheet nldd-button').find((b) => b.attributes('text') === 'Verder naar samenvatting')!.trigger('click');
    await rond();
    expect(r.currentRoute.value.hash).toBe('#besluiten-samenvatting');
    expect(w.find('nldd-sheet nldd-title#samenvatting-titel').attributes('text')).toBe('Samenvatting');
    expect(w.findAll('nldd-sheet nldd-title').map((t) => t.attributes('text'))).toContain('Voorvertoning van het besluit');
    expect(knoppen()).toEqual(['Wijs aanvraag af']);
    // The bar leads back to the findings.
    expect([balk().attributes('back-text'), balk().attributes('back-href')]).toEqual(['Besluiten', '#besluiten']);
    // The findings page has no heading of its own for them: the page is about nothing else.
    await r.push({ path: r.currentRoute.value.path, hash: '#besluiten' });
    await rond();
    expect(w.findAll('nldd-sheet[accessible-label="Besluiten"] nldd-title').map((t) => t.attributes('text'))).toEqual(['Besluiten']);
    // While no decision can be computed, the findings page says what is missing and leads nowhere.
    voorvertoning.mockResolvedValue({ stil: [vraag], geweigerd: 'missing' });
    await r.push({ path: r.currentRoute.value.path, hash: '#besluiten' });
    await w.setProps({ kronieken: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: 2, grammen: [aanvraag, { ...gram('a', 'EXECUTOGRAM', 'beleid#5'), fact: 'advies_ontvangen' }], stand: wacht }] });
    await new Promise((x) => setTimeout(x, 350));
    await rond();
    expect(w.findAll('nldd-sheet nldd-banner').map((b) => b.attributes('text'))).toContain('Er is nog geen besluit te berekenen');
    expect(knoppen()).toEqual([]);
    voorvertoning.mockReset().mockResolvedValue({ stil: [] });
  });

  it('lists on a page of its own which article leaves each finding to the executor, not under each question', async () => {
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [
        { naam: 'eerder_verstrekt', type: 'boolean', basis: 'beleid#5', beleid: true },
        { naam: 'loopbeperking', type: 'boolean', basis: 'wet#1', beleid: true },
        { naam: 'rolstoel', type: 'boolean', basis: 'wet#1', beleid: true },
      ],
      // One already asked, one more of an article on the form, one of an article not on it yet.
      mogelijk: [
        { naam: 'loopbeperking', type: 'boolean', basis: 'wet#1', beleid: true },
        { naam: 'hulp', type: 'boolean', basis: 'wet#1', beleid: true },
        { naam: 'blind', type: 'boolean', basis: 'beleid#9', beleid: true },
      ],
      geweigerd: 'missing',
    });
    const r = router();
    const w = toon(wacht, [bezwaar], [{ stuk: bezwaar }], r);
    await rond();
    expect(w.findAll('nldd-sheet nldd-form-field').map((f) => f.attributes('supporting-label') ?? '')).toEqual(['', '', '']);
    const link = w.find('nldd-sheet nldd-text nldd-link');
    expect([link.attributes('href'), link.attributes('text')]).toEqual(['#besluiten-herkomst', 'punten die de wet aan gemeente Voorbeeld overlaat']);
    await r.push({ path: r.currentRoute.value.path, hash: '#besluiten-herkomst' });
    await rond();
    expect(w.find('nldd-sheet nldd-title#herkomst-titel').attributes('text')).toBe('Waar deze punten vandaan komen');
    const balk = w.find('nldd-sheet nldd-top-title-bar');
    expect([balk.attributes('back-text'), balk.attributes('back-href')]).toEqual(['Besluiten', '#besluiten']);
    // A title per article, with the findings it leaves under it.
    const pagina = w.find('nldd-sheet nldd-title#herkomst-titel').element.parentElement!;
    const koppen = [...pagina.querySelectorAll('nldd-title[heading-level="3"]')].map((t) => t.getAttribute('text'));
    expect(koppen).toEqual([artikelNaam('beleid#5'), artikelNaam('wet#1'), artikelNaam('beleid#9')]);
    const lijsten = [...pagina.querySelectorAll('nldd-rich-text')].map((l) => [...l.querySelectorAll('li')].map((li) => li.textContent));
    expect(lijsten).toEqual([['Eerder verstrekt'], ['Loopbeperking', 'Rolstoel'], ['Hulp'], ['Blind']]);
    // What may still be asked stands apart, after what is asked.
    const apart = [...pagina.querySelectorAll('nldd-text')].filter((t) => t.textContent?.startsWith('Kan nog gevraagd worden'));
    expect(apart).toHaveLength(2);
  });

  it('adds a finding the preview asks for later under the others, whatever the UI spec order', async () => {
    const eerst = { naam: 'eerst', type: 'boolean', basis: 'wet#1', beleid: true };
    const later = { naam: 'later', type: 'boolean', basis: 'wet#1', beleid: true };
    voorvertoning.mockClear().mockResolvedValue({ stil: [eerst], geweigerd: 'missing' });
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }]);
    // The UI spec puts the later finding first.
    await w.setProps({ vormen: { teksten: {}, volgorde: ['later', 'eerst'], indieningen: {}, besluiten: {} } as never });
    await rond();
    voorvertoning.mockResolvedValue({ stil: [later], geweigerd: 'missing' });
    await w.setProps({ kronieken: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: 2, grammen: [aanvraag, { ...gram('a', 'EXECUTOGRAM', 'beleid#5'), fact: 'advies_ontvangen' }], stand: wacht }] });
    await new Promise((x) => setTimeout(x, 350));
    await rond();
    const vragen = w.findAll('nldd-sheet nldd-form-field').map((f) => f.attributes('label'));
    expect(vragen).toEqual(['Eerst', 'Later']);
    voorvertoning.mockReset().mockResolvedValue({ stil: [] });
  });

  it('marks the finding the decision still needs once the decider tries to go on without it', async () => {
    const vraag = { naam: 'loopbeperking', type: 'boolean', basis: 'wet#1', beleid: true };
    voorvertoning.mockClear().mockResolvedValue({
      stil: [vraag],
      geweigerd: "the policy leaves 'loopbeperking' to the authority for this case: no judgement was given",
    });
    const r = router();
    const w = toon(wacht, [aanvraag], [{ stuk: aanvraag }], r);
    await rond();
    await knop(w)?.trigger('click');
    await rond();
    // No banner over the whole form, but the button to go on.
    expect(w.findAll('nldd-sheet nldd-banner').map((b) => b.attributes('text'))).not.toContain('Er is nog geen besluit te berekenen');
    const groep = () => w.find('nldd-sheet nldd-radio-button-group');
    expect(groep().attributes('invalid')).toBeUndefined();
    await w.findAll('nldd-sheet nldd-button').find((b) => b.attributes('text') === 'Verder naar samenvatting')!.trigger('click');
    await rond();
    // It stays on the findings, and the question says what it lacks.
    expect(r.currentRoute.value.hash).toBe('#besluiten');
    expect(groep().attributes('invalid')).toBeDefined();
    const item = w.find('nldd-sheet nldd-validation-item');
    expect(groep().attributes('unmet')).toBe(item.attributes('id'));
    expect(item.text()).toBe('Een oordeel: het besluit kan niet zonder deze bevinding.');
    voorvertoning.mockReset().mockResolvedValue({ stil: [] });
  });

  it('asks again when the chronicle changes, such as when an advice comes in', async () => {
    voorvertoning.mockClear();
    const w = toon(wacht, [bezwaar], [{ stuk: bezwaar }]);
    await rond();
    expect(voorvertoning).toHaveBeenCalledTimes(1);
    const advies = { ...gram('a', 'EXECUTOGRAM', 'beleid#5', undefined), fact: 'advies_ontvangen' };
    const kroniek: Kroniek = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: 2, grammen: [bezwaar, advies], stand: wacht };
    await w.setProps({ kronieken: [kroniek] });
    await new Promise((r) => setTimeout(r, 350));
    await rond();
    expect(voorvertoning).toHaveBeenCalledTimes(2);
  });
});

describe('what was asked of another party', () => {
  const aanvraag = { ...gram('c', 'AANVRAAG', 'wet#1'), fields: { bsn: '999990000' } };
  const stand: Stand = { rollen: { c: 'indiening' }, herroepen: {}, antwoorden: {} };
  const verzoek = (id: string, fields: Record<string, unknown>): EigenVerzoek => ({
    verzoek: { ...gram(id, 'EXECUTOGRAM', 'beleid#9'), fact: 'onderzoek_gevraagd', fields },
    aan: 'ggd',
    aan_naam: 'GGD Voorbeeld',
    open: true,
    antwoord_ontvangen: null,
  });
  const lijst = (w: ReturnType<typeof toon>) => w.find('nldd-list[accessible-label="Gevraagd bij een eerdere aanvraag"]');

  it('leaves a request made for this submission to the timeline', async () => {
    verzoeken.mockResolvedValueOnce({ verzoeken: [verzoek('v', { aanvraag: 'c' })] });
    const w = toon(stand, [aanvraag]);
    await rond();
    expect(lijst(w).exists()).toBe(false);
    expect(w.findAll('nldd-text-cell').map((c) => c.attributes('text'))).not.toContain('Onderzoek gevraagd');
  });

  it('shows an open request made for an earlier submission of the same person, which the timeline lacks', async () => {
    verzoeken.mockResolvedValueOnce({ verzoeken: [verzoek('v', { aanvraag: 'eerder', bsn: '999990000' })] });
    const w = toon(stand, [aanvraag]);
    await rond();
    expect(lijst(w).find('nldd-text-cell').attributes('text')).toBe('Onderzoek gevraagd');
    expect(lijst(w).find('nldd-text-cell').attributes('supporting-text')).toBe('Aan GGD Voorbeeld, op 15 november 2026. Het antwoord is nog niet binnen.');
  });

  it('says under the title and its buttons whom the submission waits on and why', async () => {
    verzoeken.mockResolvedValueOnce({ verzoeken: [verzoek('v', { aanvraag: 'c' }), { ...verzoek('w', { aanvraag: 'c' }), open: false }] });
    const w = toon(stand, [aanvraag]);
    await rond();
    const banner = w.find('nldd-banner[icon="time"]');
    expect(banner.attributes('text')).toBe('Wacht op GGD Voorbeeld');
    expect(w.html().indexOf('icon="time"')).toBeGreaterThan(w.html().indexOf('id="indiening-titel"'));
    expect(w.html().indexOf('icon="time"')).toBeGreaterThan(w.html().indexOf('text="Besluiten"'));
    expect(banner.attributes('supporting-text')).toBe(`Onderzoek gevraagd op 15 november 2026, volgens ${artikelNaam('beleid#9')}.`);
  });

  it('says an open request of an earlier submission was asked there', async () => {
    verzoeken.mockResolvedValueOnce({ verzoeken: [verzoek('v', { aanvraag: 'eerder', bsn: '999990000' })] });
    const w = toon(stand, [aanvraag]);
    await rond();
    expect(w.find('nldd-banner[icon="time"]').attributes('supporting-text')).toBe(
      `Onderzoek gevraagd op 15 november 2026, bij een eerdere aanvraag, volgens ${artikelNaam('beleid#9')}.`,
    );
  });

  it('shows no waiting banner without an open request', async () => {
    verzoeken.mockResolvedValueOnce({ verzoeken: [{ ...verzoek('v', { aanvraag: 'c' }), open: false }] });
    const w = toon(stand, [aanvraag]);
    await rond();
    expect(w.find('nldd-banner[icon="time"]').exists()).toBe(false);
  });

  it('names the article the submission rests on under what was submitted, not under its title', async () => {
    const w = toon(stand, [aanvraag]);
    await rond();
    const titel = w.find('#indiening-titel');
    expect(titel.attributes('supporting-text')).not.toContain(artikelNaam('wet#1'));
    const rij = w.findAll('nldd-list-item').find((r) => r.find('nldd-text-cell').attributes('text') === 'Op grond van');
    expect(rij?.findAll('nldd-text-cell').at(-1)?.attributes('text')).toBe(artikelNaam('wet#1'));
  });
});

describe('the timeline under the pane', () => {
  const stand: Stand = { rollen, herroepen: {}, antwoorden: { c: { soort: 'besloten', besluit: 'd' } } };

  it('opens a record of the executor in a sheet over the pane, without leaving it', async () => {
    tijdlijn.mockResolvedValueOnce({ stuk: 'c', grammen: [bezwaar, opBezwaar], stand, door_het_systeem: {}, volgende: [] });
    const w = toon(stand, [bezwaar, opBezwaar]);
    await rond();
    const sheet = () => w.findAll('nldd-sheet').find((s) => s.find('nldd-top-title-bar[dismiss-text="Sluit"]').exists())!;
    expect(sheet().attributes('open')).toBeUndefined();
    const details = w.findAll('nldd-list[accessible-label="Tijdlijn van de aanvraag"] nldd-icon-button[text="Details"]');
    // The submission on screen has no button; its decision does.
    expect(details).toHaveLength(1);
    await details[0]!.trigger('click');
    await flushPromises();
    expect(sheet().attributes('open')).toBe('true');
    expect(sheet().find('nldd-top-title-bar').attributes('text')).toContain('15 november 2026');
    // The decision answers the submission on screen: the sheet does not lead to it again.
    expect(sheet().findAll('nldd-button').map((b) => b.attributes('text'))).not.toContain('Open in Aanvragen');
    sheet().trigger('close');
    await flushPromises();
    expect(sheet().attributes('open')).toBeUndefined();
  });

  it('opens the letter about the submission from its row in a bottom sheet, at an address of its own', async () => {
    tijdlijn.mockResolvedValueOnce({ stuk: 'c', grammen: [bezwaar, opBezwaar], stand, door_het_systeem: {}, volgende: [] });
    const r = router();
    const w = toon(stand, [bezwaar, opBezwaar], [], r);
    const ontvangst = { id: 'ontvangst', patroon: 'ontvangstbevestiging', vastlegging: { rol: 'indiening' as const, artikel: 'awb#7:11' }, delen: [] };
    await w.setProps({ vormen: { teksten: {}, indieningen: {}, besluiten: {}, berichten: { berichtsoorten: [ontvangst] } } as never });
    await rond();
    const rij = w.findAll('nldd-list[accessible-label="Tijdlijn van de aanvraag"] nldd-list-item')[0]!;
    await rij.find('nldd-icon-button[text="Bericht"]').trigger('click');
    await flushPromises();
    expect(r.currentRoute.value.hash).toBe('#bericht/c');
    const sheet = w.find('nldd-sheet[accessible-label="Bericht"]');
    expect([sheet.attributes('placement'), sheet.attributes('open')]).toEqual(['bottom', 'true']);
  });

  it('leads from a decision on another submission to that pane, and gives a record of another cell no button', async () => {
    const eerder = gram('b', 'AANVRAAG', 'wet#1');
    const opEerder = { ...gram('e', 'BESCHIKKING', 'wet#1', 'b'), outputs: { verleend: true } };
    const vanGgd = { ...gram('g', 'EXECUTOGRAM', 'beleid#9', 'c'), recording_actor: 'ggd' };
    const metEerder: Stand = { ...stand, rollen: { ...rollen, b: 'indiening', e: 'besluit', g: 'vastlegging' } };
    tijdlijn.mockResolvedValueOnce({ stuk: 'c', grammen: [eerder, opEerder, bezwaar, vanGgd], stand: metEerder, door_het_systeem: {}, volgende: [] });
    const r = router();
    const w = toon(metEerder, [eerder, opEerder, bezwaar], [], r);
    await rond();
    const rijen = w.findAll('nldd-list[accessible-label="Tijdlijn van de aanvraag"] nldd-list-item');
    expect(rijen.map((x) => x.find('nldd-icon-button[text="Details"]').exists())).toEqual([true, true, false, false]);
    await rijen[1]!.find('nldd-icon-button').trigger('click');
    await flushPromises();
    const sheet = w.findAll('nldd-sheet').find((x) => x.attributes('open') === 'true')!;
    await sheet.findAll('nldd-button').find((b) => b.attributes('text') === 'Open in Aanvragen')!.trigger('click');
    await flushPromises();
    expect(r.currentRoute.value.fullPath).toBe('/behandelen/gemeente/b');
  });
});
