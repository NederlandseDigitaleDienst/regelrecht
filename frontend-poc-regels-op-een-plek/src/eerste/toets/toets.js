// De toets op een punt: twee kolommen naast elkaar, tijdelijk en nieuw, elk met een
// eigen engine. Per kolom rekent de engine de regels van het plan op het punt door en geeft de
// uitkomsten aan artikel 22.29 van het omgevingsplan, dat het eindoordeel geeft: waar, onwaar
// of onbekend. Geen voorrang tussen de kolommen: beide worden getoond.
//
// Deze module rekent zelf niets. Ze kiest de regelingen en de parameters (plannen.js) en leest
// de uitkomst van de engine. Een onbekende uitkomst van een plan gaat niet als waarde naar
// 22.29 maar wordt weggelaten, zodat 22.29 zelf onbekend teruggeeft.
import { ARCHEOLOGIE, FACETTEN, KORT, OMGEVINGSPLAN, PARAPLU, PARAPLU_ARTIKELEN, PLANNEN, defOpPunt } from './plannen.js';

export const UITKOMST = { true: 'toegestaan', false: 'niet toegestaan', onbekend: 'onthouding' };

/** Uitkomst van de engine naar waarde en ontbrekende gegevens. */
export function lees(v) {
  if (v === true || v === false) return { waarde: v, uitkomst: UITKOMST[v], ontbrekend: [] };
  if (v && v.__unknown) return { waarde: null, uitkomst: UITKOMST.onbekend, ontbrekend: [...new Set((v.missing ?? []).map((m) => m.name))] };
  return { waarde: null, uitkomst: 'geen uitkomst', ontbrekend: [] };
}

/** Foutmelding van de engine als tekst. De WASM-engine gooit soms een object in plaats van een Error. */
function foutTekst(e) {
  const tekst = typeof e === 'string' ? e : (e?.error ?? e?.message);
  // Een markering die nog niet is geaccepteerd: de engine rekent het artikel dan niet.
  const m = tekst?.match(/Untranslatable construct in \S+ article (\S+): (.*?) —/);
  if (m) return `Artikel ${m[1]} heeft een markering die nog niet is geaccepteerd ("${m[2]}"). De engine rekent dit artikel pas na acceptatie.`;
  if (tekst) return tekst;
  try { return JSON.stringify(e); } catch { return String(e); }
}

// `rol` zegt wat de stap voor het besluit betekent: 'vergunningplicht', 'toepassing',
// 'aanvullend' (een laag bovenop het plan), 'parkeren' (paraplu 11, ook invoer van de bouwregels),
// 'gebruik', 'bouw' of 'hub'. De graaf (src/graaf/) legt er de lijnen mee; een stap zonder of met
// een andere rol wordt daar een los artikel. `boom` is de gestructureerde trace van de engine
// (RFC-039), `ingangen` wat de engine binnen de regeling uit andere artikelen las.
// `extra` zijn de aannames uit Simuleren (aannames.js). Ze vullen alleen aan wat het plan zelf
// niet invult: een feit van de kaart gaat altijd voor. `aangenomen` zegt welke de engine las.
function voer(engine, regeling, artikel, output, gegeven, datum, rol = null, extra = {}) {
  const eigen = Object.fromEntries(Object.entries(gegeven).filter(([, v]) => v !== undefined));
  const aanvulling = Object.fromEntries(Object.entries(extra).filter(([k]) => !(k in eigen)));
  const params = { ...aanvulling, ...eigen };
  const gelezen = (r) => Object.keys(aanvulling).filter((k) => r?.trace_text?.includes(k));
  try {
    const r = engine.executeWithTrace(regeling, output, params, datum);
    // Alle uitkomsten van het artikel, niet alleen de gevraagde: de simulatie toont ze per regel.
    const uitvoer = Object.fromEntries(Object.entries(r.outputs ?? {}).map(([k, v]) => [k, lees(v)]));
    return {
      regeling, artikel, output, params, rol, ...lees(r.outputs[output]), uitvoer, trace: r.trace_text ?? null,
      boom: r.trace?.root ?? null, ingangen: r.resolved_inputs ?? {}, aangenomen: gelezen(r),
    };
  } catch (e) {
    // De WASM-engine gooit soms een object met een (gedeeltelijke) trace in plaats van een Error.
    return {
      regeling, artikel, output, params, rol, waarde: null, uitkomst: 'fout', ontbrekend: [], fout: foutTekst(e),
      boom: e?.trace?.root ?? null, ingangen: {}, aangenomen: [],
    };
  }
}

