/**
 * useAssistent - praat met de beleidsassistent-backend (packages/poc-assistent,
 * poort 3600, via de vite dev-proxy op /api).
 *
 * Een gesprek en niet een losse opdracht: `run()` opent de SSE-stream en houdt
 * die open, en zolang hij openstaat kan er via `stuur()` een bericht bij en via
 * `antwoord()` een keuze terug. Beide gaan naar hetzelfde CLI-proces, dus de
 * assistent onthoudt wat er eerder is gezegd.
 *
 * De /api/assistent-respons is een POST-SSE-stream (text/event-stream). Een
 * EventSource kan geen POST doen, dus we lezen de ReadableStream zelf en
 * parsen 'data:'-regels als JSON. Berichttypes (contract van de backend):
 *   {type:'gesprek', gesprek_id}       eerst; nodig voor stuur/antwoord
 *   {type:'tekst', tekst}              complete beurt
 *   {type:'tekst_deel', tekst}         hetzelfde, terwijl het getypt wordt
 *   {type:'gebruiker', tekst}          wat wij insturen, voor de feed
 *   {type:'voortgang', beurten, seconden, status, tool}
 *   {type:'vraag', id, vraag, opties, meerkeuze?, toelichting?}
 *   {type:'vraag_verlopen', id}
 *   {type:'tool', naam, input}
 *   {type:'wijziging', document_key, toelichting}
 *   {type:'simulatie', doel, n?, metrics?, wijzigingen?}
 *   {type:'klaar', overlays: {document_key: yaml}, handelingen: yaml|null, beurten, seconden}
 *   {type:'fout', melding}
 */
import { ref } from 'vue';
import { b } from '../basePad.js';

// ---- Module-singleton --------------------------------------------------
//
// Deze state stond binnen `useAssistent()`, dus elke component kreeg een eigen
// instantie. Het assistentpaneel zit in BeleidView, en die unmount zodra je
// naar Scenario's of Casussen loopt: de state was weg, de stream brak af en de
// backend doodde het CLI-proces. Een doel-run mag 120 beurten doen, dus dat
// gebeurde midden in het werk.
//
// Op moduleniveau leeft het gesprek buiten het paneel, precies zoals
// usePopulation en useLawStore het al doen. Het paneel is nu een venster erop.
const available = ref(null); // null = onbekend, true/false na health-check
const model = ref(null);
// Kan deze omgeving een variant opslaan? Hosted niet: dat maakt een
// git-branch in de casus-checkout, en die is er in de container niet.
const variantOpslag = ref(false);
const streaming = ref(false);

/**
 * Het id van het lopende gesprek, zodra de backend het heeft gestuurd.
 * Nodig om een vervolgbericht of het antwoord op een keuze te versturen, en om
 * opnieuw aan te haken als de stream is weggevallen.
 */
const gesprekId = ref(null);

/**
 * Alles wat er in dit gesprek gebeurd is, in volgorde. Staat hier en niet in
 * het paneel, zodat het gesprek na een routewissel nog leesbaar is.
 */
const feed = ref([]);

/** De laatste voortgangsmelding en de afronding, voor de statusregel. */
const voortgang = ref(null);
const afronding = ref(null);

/** De keuze die nu voorligt, als de assistent er een stelde. */
const openVraag = ref(null);

/** Wat de assistent wijzigde, klaar om over te nemen. */
const overlays = ref(null);
const handelingenYaml = ref(null);

/**
 * Wat de bezoeker had ingesteld voordat hij wegliep: de gekozen modus, de
 * tekst die klaarstond, en het optimalisatiepad met het aangeklikte punt.
 *
 * Hoort bij het gesprek en niet bij het paneel. Stond dit in het paneel, dan
 * viel het bij een routewissel terug op de standaard: je had een doel gesteld
 * en kwam terug op 'vraag'.
 */
const modus = ref('vraag');
const prompt = ref('');
const pad = ref([]);
const gekozenPunt = ref(null);

/**
 * Meldingen voor wie niet op de beleidspagina staat. Elk item is
 * `{ id, kind, text }` (de vorm die AssistantNotices leest); de app haalt ze
 * weg zodra ze gezien zijn.
 */
const meldingen = ref([]);
let meldingTeller = 0;

/** Zet een melding klaar, en stuur er een systeemnotificatie bij als dat mag. */
function meld(kind, text) {
  meldingen.value = [...meldingen.value, { id: ++meldingTeller, kind, text }];
  toonNotificatie(text);
}

/** Haal een gelezen melding weg. */
function wisMelding(id) {
  meldingen.value = meldingen.value.filter((m) => m.id !== id);
}

/**
 * Een systeemnotificatie, maar alleen als de pagina verborgen is.
 *
 * Staat de pagina gewoon open, dan toont de browser zo'n melding niet of
 * nauwelijks, en is de melding in de app zelf het signaal. Toestemming vragen
 * we niet bij het laden maar pas bij de eerste opdracht: een browserprompt
 * zodra je binnenkomt is in een demo voor OCW lelijk.
 */
function toonNotificatie(tekst) {
  try {
    if (typeof Notification === 'undefined') return;
    if (Notification.permission !== 'granted') return;
    if (typeof document !== 'undefined' && document.visibilityState === 'visible') return;
    // eslint-disable-next-line no-new
    new Notification('Beleidsassistent', { body: tekst, tag: 'beleidsassistent' });
  } catch {
    // Notificaties zijn een extraatje; een weigering mag niets breken.
  }
}

