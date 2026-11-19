// De graaf van één geval: een punt op de kaart plus het bouwplan, met alles wat het besluit
// bepaalt en alles wat daarbij afvalt. Naar de GraafView van de RegelRecht-demo
// (frontend-demo/src/views/GraafView.vue): een kader per regel met invoer en uitvoer, lijnen
// van leverancier naar afnemer, van links (bronnen) naar rechts (besluit).
//
// Lagen, van links naar rechts:
//   bronnen     de kaart op het punt (GML), het bouwplan, de plannen en bestemmingen op het punt
//   regels      elk artikel dat de engine doorrekende, met invoer, tussenuitkomsten en de takken
//               van elke voorwaarde (gekozen en afgevallen), uit de trace van de engine (RFC-039)
//   22.29       per kolom (tijdelijk, nieuw) het artikel dat het oordeel geeft
//   besluit     vergunningplicht (22.26) en het oordeel per kolom, zonder voorrang
//
// Deze module rekent niets: ze leest de uitkomst van rekenToets (src/toets/toets.js), de stapel
// (src/kaart/stapel.js) en de plandefinities (src/toets/plannen.js), en tekent die.
import { KORT, PARAPLU, PLANNEN, defOpPunt } from '../toets/plannen.js';

// ---- maten (moeten passen bij de CSS in GraafKnoop.vue) -----------------------------------
export const MAAT = { breedte: 380, kop: 60, sectie: 22, rij: 26, tak: 22, onder: 10, tussen: 4 };
const LAAG_AFSTAND = 140;
const KADER_AFSTAND = 28;

// ---- waarden -----------------------------------------------------------------------------
const isOnbekend = (v) => v && typeof v === 'object' && v.__unknown;

export function waardeTekst(v) {
  if (v === true) return 'ja';
  if (v === false) return 'nee';
  if (isOnbekend(v)) return 'onbekend';
  if (typeof v === 'number') return v.toLocaleString('nl-NL');
  if (v === undefined || v === null) return '–';
  return String(v);
}

/** Toon van een feit of invoer: alleen onbekend valt op; ja of nee is hier geen oordeel. */
export const feitToon = (v) => (isOnbekend(v) ? 'onbekend' : 'neutraal');

/** Toon van een waarde: ok (waar), nee (onwaar), onbekend, neutraal (een getal of tekst). */
export function toonVan(v) {
  if (v === true) return 'ok';
  if (v === false) return 'nee';
  if (isOnbekend(v)) return 'onbekend';
  return 'neutraal';
}

const UITKOMST_TOON = { toegestaan: 'ok', 'niet toegestaan': 'nee', onthouding: 'onbekend', fout: 'nee' };
const leesbaar = (naam) => String(naam).replaceAll('_', ' ');

// ---- de trace van één artikel ------------------------------------------------------------
function* knopen(n) {
  if (!n) return;
  yield n;
  for (const c of n.children ?? []) yield* knopen(c);
}

/** Parameters die de engine las, met hun waarde, op volgorde van eerste gebruik. */
function parametersUit(boom) {
  const uit = new Map();
  for (const n of knopen(boom)) {
    if (n.node_type === 'resolve' && n.resolve_type === 'PARAMETER' && !uit.has(n.name)) uit.set(n.name, n.result);
  }
  return uit;
}

/** Beschrijving van de voorwaarde van een tak: de gelezen waarden in de tak. */
function voorwaarde(tak) {
  const delen = [];
  for (const n of knopen(tak)) {
    if (n.node_type === 'resolve') delen.push(`${leesbaar(n.name)} = ${waardeTekst(n.result)}`);
  }
  return [...new Set(delen)].slice(0, 3).join(', ') || tak.message || '';
}