/** Alleen bekende waarden doorgeven aan 22.29; onbekend blijft weg. */
function alsBekend(stap) {
  return stap && typeof stap.waarde === 'boolean' ? stap.waarde : undefined;
}

function hubParams(bouw, gebruik, aanvullend) {
  const p = {
    bouwregels_ter_plaatse_voldaan: alsBekend(bouw),
    gebruiksregels_ter_plaatse_voldaan: alsBekend(gebruik),
    aanvullende_regels_ter_plaatse_voldaan: aanvullend,
  };
  return Object.fromEntries(Object.entries(p).filter(([, v]) => typeof v === 'boolean'));
}

/**
 * De lagen bovenop het basisplan, uit de stapel: wat in YAML staat (defs) en wat nog open is.
 * Archeologie bij een plan van na het paraplu is open: of het paraplu daar geldt, is een vraag.
 */
function aanvullendOpPunt(stapel) {
  const defs = [];
  const open = [];
  for (const a of stapel.aanvullend ?? []) {
    const namen = [...new Set((a.objecten ?? []).map((o) => o.naam))];
    if (a.soort === 'paraplu-artikelen') {
      for (const ar of a.artikelen) {
        const d = PARAPLU_ARTIKELEN[String(ar.nr)];
        if (d) defs.push({ regeling: PARAPLU, ...d });
        else open.push(`${a.plan.naam}, artikel ${ar.nr} (${ar.onderwerp})`);
      }
    } else if (a.soort === 'archeologie') {
      for (const n of namen) {
        const d = ARCHEOLOGIE[n];
        if (d && !a.twijfel) defs.push({ regeling: PARAPLU, ...d, params: () => ({}) });
        else open.push(`${a.plan.naam}: ${n}${a.twijfel ? ' (of het paraplu hier geldt, is open)' : ''}`);
      }
    } else if (a.soort === 'facet') {
      for (const n of namen) {
        if (!(n in FACETTEN)) open.push(`${a.plan.naam}: ${n}`);
        else if (FACETTEN[n]) defs.push(FACETTEN[n]);
      }
    } else {
      open.push(`${a.plan.naam}: ${namen.join(', ') || a.soort}`);
    }
  }
  return { defs, open };
}

/**
 * Kleene-en over de aanvullende regels, zoals de engine AND rekent: onwaar als er één onwaar is,
 * waar als alles waar is en niets open staat, anders onbekend (undefined).
 */
function allemaal(stappen, open) {
  if (stappen.some((s) => s.waarde === false)) return false;
  if (!open.length && stappen.every((s) => s.waarde === true)) return true;
  return undefined;
}

