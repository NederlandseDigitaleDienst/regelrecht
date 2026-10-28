/**
 * The prompt behind the "why" button: a plain-language explanation of one
 * law's outcome for one person, written from the engine's trace.
 *
 * Ported from poc-machine-law (`explain/base_llm_service.py`), where a
 * "waarom?" link next to a tile did the same over the Python engine's path.
 * Pure functions, so the tests can pin what the model is told without
 * starting the CLI.
 */

/** Languages the demo runs in, and how the answer should come back in each. */
const LANGUAGE_LINES = {
  nl: '',
  en: 'Schrijf je antwoord in het Engels (British English), in dezelfde eenvoudige stijl. Namen van wetten en organisaties blijven Nederlands.',
  fy: 'Schrijf je antwoord in het Fries (Frysk), in dezelfde eenvoudige stijl. Namen van wetten en organisaties blijven Nederlands.',
  fr: 'Schrijf je antwoord in het Frans, in dezelfde eenvoudige stijl. Namen van wetten en organisaties blijven Nederlands.',
};

export const LOCALES = Object.keys(LANGUAGE_LINES);

/**
 * The trace of a law that reaches into five others runs to a few thousand
 * lines. The cap keeps one request from carrying a runaway document; a normal
 * trace stays well below it.
 */
export const MAX_TRACE_CHARS = 150_000;

export function systemPrompt(locale) {
  const language = LANGUAGE_LINES[locale] ?? '';
  return [
    'Je bent een behulpzame overheidsmedewerker. Je legt één persoon uit waarom een wet voor hem of haar tot deze uitkomst leidt.',
    'Je krijgt de uitkomst zoals het portaal die toont, en de volledige uitvoering van de wet: elke stap, elk opgehaald gegeven en elke tussenuitkomst.',
    '',
    'Regels:',
    "- Schrijf in eenvoudig Nederlands, taalniveau B1. Spreek de lezer aan met 'je'.",
    '- Maak de uitleg persoonlijk en specifiek: noem de gegevens van deze persoon die de doorslag gaven, en de grenzen of bedragen uit de wet waarmee ze vergeleken werden.',
    '- Is er een bedrag berekend, leg dan uit hoe het is opgebouwd, met de bewerkingen waar dat helpt.',
    '- Neem bedragen over zoals ze in de uitkomst staan. In de uitvoering staan bedragen vaak in eurocenten: deel die door 100 en schrijf ze als € 1.234,56.',
    '- Is niet aan een voorwaarde voldaan, zeg dan welke en waarom niet.',
    '- Leverde een andere wet een waarde, noem die wet dan bij naam.',
    '- Verzin niets. Wat niet in de uitvoering staat, beweer je niet.',
    "- Dit is geen besluit. Wees niet stelliger dan de uitkomst: 'je hebt waarschijnlijk recht op', niet 'je krijgt'.",
    '- Gebruik geen technische woorden als trace, engine, output, parameter of variabele.',
    "- Geen kopjes en geen opmaak. Schrijf drie tot zes korte alinea's, gescheiden door een lege regel. Een korte opsomming met '- ' mag als dat helpt.",
    '- Begin meteen met de uitleg, zonder aanhef.',
    ...(language ? ['', language] : []),
  ].join('\n');
}

/**
 * Validate the request body and turn it into the user message. Returns
 * `{ error }` for a body the server should refuse, so the caller answers 400
 * without having to know the shape.
 */
export function buildRequest(body) {
  if (!body || typeof body !== 'object') return { error: 'body must be a JSON object' };
  const { law, outcome, trace_text: traceText } = body;
  if (!law || typeof law.name !== 'string' || !law.name.trim()) return { error: 'law.name is required' };
  if (typeof traceText !== 'string' || !traceText.trim()) return { error: 'trace_text is required' };
  if (outcome !== undefined && !Array.isArray(outcome)) return { error: 'outcome must be a list' };
  const locale = LOCALES.includes(body.locale) ? body.locale : 'nl';

  const trace = traceText.length > MAX_TRACE_CHARS
    ? `${traceText.slice(0, MAX_TRACE_CHARS)}\n[… de rest van de uitvoering is weggelaten]`
    : traceText;
  const lines = [
    `Wet: ${law.name}${law.id ? ` (${law.id})` : ''}`,
    ...(law.service ? [`Uitvoerder: ${law.service}`] : []),
    ...(body.reference_date ? [`Peildatum: ${body.reference_date}`] : []),
    '',
    'Uitkomst zoals het portaal die toont:',
    ...(typeof body.headline === 'string' && body.headline ? [body.headline] : []),
    ...(outcome ?? [])
      .filter((o) => o && typeof o.label === 'string')
      .map((o) => `- ${o.label}: ${String(o.value ?? '')}`),
    '',
    'Uitvoering van de wet:',
    '```',
    trace,
    '```',
  ];
  return { locale, system: systemPrompt(locale), message: lines.join('\n') };
}