/** De takken van de eerste ALS in een tussenuitkomst: welke gekozen, welke afgevallen. */
function takkenUit(actie) {
  const als = (actie.children ?? []).find((c) => c.node_type === 'operation' && c.name === 'IF');
  if (!als) return [];
  const takken = [];
  for (const c of als.children ?? []) {
    const m = /^CASE_(\d+)$/.exec(c.name);
    if (m) {
      const nr = Number(m[1]) + 1;
      // Een geval met onwaar als voorwaarde valt af; waar is gekozen; onbekend laat het open.
      const toon = c.result === false ? 'af' : c.result === true ? 'ok' : 'onbekend';
      takken.push({ naam: `geval ${nr}: ${voorwaarde(c)}`, waarde: c.result === false ? 'valt af' : c.result === true ? 'gekozen' : 'onbekend', toon });
    } else if (c.name === 'THEN') {
      takken.push({ naam: 'dan', waarde: waardeTekst(c.result), toon: toonVan(c.result) });
    } else if (c.name === 'DEFAULT') {
      takken.push({ naam: 'anders (geen geval paste)', waarde: waardeTekst(c.result), toon: toonVan(c.result) });
    }
  }
  return takken;
}

/** Tussenuitkomsten, markeringen en anker uit de trace van een artikel. */
function leesBoom(boom) {
  const kinderen = boom?.children ?? [];
  const acties = kinderen.filter((c) => c.node_type === 'action').map((a) => ({
    naam: a.name,
    waarde: a.result,
    basis: a.legal_basis ? [a.legal_basis.article && `artikel ${a.legal_basis.article}`, a.legal_basis.explanation].filter(Boolean).join(': ') : '',
    takken: takkenUit(a),
  }));
  const markeringen = kinderen.filter((c) => c.node_type === 'article' && String(c.name).startsWith('marking:'))
    .map((c) => ({ naam: String(c.name).slice('marking:'.length), uitleg: c.message ?? '' }));
  let anker = null;
  for (const n of knopen(boom)) if (n.anchor?.url) { anker = n.anchor; break; }
  return { acties, markeringen, anker };
}

// ---- herkomst van parameters --------------------------------------------------------------
const UIT_ANDERE_STAP = {
  parkeren_voldaan: 'parkeren',
  bouwregels_ter_plaatse_voldaan: 'bouw',
  gebruiksregels_ter_plaatse_voldaan: 'gebruik',
  aanvullende_regels_ter_plaatse_voldaan: 'aanvullend',
};
const AANNAME = {
  bouwwerk_valt_onder_artikel_22_27: 'Aanname: een nieuw hoofdgebouw valt niet onder de uitzonderingen van 22.27.',
  dakhelling_bouwplan_graden: 'Aanname over het bouwplan: plat dak.',
  goothoogte_bouwplan_m: 'Aanname over het bouwplan: plat dak, dus goothoogte gelijk aan bouwhoogte.',
  punt_in_locatie_tam_omgevingsplan: 'Volgt uit de kaart: het punt ligt in de locatie van het TAM-plan.',
};
function herkomst(naam) {
  if (UIT_ANDERE_STAP[naam]) return 'stap';
  if (/bouwplan|^soort_gebouw$|^bouwwerk_valt_onder/.test(naam)) return 'bouwplan';
  return 'kaart';
}

// ---- opbouw ------------------------------------------------------------------------------
/**
 * @param {object} a
 * @param {object} a.stapel       bepaalStapel
 * @param {object} a.toets        rekenToets
 * @param {[number,number]} a.rd  het punt in RD
 * @param {boolean} [a.afgevallen=true]  afgevallen opties tonen
 * @param {'beide'|'tijdelijk'|'nieuw'} [a.kolommen='beide']
 * @returns {{nodes: object[], edges: object[], kaders: object[]}}
 */