/**
 * Vraag toestemming voor notificaties. Aanroepen vanuit een klik, want anders
 * weigert de browser de prompt.
 */
function vraagNotificatieToestemming() {
  try {
    if (typeof Notification === 'undefined') return;
    if (Notification.permission !== 'default') return;
    Notification.requestPermission().catch(() => {});
  } catch {
    // niet beschikbaar; dan blijft het bij de melding in de app
  }
}

let controller = null;

export function useAssistent() {

  async function checkHealth() {
    try {
      const res = await fetch(b('/api/health'), { signal: AbortSignal.timeout(3000) });
      if (!res.ok) throw new Error(String(res.status));
      const data = await res.json();
      available.value = !!data.ok;
      model.value = data.model ?? null;
      variantOpslag.value = data.varianten === true;
    } catch {
      available.value = false;
    }
    return available.value;
  }


  /** Breek de lopende run af; de backend stopt dan het CLI-proces. */
  function abort() {
    controller?.abort();
  }

  /**
   * Stuur een bericht naar het lopende gesprek. De assistent pakt het bij zijn
   * volgende beurt op, ook als hij nu nog aan het rekenen is: dat is precies
   * het punt van een gesprek in plaats van losse opdrachten.
   */
  async function stuur(tekst) {
    if (!gesprekId.value) throw new Error('Er loopt geen gesprek.');
    const res = await fetch(b(`/api/gesprek/${gesprekId.value}/bericht`), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tekst }),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).fout ?? `Backend gaf ${res.status}`);
  }

  /** Beantwoord een keuze die de assistent heeft voorgelegd. */
  async function antwoord(vraagId, keuzes) {
    if (!gesprekId.value) throw new Error('Er loopt geen gesprek.');
    const res = await fetch(b(`/api/gesprek/${gesprekId.value}/antwoord`), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ vraag_id: vraagId, keuzes }),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).fout ?? `Backend gaf ${res.status}`);
  }

  async function run({ modus, prompt, documenten = [], handelingen = null }, onEvent) {
    streaming.value = true;
    controller = new AbortController();
    try {
      const res = await fetch(b('/api/assistent'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ modus, prompt, documenten, handelingen }),
        signal: controller.signal,
      });
      if (!res.ok) {
        // 503 = te veel gesprekken tegelijk; die melding is voor de gebruiker.
        const body = await res.json().catch(() => ({}));
        throw new Error(body.fout ?? `Backend gaf ${res.status}`);
      }
      if (!res.body) throw new Error('De backend stuurde geen stream.');

      await leesStream(res.body, onEvent);
    } catch (e) {
      if (e?.name !== 'AbortError') onEvent({ type: 'fout', melding: String(e?.message ?? e) });
    } finally {
      streaming.value = false;
      // `gesprekId` blijft bewust staan. De stream kan eindigen zonder dat het
      // gesprek klaar is (een routewissel, een ververs), en dan is dit id het
      // enige waarmee we er weer op kunnen aanhaken. Het wordt gewist als de
      // backend `klaar` stuurt of als hervatten 404 geeft.
    }
  }

  /**
   * Haak opnieuw aan op een gesprek dat nog loopt.
   *
   * Voor wie terugkomt op de beleidspagina, of de pagina ververst heeft. De
   * backend stuurt eerst wat er gemist is en gaat daarna live verder. Loopt het
   * gesprek niet meer, dan geeft hij 404 en weten we dat er niets te volgen is.
   *
   * @returns {Promise<boolean>} of er werkelijk iets te hervatten viel
   */
  async function hervat(id, onEvent) {
    if (!id || streaming.value) return false;
    streaming.value = true;
    controller = new AbortController();
    try {
      const res = await fetch(b(`/api/gesprek/${id}/stream`), { signal: controller.signal });
      if (res.status === 404) {
        gesprekId.value = null;
        return false;
      }
      if (!res.ok || !res.body) throw new Error(`Backend gaf ${res.status}`);
      await leesStream(res.body, onEvent);
      return true;
    } catch (e) {
      if (e?.name !== 'AbortError') onEvent({ type: 'fout', melding: String(e?.message ?? e) });
      return false;
    } finally {
      streaming.value = false;
    }
  }

  /** Lees een SSE-stream en geef elk bericht door. */
  async function leesStream(stream, onEvent) {
    const reader = stream.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      // SSE-berichten zijn gescheiden door een lege regel.
      const parts = buffer.split('\n\n');
      buffer = parts.pop() ?? '';
      for (const part of parts) {
        for (const line of part.split('\n')) {
          const trimmed = line.trimStart();
          if (!trimmed.startsWith('data:')) continue;
          const payload = trimmed.slice(5).trim();
          if (!payload) continue;
          try {
            const ev = JSON.parse(payload);
            if (ev.type === 'gesprek') gesprekId.value = ev.gesprek_id ?? null;
            if (ev.type === 'klaar') gesprekId.value = null;
            onEvent(ev);
          } catch {
            // ongeldige regel overslaan
          }
        }
      }
    }
  }

  return {
    available,
    model,
    variantOpslag,
    streaming,
    gesprekId,
    feed,
    voortgang,
    afronding,
    openVraag,
    overlays,
    handelingenYaml,
    modus,
    prompt,
    pad,
    gekozenPunt,
    meldingen,
    meld,
    wisMelding,
    vraagNotificatieToestemming,
    checkHealth,
    run,
    hervat,
    stuur,
    antwoord,
    abort,
  };
}