async function kolomVoorPlan({ kolom, plan, feiten, def, label, bouwplan, datum, paden, maakEngine, aanvullend, extra }) {
  const engine = await maakEngine(paden);
  const stappen = [];
  const lagen = [];
  let parkeren;
  for (const d of aanvullend.defs) {
    const s = voer(engine, d.regeling, d.artikel, d.output, d.params(feiten, bouwplan), datum,
      d.regeling === PARAPLU && d.artikel === '11' ? 'parkeren' : 'aanvullend', extra);
    stappen.push(s);
    lagen.push(s);
    if (d.regeling === PARAPLU && d.artikel === '11') parkeren = alsBekend(s);
  }
  const gebruik = def.gebruik && (!def.gebruik.geldtOp || def.gebruik.geldtOp(feiten))
    ? voer(engine, def.regeling, def.gebruik.artikel, def.gebruik.output, def.gebruik.params(feiten, bouwplan), datum, 'gebruik', extra)
    : null;
  if (gebruik) stappen.push(gebruik);
  // Zonder bouwregels in YAML blijft de bouw onbekend, en staat hij bij wat nog open is.
  const bouw = def.bouw ? voer(engine, def.regeling, def.bouw.artikel, def.bouw.output, def.bouw.params(feiten, bouwplan, { parkeren }), datum, 'bouw', extra) : null;
  if (bouw) stappen.push(bouw);

  const nietGemodelleerd = [...aanvullend.open, ...(def.bouwOpen ? [def.bouwOpen] : []), ...Object.entries(def.nietGemodelleerd ?? {})
    .filter(([art]) => feiten.enkelbestemming.some((e) => String(e.artikel) === art)).map(([, t]) => t)];
  const aanv = allemaal(lagen, nietGemodelleerd);

  const gebruikParam = gebruik ? gebruik : { waarde: undefined };
  const hub = voer(engine, OMGEVINGSPLAN, '22.29', 'omgevingsvergunning_wordt_verleend', hubParams(bouw, gebruikParam, aanv), datum, 'hub', extra);
  stappen.push(hub);
  return {
    kolom, label, status: 'gerekend', plan: { id: plan.id, naam: plan.naam, datum: plan.datum, viewer: plan.viewer },
    bestemming: def.bestemming, stappen, nietGemodelleerd, eindoordeel: hub, aanvullendSamen: aanv,
  };
}

/** Aanvullingen in het TAM-plan zelf (bijvoorbeeld Het Ambacht 70 en 71), op hun eigen locatie. */
function aanvullendNieuw(def, feiten) {
  return { defs: (def.aanvullend ?? []).filter((d) => d.geldtOp(feiten)).map((d) => ({ regeling: def.regeling, ...d })), open: [] };
}

/** Waar de toets wel rekent, in één zin, voor bij een melding "niet in YAML". */
export function wasGedekt() {
  const delen = Object.values(PLANNEN).map((d) => KORT[d.regeling] ?? d.regeling);
  return `De toets rekent nu in ${delen.slice(0, -1).join(', ')} en ${delen.at(-1)}, niet overal met alle bestemmingen. Kies op de kaart het thema Toets om te zien waar.`;
}

/**
 * Wat de toets op een punt kan, zonder te rekenen: per kolom de status. De toets gebruikt
 * dit zelf, en de kaart tekent er de dekkingslaag mee. Zo zeggen kaart en toets hetzelfde.
 * @returns {{tijdelijk: string, nieuw: string}}  status per kolom, zie STATUS in ToetsTab.vue
 */
export function dekkingOpPunt(stapel) {
  let tijdelijk;
  if (stapel.tam) tijdelijk = 'niet-van-toepassing';
  else if (stapel.basisplan) {
    const def = PLANNEN[stapel.basisplan.plan.id];
    // Een plan met alleen een digitale plangrens (Het Ambacht 2009): de regels zijn een scan.
    // Dat telt als "bron ontbreekt", niet als "niet in YAML": hoe uit een scan te citeren is nog
    // niet besloten.
    if (stapel.basisplan.plan.alleen_plangrens) tijdelijk = 'bron-ontbreekt';
    else if (!stapel.basisplan.feiten.enkelbestemming.length) tijdelijk = 'geen-bestemming';
    else tijdelijk = def?.kolom === 'tijdelijk' && defOpPunt(def, stapel.basisplan.feiten) ? 'gerekend' : 'niet-gemodelleerd';
  } else {
    tijdelijk = (stapel.meldingen ?? []).some((x) => x.soort === 'bron-ontbreekt') ? 'bron-ontbreekt' : 'geen-plan';
  }
  let nieuw;
  const bron = stapel.tam ?? stapel.beoogd;
  if (bron) {
    const def = PLANNEN[bron.plan.id];
    nieuw = defOpPunt(def, bron.feiten) ? 'gerekend' : 'niet-gemodelleerd';
  } else nieuw = 'geen-regels';
  return { tijdelijk, nieuw };
}