export function bouwGraaf({ stapel, toets, rd, afgevallen = true, kolommen = 'beide' }) {
  const kaders = [];
  const lijnen = [];
  const kader = (k) => { kaders.push({ secties: [], ...k }); return kaders.at(-1); };
  const sectie = (k, titel, rijen) => { if (rijen.length) k.secties.push({ titel, rijen }); };
  const lijn = (van, naar, toon, extra = {}) => lijnen.push({ van, naar, toon, ...extra });

  // Bronrijen worden pas gemaakt als een regel ze leest; zo staat er alleen in wat telt.
  const kaartK = kader({ id: 'kaart', soort: 'bron', baan: 'gedeeld', kop: 'Kaart op dit punt', sub: rd ? `RD ${Math.round(rd[0])}, ${Math.round(rd[1])} · uit de GML van het plan` : 'uit de GML' });
  const bouwK = kader({ id: 'bouwplan', soort: 'bron', baan: 'gedeeld', kop: 'Bouwplan', sub: `${toets.bouwplan.lagen} lagen × ${toets.bouwplan.meterPerLaag} m = ${toets.bouwplan.bouwhoogte} m · invoer en aannames` });
  const bron = { kaart: new Map(), bouwplan: new Map() };
  function bronRij(soort, naam, waarde) {
    const map = bron[soort];
    if (!map.has(naam)) {
      map.set(naam, { id: `${soort}:${naam}`, naam: leesbaar(naam), waarde: waardeTekst(waarde), toon: feitToon(waarde), uit: true, uitleg: AANNAME[naam] ?? '' });
    }
    return map.get(naam);
  }

  // Plannen op het punt: wat geldt, wat erbovenop komt, wat opzij gaat.
  const plannenK = kader({ id: 'plannen', soort: 'selectie', baan: 'gedeeld', kop: 'Plannen op dit punt', sub: `${stapel.opPunt?.length ?? 0} plannen · keuze volgens de gelaagdheidsregel` });
  const planRijen = [];
  const planRij = {};
  if (stapel.tam) {
    planRij[stapel.tam.plan.id] = { id: `plan:${stapel.tam.plan.id}`, naam: stapel.tam.plan.naam, waarde: 'geldt: TAM-plan', toon: 'ok', uit: true, uitleg: stapel.tam.plan.grond ?? '' };
  }
  if (stapel.basisplan) {
    planRij[stapel.basisplan.plan.id] = { id: `plan:${stapel.basisplan.plan.id}`, naam: stapel.basisplan.plan.naam, waarde: 'geldt: basisplan', toon: 'ok', uit: true, uitleg: stapel.basisplan.plan.grond ?? '' };
  }
  if (stapel.beoogd) {
    planRij[stapel.beoogd.plan.id] = { id: `plan:${stapel.beoogd.plan.id}`, naam: stapel.beoogd.plan.naam, waarde: 'beoogd (ontwerp)', toon: 'neutraal', uit: true, uitleg: 'Telt als beoogd.' };
  }
  for (const a of stapel.aanvullend ?? []) {
    const wat = a.soort === 'paraplu-artikelen' ? `daarbovenop: artikel ${a.artikelen.map((x) => x.nr).join(', ')}` : a.soort === 'archeologie' ? 'daarbovenop: archeologie' : 'daarbovenop: strook';
    const r = planRij[a.plan.id];
    if (r) r.waarde += ` · ${wat}`;
    else planRij[a.plan.id] = { id: `plan:${a.plan.id}`, naam: a.plan.naam, waarde: wat, toon: a.twijfel ? 'onbekend' : 'ok', uit: true, uitleg: a.tekst ?? '' };
  }
  planRijen.push(...Object.values(planRij));
  if (afgevallen) {
    for (const o of stapel.opzij ?? []) {
      if (planRij[o.plan.id]) continue;
      planRijen.push({ id: `plan:${o.plan.id}`, naam: o.plan.naam, waarde: o.twijfel ? 'twijfel' : 'valt af', toon: o.twijfel ? 'onbekend' : 'af', uitleg: o.reden });
    }
  }
  sectie(plannenK, 'Plannen', planRijen);
  for (const m of stapel.meldingen ?? []) {
    sectie(plannenK, 'Melding', [{ id: `melding:${m.soort}`, naam: m.tekst, waarde: '', toon: 'onbekend', uitleg: m.tekst }]);
  }

  // Besluit, rechts.
  const besluitK = kader({ id: 'besluit', soort: 'besluit', baan: 'gedeeld', kop: 'Besluit: omgevingsvergunning bouwen', sub: 'tijdelijk en nieuw deel naast elkaar, zonder voorrang' });
  const besluitRijen = [];

  // Vergunningplicht (22.26).
  const vp = toets.vergunningplicht;
  if (vp) {
    const k = artikelKader('gedeeld', vp, 'vp');
    besluitRijen.push({ id: 'besluit:vergunningplicht', naam: 'Vergunningplicht (22.26)', waarde: waardeTekst(vp.waarde ?? vp.uitvoer?.[vp.output]?.waarde), toon: toonVan(vp.waarde), in: true });
    lijn({ kader: k.id, rij: `${k.id}:uit` }, { kader: 'besluit', rij: 'besluit:vergunningplicht' }, toonVan(vp.waarde));
  }

  // Een artikel als kader: invoer, markeringen, tussenuitkomsten met takken, uitkomst.
  function artikelKader(baan, stap, sleutel) {
    const id = `art:${sleutel}`;
    const { acties, markeringen, anker } = leesBoom(stap.boom);
    const k = kader({
      id, soort: stap.rol === 'hub' ? 'hub' : 'artikel', baan,
      kop: `${KORT[stap.regeling] ?? leesbaar(stap.regeling)}, artikel ${stap.artikel}`,
      sub: leesbaar(stap.output), status: stap.uitkomst, toon: UITKOMST_TOON[stap.uitkomst] ?? toonVan(stap.waarde),
      link: anker?.url ?? null, bron: anker ? `${anker.law} · geldig vanaf ${anker.valid_from}` : '',
      fout: stap.fout ?? null,
    });
    // Invoer: de parameters die de engine las, plus wat hij uit andere artikelen las.
    const params = stap.boom ? parametersUit(stap.boom) : new Map(Object.entries(stap.params ?? {}));
    const invoer = [];
    for (const [naam, waarde] of params) {
      // Invoer die een uitkomst van een ander artikel is (22.29, parkeren), houdt de kleur van die uitkomst.
      const uitStap = herkomst(naam) === 'stap';
      const rij = { id: `${id}:in:${naam}`, naam: leesbaar(naam), waarde: waardeTekst(waarde), toon: uitStap ? toonVan(waarde) : feitToon(waarde), in: true, param: naam, uitleg: AANNAME[naam] ?? '' };
      if (stap.aangenomen?.includes(naam)) {
        rij.naam = `${rij.naam} (aanname)`;
        rij.uitleg = 'Ingevuld onder Simuleren, Gegevens invullen. Een aanname, geen feit.';
      }
      if (isOnbekend(waarde)) {
        rij.uitleg = uitStap
          ? 'Uitkomst van een ander artikel (zie de lijn ernaartoe); daar bleef hij onbekend.'
          : 'Niet gegeven: de engine kan dit deel niet uitrekenen.';
      }
      invoer.push(rij);
    }
    for (const [naam, waarde] of Object.entries(stap.ingangen ?? {})) {
      if (params.has(naam)) continue;
      invoer.push({ id: `${id}:in:${naam}`, naam: `${leesbaar(naam)} (uit een ander artikel)`, waarde: waardeTekst(waarde), toon: toonVan(waarde), in: true, ingang: naam });
    }
    sectie(k, 'Invoer', invoer);
    sectie(k, 'Markering', markeringen.map((m, i) => ({ id: `${id}:mark:${i}`, naam: m.naam, waarde: 'markering', toon: 'neutraal', uitleg: m.uitleg })));
    const tussen = acties.filter((a) => a.naam !== stap.output).map((a) => ({
      id: `${id}:act:${a.naam}`, naam: leesbaar(a.naam), waarde: waardeTekst(a.waarde), toon: toonVan(a.waarde), uitleg: a.basis,
      takken: afgevallen ? a.takken : a.takken.filter((t) => t.toon !== 'af'),
    }));
    sectie(k, 'Tussenuitkomsten', tussen);
    const eind = acties.find((a) => a.naam === stap.output);
    sectie(k, 'Uitkomst', [{
      id: `${id}:uit`, naam: leesbaar(stap.output), waarde: stap.fout ? 'fout' : stap.uitkomst, toon: k.toon, uit: true,
      uitleg: stap.fout ?? eind?.basis ?? '',
      takken: eind ? (afgevallen ? eind.takken : eind.takken.filter((t) => t.toon !== 'af')) : [],
    }]);
    // Bronlijnen naar de invoer.
    for (const r of invoer) {
      if (!r.param || isOnbekend(params.get(r.param))) continue;
      const h = herkomst(r.param);
      if (h === 'stap') continue;
      const b = bronRij(h, r.param, params.get(r.param));
      lijn({ kader: h, rij: b.id }, { kader: id, rij: r.id }, b.toon);
    }
    return k;
  }

  // Per kolom.
  const toonKolom = (kol) => kolommen === 'beide' || kolommen === kol;
  for (const kol of ['tijdelijk', 'nieuw']) {
    const k = toets[kol];
    if (!k || !toonKolom(kol)) continue;
    const naamKolom = kol === 'tijdelijk' ? 'Tijdelijk deel' : `Nieuw deel${k.label ? ` (${k.label})` : ''}`;
    const besluitRij = { id: `besluit:${kol}`, naam: naamKolom, in: true };
    besluitRijen.push(besluitRij);

    if (k.status !== 'gerekend') {
      // De kolom valt af of kan niet rekenen; de reden is zelf een kader.
      const id = `kolom:${kol}`;
      const af = k.status === 'niet-van-toepassing' || k.status === 'geen-regels';
      if (af && !afgevallen) {
        besluitRij.waarde = 'valt af';
        besluitRij.toon = 'af';
        continue;
      }
      const kk = kader({ id, soort: af ? 'afgevallen' : 'open', baan: kol, kop: `${naamKolom}: ${({ 'niet-van-toepassing': 'niet van toepassing', 'niet-gemodelleerd': 'niet in YAML', 'bron-ontbreekt': 'bron ontbreekt', 'geen-regels': 'geen regels over bouwen', 'geen-plan': 'geen plan', 'geen-bestemming': 'geen bestemming op dit punt' })[k.status] ?? k.status}`, sub: k.plan?.naam ?? '', toon: af ? 'af' : 'onbekend' });
      sectie(kk, 'Waarom', [{ id: `${id}:waarom`, naam: k.toelichting ?? '', waarde: '', toon: af ? 'af' : 'onbekend', in: true, uit: true, uitleg: k.toelichting ?? '' }]);
      for (const [i, s] of (k.stappen ?? []).entries()) {
        const ak = artikelKader(kol, s, `${kol}:${i}`);
        lijn({ kader: ak.id, rij: `${ak.id}:uit` }, { kader: id, rij: `${id}:waarom` }, toonVan(s.waarde));
      }
      if (k.plan && planRij[k.plan.id]) lijn({ kader: 'plannen', rij: planRij[k.plan.id].id }, { kader: id, rij: null }, 'neutraal');
      besluitRij.waarde = af ? 'valt af' : 'onthouding';
      besluitRij.toon = af ? 'af' : 'onbekend';
      lijn({ kader: id, rij: `${id}:waarom` }, { kader: 'besluit', rij: besluitRij.id }, besluitRij.toon);
      continue;
    }

    // Bestemmingen of locaties van het plan: welke hier geldt, welke afvallen.
    const def = PLANNEN[k.plan.id];
    const feiten = kol === 'tijdelijk' ? stapel.basisplan?.feiten : (stapel.tam ?? stapel.beoogd)?.feiten;
    const gekozen = def && feiten ? defOpPunt(def, feiten) : null;
    const bestK = kader({ id: `best:${kol}`, soort: 'selectie', baan: kol, kop: `Bestemmingen in ${KORT[def?.regeling] ?? k.plan.naam}`, sub: 'gemodelleerd in YAML · welke op dit punt geldt' });
    const opties = def?.bestemmingen ?? (def ? [def] : []);
    const bestRijen = [];
    for (const [i, b] of opties.entries()) {
      const geldt = gekozen && b.bestemming === gekozen.bestemming;
      if (!geldt && !afgevallen) continue;
      bestRijen.push({ id: `best:${kol}:${i}`, naam: b.bestemming, waarde: geldt ? 'op dit punt' : 'valt af', toon: geldt ? 'ok' : 'af', uit: geldt, uitleg: geldt ? '' : 'Ligt niet op dit punt.' });
    }
    sectie(bestK, 'Bestemmingen en locaties', bestRijen);
    if (planRij[k.plan.id]) lijn({ kader: 'plannen', rij: planRij[k.plan.id].id }, { kader: bestK.id, rij: null }, 'ok');
    const gekozenRij = bestRijen.find((r) => r.toon === 'ok');

    // De artikelen van de kolom. Lagen bovenop het plan (rol aanvullend of parkeren) kunnen er
    // meerdere zijn; gebruik, bouw en hub elk één.
    const perRol = {};
    const lagen = [];
    const aanvPlan = (soorten) => (stapel.aanvullend ?? []).find((a) => soorten.includes(a.soort))?.plan.id;
    for (const [i, s] of k.stappen.entries()) {
      const ak = artikelKader(kol, s, `${kol}:${i}`);
      const item = { stap: s, kader: ak };
      if (s.rol === 'aanvullend' || s.rol === 'parkeren') lagen.push(item);
      if (s.rol !== 'aanvullend') perRol[s.rol ?? `stap${i}`] = item;
      // Waar het artikel vandaan komt: de paraplu, een facetplan, of het plan zelf (bestemming).
      let herkomstRij = null;
      if (s.rol === 'aanvullend' || s.rol === 'parkeren') {
        const id = s.regeling === PARAPLU ? aanvPlan(['paraplu-artikelen', 'archeologie'])
          : s.regeling === def?.regeling ? null : aanvPlan(['facet']);
        herkomstRij = id && planRij[id] ? { kader: 'plannen', rij: planRij[id].id } : null;
      }
      if (!herkomstRij && s.rol !== 'hub' && gekozenRij) herkomstRij = { kader: bestK.id, rij: gekozenRij.id };
      if (herkomstRij) lijn(herkomstRij, { kader: ak.id, rij: null }, 'ok');
    }
    const uit = (item) => item && { kader: item.kader.id, rij: `${item.kader.id}:uit`, toon: toonVan(item.stap.waarde) };
    const uitVan = (rol) => uit(perRol[rol]);
    const naarIn = (rol, param) => perRol[rol] && { kader: perRol[rol].kader.id, rij: `${perRol[rol].kader.id}:in:${param}` };
    const heeftIn = (rol, param) => perRol[rol]?.kader.secties.some((sc) => sc.rijen.some((r) => r.id === `${perRol[rol].kader.id}:in:${param}`));

    // Parkeren (paraplu 11) gaat ook naar de bouwregels.
    if (uitVan('parkeren') && heeftIn('bouw', 'parkeren_voldaan')) lijn(uitVan('parkeren'), naarIn('bouw', 'parkeren_voldaan'), uitVan('parkeren').toon);
    // Wat het bouwartikel uit het gebruiksartikel las.
    if (uitVan('gebruik') && perRol.bouw) {
      for (const naam of Object.keys(perRol.bouw.stap.ingangen ?? {})) {
        if (naam === perRol.gebruik.stap.output && heeftIn('bouw', naam)) lijn(uitVan('gebruik'), naarIn('bouw', naam), uitVan('gebruik').toon);
      }
    }
    // Naar 22.29.
    if (perRol.hub) {
      if (uitVan('bouw') && heeftIn('hub', 'bouwregels_ter_plaatse_voldaan')) lijn(uitVan('bouw'), naarIn('hub', 'bouwregels_ter_plaatse_voldaan'), uitVan('bouw').toon);
      if (uitVan('gebruik') && heeftIn('hub', 'gebruiksregels_ter_plaatse_voldaan')) lijn(uitVan('gebruik'), naarIn('hub', 'gebruiksregels_ter_plaatse_voldaan'), uitVan('gebruik').toon);

      // Wat er bovenop ligt maar nog niet in YAML staat.
      let open = null;
      if (k.nietGemodelleerd?.length) {
        const id = `open:${kol}`;
        open = kader({ id, soort: 'open', baan: kol, kop: 'Nog niet in YAML', sub: 'ligt hier wel, maar wordt niet gerekend: het oordeel blijft open', toon: 'onbekend' });
        sectie(open, 'Regels', k.nietGemodelleerd.map((t, i) => ({ id: `${id}:${i}`, naam: t, waarde: 'open', toon: 'onbekend', uitleg: t })));
        sectie(open, 'Gevolg', [{ id: `${id}:uit`, naam: 'nog niet in YAML', waarde: 'onbekend', toon: 'onbekend', uit: true }]);
      }
      // De aanvullende regels samen: een Kleene-en in de toetscode (toets.js, allemaal()), niet in
      // de engine. Onwaar als er één onwaar is, waar als alles waar is en niets open, anders onbekend.
      if (lagen.length || open) {
        const id = `samen:${kol}`;
        const samen = kader({ id, soort: 'samen', baan: kol, kop: 'Aanvullende regels samen (EN)', sub: 'in de toetscode, niet in de YAML: één nee is nee, alles ja is ja, anders onbekend' });
        const ingangen = lagen.map((l) => ({
          id: `${id}:in:${l.kader.id}`, naam: `${KORT[l.stap.regeling] ?? leesbaar(l.stap.regeling)}, artikel ${l.stap.artikel}`,
          waarde: l.stap.uitkomst, toon: UITKOMST_TOON[l.stap.uitkomst] ?? 'onbekend', in: true,
        }));
        if (open) ingangen.push({ id: `${id}:in:open`, naam: 'nog niet in YAML', waarde: 'onbekend', toon: 'onbekend', in: true });
        sectie(samen, 'Invoer', ingangen);
        const w = k.aanvullendSamen;
        sectie(samen, 'Uitkomst', [{ id: `${id}:uit`, naam: 'aanvullende regels ter plaatse voldaan', waarde: waardeTekst(w), toon: w === undefined ? 'onbekend' : toonVan(w), uit: true }]);
        for (const l of lagen) lijn(uit(l), { kader: id, rij: `${id}:in:${l.kader.id}` }, uit(l).toon);
        if (open) lijn({ kader: open.id, rij: `${open.id}:uit` }, { kader: id, rij: `${id}:in:open` }, 'onbekend');
        // 22.29 kreeg een onbekende uitkomst niet mee; toon de ingang toch, als onbekend.
        if (!heeftIn('hub', 'aanvullende_regels_ter_plaatse_voldaan')) {
          const invoer = perRol.hub.kader.secties.find((sc) => sc.titel === 'Invoer');
          invoer?.rijen.push({ id: `${perRol.hub.kader.id}:in:aanvullende_regels_ter_plaatse_voldaan`, naam: 'aanvullende regels ter plaatse voldaan', waarde: 'onbekend', toon: 'onbekend', in: true, uitleg: 'Niet doorgegeven, omdat de samenvoeging onbekend is.' });
        }
        lijn({ kader: id, rij: `${id}:uit` }, naarIn('hub', 'aanvullende_regels_ter_plaatse_voldaan'), w === undefined ? 'onbekend' : toonVan(w));
      }
      const eind = k.eindoordeel;
      besluitRij.waarde = k.label === 'beoogd' ? `beoogd: ${eind.uitkomst}` : eind.uitkomst;
      besluitRij.toon = UITKOMST_TOON[eind.uitkomst] ?? 'onbekend';
      lijn(uitVan('hub'), { kader: 'besluit', rij: besluitRij.id }, besluitRij.toon);
    }
  }

  // Bronkaders vullen met de rijen die een regel las.
  sectie(kaartK, 'Gelezen op het punt', [...bron.kaart.values()]);
  sectie(bouwK, 'Gelezen uit het bouwplan', [...bron.bouwplan.values()]);
  sectie(besluitK, 'Oordeel', besluitRijen);

  const zichtbaar = kaders.filter((k) => k.secties.length);
  const ids = new Set(zichtbaar.map((k) => k.id));
  const geldig = lijnen.filter((l) => ids.has(l.van.kader) && ids.has(l.naar.kader));
  return { ...indeling(zichtbaar, geldig), kaders: zichtbaar };
}

// ---- indeling ----------------------------------------------------------------------------
export function hoogte(k) {
  let h = MAAT.kop;
  for (const s of k.secties) {
    h += MAAT.sectie;
    for (const r of s.rijen) h += MAAT.rij + (r.takken?.length ?? 0) * MAAT.tak;
    h += MAAT.tussen;
  }
  return h + MAAT.onder;
}

const BAAN = { gedeeld: 0, tijdelijk: 1, nieuw: 2 };

/** Lagen naar de langste weg vanaf een bron; binnen een laag op baan, dan op volgorde. */
function indeling(kaders, lijnen) {
  const voor = new Map(kaders.map((k) => [k.id, new Set()]));
  for (const l of lijnen) if (l.van.kader !== l.naar.kader) voor.get(l.naar.kader).add(l.van.kader);
  const laag = new Map();
  const bezig = new Set();
  const diepte = (id) => {
    if (laag.has(id)) return laag.get(id);
    if (bezig.has(id)) return 0; // cirkel: komt niet voor, maar breek hem
    bezig.add(id);
    let d = 0;
    for (const v of voor.get(id)) d = Math.max(d, diepte(v) + 1);
    bezig.delete(id);
    laag.set(id, d);
    return d;
  };
  for (const k of kaders) diepte(k.id);
  // Het besluit staat altijd helemaal rechts.
  const max = Math.max(0, ...laag.values());
  if (laag.has('besluit')) laag.set('besluit', Math.max(max, 1));

  const perLaag = new Map();
  for (const k of kaders) {
    const d = laag.get(k.id);
    if (!perLaag.has(d)) perLaag.set(d, []);
    perLaag.get(d).push(k);
  }
  const nodes = [];
  const hoogsteLaag = Math.max(...[...perLaag.values()].map((ks) => ks.reduce((s, k) => s + hoogte(k) + KADER_AFSTAND, 0)));
  for (const [d, ks] of [...perLaag.entries()].sort((a, b) => a[0] - b[0])) {
    ks.sort((a, b) => BAAN[a.baan] - BAAN[b.baan]);
    const totaal = ks.reduce((s, k) => s + hoogte(k) + KADER_AFSTAND, 0);
    let y = (hoogsteLaag - totaal) / 2;
    for (const k of ks) {
      const h = hoogte(k);
      nodes.push({
        id: k.id, type: 'kader', position: { x: d * (MAAT.breedte + LAAG_AFSTAND), y },
        data: k, style: { width: `${MAAT.breedte}px`, height: `${h}px` }, draggable: true,
      });
      y += h + KADER_AFSTAND;
    }
  }
  const edges = lijnen.map((l, i) => ({
    id: `l${i}`, source: l.van.kader, target: l.naar.kader,
    sourceHandle: l.van.rij ? `uit:${l.van.rij}` : 'kop-uit',
    targetHandle: l.naar.rij ? `in:${l.naar.rij}` : 'kop-in',
    data: { toon: l.toon },
    class: `gl gl--${l.toon}`,
  }));
  return { nodes, edges };
}