/**
 * @param {object} a
 * @param {object} a.stapel       uit stapel.js bepaalStapel
 * @param {object} a.bouwplan     { lagen, meterPerLaag, woningtype, aantalWoningen, soortGebouw, voegtWoningenToe }
 * @param {object} [a.aannames]   parameters voor de engine uit Simuleren (aannames.js naarParams)
 * @param {string} a.datum        peildatum, YYYY-MM-DD
 * @param {Array}  a.index        laws/index.json
 * @param {Array}  a.varianten    laws/varianten.json
 * @param {Function} a.maakEngine (paden) => Promise<WasmEngine> met die regelingen geladen
 */
export async function rekenToets({ stapel, bouwplan: invoer, datum, index, varianten, maakEngine, aannames: extra = {} }) {
  const bouwplan = {
    soortGebouw: 'overig', voegtWoningenToe: true, ...invoer,
    bouwhoogte: Math.round(invoer.lagen * invoer.meterPerLaag * 100) / 100,
  };
  // Plat dak: de goothoogte is gelijk aan de bouwhoogte. Een aanname over het bouwplan.
  bouwplan.goothoogte = invoer.goothoogte ?? bouwplan.bouwhoogte;

  const pad = (id) => index.find((r) => r.id === id)?.path;
  const variantPad = (vid, id) => varianten.find((v) => v.id === vid)?.wetten.find((w) => w.id === id)?.path;
  const hubPad = pad(OMGEVINGSPLAN);
  const dekt = dekkingOpPunt(stapel);

  // Vergunningplicht (artikel 22.26): een nieuw hoofdgebouw valt niet onder 22.27.
  const e0 = await maakEngine([hubPad]);
  const vergunningplicht = voer(e0, OMGEVINGSPLAN, '22.26', 'omgevingsvergunning_vereist', { bouwwerk_valt_onder_artikel_22_27: false }, datum, 'vergunningplicht');

  // --- kolom tijdelijk ---
  let tijdelijk;
  if (stapel.tam) {
    const def = PLANNEN[stapel.tam.plan.id];
    let stap = null;
    if (def?.toepassing) {
      const e = await maakEngine([pad(def.regeling)]);
      stap = voer(e, def.regeling, def.toepassing.artikel, def.toepassing.output, def.toepassing.params(), datum, 'toepassing', extra);
    }
    tijdelijk = {
      kolom: 'tijdelijk', status: 'niet-van-toepassing', plan: null, stappen: stap ? [stap] : [],
      toelichting: `Op dit punt geldt het vastgestelde ${stapel.tam.plan.naam}. Artikel 2 daarvan zet de bestemmingsplannen uit het tijdelijk deel hier buiten toepassing, het paraplu inbegrepen. De regels staan in de kolom nieuw.`,
    };
  } else if (stapel.basisplan) {
    const { plan, feiten } = stapel.basisplan;
    const def = PLANNEN[plan.id];
    if (dekt.tijdelijk === 'gerekend') {
      const aanvullend = aanvullendOpPunt(stapel);
      // De overige artikelen van het plan zelf die op deze bestemming meetellen.
      aanvullend.defs.push(...aanvullendNieuw(defOpPunt(def, feiten), feiten).defs);
      const paden = [...new Set([hubPad, pad(def.regeling), ...aanvullend.defs.map((d) => pad(d.regeling))])];
      tijdelijk = await kolomVoorPlan({
        kolom: 'tijdelijk', plan, feiten, def: defOpPunt(def, feiten), label: 'geldend', bouwplan, datum, paden, maakEngine,
        aanvullend, extra,
      });
    } else if (dekt.tijdelijk === 'bron-ontbreekt') {
      const m = (stapel.meldingen ?? []).find((x) => x.soort === 'alleen-plangrens');
      tijdelijk = {
        kolom: 'tijdelijk', status: 'bron-ontbreekt', plan: { id: plan.id, naam: plan.naam, datum: plan.datum, viewer: plan.viewer }, stappen: [],
        toelichting: `${m?.tekst ?? `${plan.naam} geldt hier, maar alleen de plangrens is digitaal.`} Voorlopig een onthouding, in afwachting van de keuze hoe uit de scan te citeren.`,
      };
    } else if (dekt.tijdelijk === 'geen-bestemming') {
      tijdelijk = {
        kolom: 'tijdelijk', status: 'geen-bestemming', plan: { id: plan.id, naam: plan.naam, datum: plan.datum, viewer: plan.viewer }, stappen: [],
        toelichting: `${plan.naam} geldt hier, maar op dit punt ligt in de GML geen bestemming: een naad tussen twee vlakken. De regels zeggen hier niets.`,
      };
    } else {
      const best = feiten.enkelbestemming.map((e) => `${e.naam} (artikel ${e.artikel})`).join(', ') || 'geen bestemming';
      tijdelijk = {
        kolom: 'tijdelijk', status: 'niet-gemodelleerd', plan: { id: plan.id, naam: plan.naam, datum: plan.datum, viewer: plan.viewer }, stappen: [],
        toelichting: def
          ? `${plan.naam} geldt hier met ${best}. Van dit plan staat alleen ${def.bestemming} in YAML. ${wasGedekt()}`
          : `${plan.naam} geldt hier (${best}). Dit plan staat nog niet in YAML. ${wasGedekt()}`,
      };
    }
  } else {
    const m = (stapel.meldingen ?? []).find((x) => x.soort === 'bron-ontbreekt');
    tijdelijk = {
      kolom: 'tijdelijk', status: m ? 'bron-ontbreekt' : 'geen-plan', plan: null, stappen: [],
      toelichting: m ? `${m.tekst} Voorlopig een onthouding, in afwachting van de keuze hoe uit de scan te citeren.` : 'Geen geldend plan gevonden op dit punt.',
    };
  }

  // --- kolom nieuw ---
  let nieuw;
  const nieuwBron = stapel.tam ?? stapel.beoogd;
  if (nieuwBron) {
    const { plan, feiten } = nieuwBron;
    const def = PLANNEN[plan.id];
    const label = stapel.tam ? 'geldend' : 'beoogd';
    if (dekt.nieuw === 'gerekend') {
      const planPad = def.variant ? variantPad(def.variant, def.regeling) : pad(def.regeling);
      nieuw = await kolomVoorPlan({
        kolom: 'nieuw', plan, feiten, def: defOpPunt(def, feiten), label, bouwplan, datum, paden: [hubPad, planPad], maakEngine,
        aanvullend: aanvullendNieuw(defOpPunt(def, feiten), feiten), extra,
      });
    } else {
      nieuw = {
        kolom: 'nieuw', label, status: 'niet-gemodelleerd', plan: { id: plan.id, naam: plan.naam, datum: plan.datum, viewer: plan.viewer }, stappen: [],
        toelichting: def
          ? `${plan.naam} geldt hier ${label === 'beoogd' ? '(ontwerp) ' : ''}met andere locaties dan ${def.bestemming}; die staan nog niet in YAML. ${wasGedekt()}`
          : `${plan.naam} staat nog niet in YAML. ${wasGedekt()}`,
      };
    }
  } else {
    nieuw = {
      kolom: 'nieuw', status: 'geen-regels', plan: null, stappen: [],
      toelichting: 'Het nieuwe deel van het omgevingsplan heeft hier geen bouwregels: afdeling 5.3 Bouwactiviteiten is gereserveerd, en er ligt geen TAM-plan.',
    };
  }

  return { datum, bouwplan, aannames: extra, vergunningplicht, tijdelijk, nieuw };
}
