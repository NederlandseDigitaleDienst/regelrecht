// De toets van het DSO-spoor. Zelfde vorm van antwoord als rekenToets in de eerste PoC: per kolom
// (tijdelijk, nieuw) een status, een uitkomst en de stappen per artikel. Daarbij: de andere
// overheden en de plichten (vergunning, melding) die het bouwplan op de plek meebrengt.
//
// Geen code gedeeld met de eerste PoC. Rekenen doet de RegelRecht-engine per artikel (YAML in
// regels/ in de bron); deze module zet alleen feiten van de kaart en het bouwplan om in parameters
// en telt de uitkomsten op met dezelfde regel als de engine voor EN gebruikt (Kleene: nee gaat voor
// onbekend, onbekend gaat voor ja), zoals de eerste PoC dat ook doet voor aanvullende regels.

export const UITKOMST = { true: 'toegestaan', false: 'niet toegestaan', onbekend: 'onthouding' };
export const OVERHEDEN = ['provincie', 'waterschap', 'ministerie'];

export function lees(v) {
  if (v === true || v === false) return v;
  return null; // onbekend
}

/** Kleene-EN over waarden true, false of null. */
export function en(waarden) {
  if (waarden.some((w) => w === false)) return false;
  if (waarden.some((w) => w === null)) return null;
  return true;
}

const naarUitkomst = (w) => (w === true ? UITKOMST.true : w === false ? UITKOMST.false : UITKOMST.onbekend);

/** Feiten van de kaart voor één plan op het punt. */
export function kaartFeiten(plan) {
  const o = plan?.objecten ?? {};
  const getal = (s) => Number(String(s).replace(',', '.'));
  const mv = (o.maatvoeringen ?? []).flatMap((m) => m.omvang ?? []);
  const hoogte = mv.filter((x) => /maximum bouwhoogte/i.test(x.naam)).map((x) => getal(x.waarde)).filter(Number.isFinite);
  const lagenMv = mv.filter((x) => /bouwlagen/i.test(x.naam)).map((x) => getal(x.waarde)).filter(Number.isFinite);
  const lagenAand = (o.bouwaanduidingen ?? []).map((b) => (b.naam ?? '').match(/(\d+)\s*bouwlagen/i)).filter(Boolean).map((m) => Number(m[1]));
  const lagen = [...lagenMv, ...lagenAand];
  return {
    binnen_bouwvlak: (o.bouwvlakken ?? []).length > 0,
    aanduiding_gestapeld_ter_plaatse: (o.bouwaanduidingen ?? []).some((b) => /gestapeld/i.test(b.naam ?? '')),
    maatvoering_hoogte_ter_plaatse: hoogte.length > 0,
    ...(hoogte.length ? { maximum_bouwhoogte_ter_plaatse_m: Math.max(...hoogte) } : {}),
    maatvoering_lagen_ter_plaatse: lagen.length > 0,
    ...(lagen.length ? { maximum_bouwlagen_ter_plaatse: Math.max(...lagen) } : {}),
    functieaanduiding_ter_plaatse_noemt_wonen: (o.functieaanduidingen ?? []).some((f) => /wonen|woning/i.test(f.naam ?? '')),
  };
}

/** De standaard voor 'alle overige gegevens zijn gunstig'. */
export function gunstig(bouwplan) {
  return {
    gegeven: true, welstand: true, bodem: true, funderingsdiepte_m: 18, bouwwerk_valt_onder_artikel_22_27: false,
    woonfunctie_in_woongebouw: bouwplan.woningtype === 'gestapeld',
  };
}

/**
 * Reken één cel.
 * @param {object} a
 * @param {object} a.cel          stapel.json: { rd, rp: [...], dso: {...} }
 * @param {string[]} a.sleutels   lijst.json: de artikelen op deze cel
 * @param {object} a.artikelen    lijst.json: artikel-metadata per sleutel
 * @param {object} a.index        regels-index.json
 * @param {Function} a.voer       (law, output, params) => engine-uitkomst (true, false of onbekend)
 * @param {object} a.bouwplan     { lagen, meterPerLaag, woningtype, aantalWoningen }
 * @param {object} a.aannames     uit gunstig(), met per artikel een eventuele afwijking: aannames.artikel[sleutel] = true|false
 */
export function rekenCel({ cel, sleutels, artikelen, index, voer, bouwplan, aannames }) {
  const plan = (laag) => cel.rp.filter((p) => laag.includes(p.laag));
  const tam = plan(['tam'])[0] ?? null;
  const beoogd = plan(['beoogd'])[0] ?? null;
  const basis = plan(['basisplan'])[0] ?? null;
  const bp = {
    bouwhoogte_bouwplan_m: Math.round(bouwplan.lagen * bouwplan.meterPerLaag * 100) / 100,
    aantal_bouwlagen_bouwplan: bouwplan.lagen, woningtype_gestapeld: bouwplan.woningtype === 'gestapeld',
  };

  const stap = (k) => {
    const a = artikelen[k];
    const ix = index[k];
    if (!ix || ix.vorm === 'geen-tekst' || !ix.law) return { sleutel: k, vorm: ix?.vorm ?? 'niet-in-yaml', waarde: null, uitkomst: 'niet gerekend' };
    let params = {};
    if (ix.vorm === 'BOUW' || ix.vorm === 'GEBRUIK') {
      const p = cel.rp.find((x) => x.id === a.regeling);
      params = { ...kaartFeiten(p), ...bp };
    } else if (ix.vorm === 'GEGEVEN') {
      const w = aannames.artikel?.[k] ?? aannames.gegeven;
      if (typeof w === 'boolean') params = { aan_artikel_voldaan: w };
    } else if (ix.vorm === 'PLICHT' || ix.vorm === 'VERBOD') {
      params = { bouwwerk_valt_onder_artikel_22_27: aannames.bouwwerk_valt_onder_artikel_22_27, woonfunctie_in_woongebouw: aannames.woonfunctie_in_woongebouw, funderingsdiepte_m: aannames.funderingsdiepte_m };
    }
    const waarde = voer(ix.law, ix.output, params);
    return { sleutel: k, vorm: ix.vorm, waarde, uitkomst: naarUitkomst(waarde) };
  };

  const toetsend = (k) => ['toets', 'onbepaald'].includes(artikelen[k].rol);
  const gemeenteDso = sleutels.filter((k) => artikelen[k].bron === 'dso' && artikelen[k].overheid === 'gemeente' && toetsend(k));
  const hub = gemeenteDso.find((k) => artikelen[k].artikel === '22.29');
  const plicht2226 = gemeenteDso.find((k) => artikelen[k].artikel === '22.26');
  const overigOmgevingsplan = gemeenteDso.filter((k) => k !== hub && k !== plicht2226);

  function kolom(plannen) {
    const ids = new Set(plannen.map((p) => p.id));
    const eigen = sleutels.filter((k) => artikelen[k].bron === 'rp' && ids.has(artikelen[k].regeling) && toetsend(k));
    const stappen = eigen.map(stap);
    // Geen bouwregel of bestemmingsomschrijving gevonden (bijvoorbeeld omdat de dienst de tekst niet
    // levert): onbekend, niet stilzwijgend voldaan (cite-or-abstain).
    const bouwStappen = stappen.filter((s) => s.vorm === 'BOUW');
    const bouw = bouwStappen.length ? en(bouwStappen.map((s) => s.waarde)) : null;
    const gebruikStappen = stappen.filter((s) => s.vorm === 'GEBRUIK');
    // Op het punt telt de bestemmingsomschrijving van het basisplan; een facetplan heeft er geen.
    // Een TAM-plan heeft geen bestemmingsomschrijving; daar tellen de artikelen over wonen.
    const woonStappen = stappen.filter((s) => s.vorm === 'GEGEVEN' && (artikelen[s.sleutel].bundel ?? []).includes('wonen'));
    const isTam = plannen.some((p) => ['tam', 'beoogd'].includes(p.laag));
    const gebruik = gebruikStappen.length ? en(gebruikStappen.map((s) => s.waarde)) : isTam && woonStappen.length ? en(woonStappen.map((s) => s.waarde)) : null;
    const omgp = overigOmgevingsplan.map(stap);
    // Een artikel zonder tekst (gereserveerd) of zonder YAML telt niet mee; het staat wel in de stappen.
    const telt = (s) => s.uitkomst !== 'niet gerekend';
    const aanvullend = en([...stappen.filter((s) => s.vorm === 'GEGEVEN'), ...omgp.filter((s) => s.vorm === 'GEGEVEN')].filter(telt).map((s) => s.waarde));
    const params = Object.fromEntries(Object.entries({
      bouwregels_ter_plaatse_voldaan: bouw, gebruiksregels_ter_plaatse_voldaan: gebruik, aanvullende_regels_ter_plaatse_voldaan: aanvullend,
      niet_in_strijd_met_redelijke_eisen_van_welstand: aannames.welstand, bodem_staat_niet_in_de_weg: aannames.bodem,
    }).filter(([, v]) => typeof v === 'boolean'));
    const h = hub ? voer(index[hub].law, index[hub].output, params) : null;
    const hubStap = hub ? { sleutel: hub, vorm: 'HUB', waarde: h, uitkomst: naarUitkomst(h) } : null;
    const vp = plicht2226 ? stap(plicht2226) : null;
    return {
      status: 'gerekend', uitkomst: naarUitkomst(h),
      stappen: [vp, ...stappen, ...omgp, hubStap].filter(Boolean),
      deel: { bouwregels: naarUitkomst(bouw), gebruiksregels: naarUitkomst(gebruik), aanvullend: naarUitkomst(aanvullend) },
    };
  }

  let tijdelijk;
  if (tam) tijdelijk = { status: 'niet-van-toepassing', uitkomst: null, stappen: [], reden: `${tam.naam}: TAM-plan, artikel 2 verklaart het tijdelijk deel ter plaatse niet van toepassing` };
  else if (!basis) tijdelijk = { status: 'geen-plan', uitkomst: null, stappen: [], reden: 'het DSO geeft hier geen geldend bestemmingsplan met een bestemming' };
  else tijdelijk = kolom(cel.rp.filter((p) => ['basisplan', 'facet', 'paraplu', 'voorbereidingsbesluit'].includes(p.laag)));

  let nieuw;
  if (tam || beoogd) nieuw = { ...kolom([tam ?? beoogd]), beoogd: !tam };
  else nieuw = { status: 'geen-regels', uitkomst: null, stappen: [], reden: 'het nieuwe deel heeft hier geen bouwregels (afdeling 5.3 gereserveerd)' };

  const andere = {};
  for (const o of OVERHEDEN) {
    const ks = sleutels.filter((k) => artikelen[k].overheid === o && artikelen[k].rol === 'toets');
    // Een plicht (vergunning, melding) is geen weigering: die staat in plichten, niet in de uitkomst.
    const stappen = ks.map(stap).filter((s) => s.uitkomst !== 'niet gerekend' && s.vorm !== 'PLICHT');
    andere[o] = { uitkomst: stappen.length ? naarUitkomst(en(stappen.map((s) => s.waarde))) : null, aantal: ks.length, gerekend: stappen.length, stappen };
  }

  const plichten = [];
  const vind = (pred) => sleutels.find((k) => pred(artikelen[k]));
  if (plicht2226) {
    const s = stap(plicht2226);
    if (s.waarde === true) plichten.push({ overheid: 'gemeente', tekst: 'Omgevingsvergunning voor de omgevingsplanactiviteit bouwen', grond: 'omgevingsplan 22.26' });
  }
  const bbl = vind((a) => a.regeling.includes('BWBR0041297') && a.artikel === '2.17');
  if (bbl) {
    const s = stap(bbl);
    if (s.waarde === false) plichten.push({ overheid: 'rijk', tekst: 'Omgevingsvergunning voor de technische bouwactiviteit (geen gevolgklasse 1)', grond: 'Bbl 2.17; de vergunningplicht zelf staat in Omgevingswet 5.1, tweede lid, en die tekst zit niet in het DSO' });
    if (s.waarde === true) plichten.push({ overheid: 'rijk', tekst: 'Bouwmelding (gevolgklasse 1)', grond: 'Bbl 2.17 en 2.18' });
  }
  for (const k of sleutels) {
    const a = artikelen[k];
    if (a.rol !== 'toets' || a.bron !== 'dso' || a.overheid === 'gemeente' || a.regeling.includes('BWBR0041297')) continue;
    for (const q of a.kwalificaties ?? []) {
      if (['vergunningplicht', 'meldingsplicht', 'verbod'].includes(q)) plichten.push({ overheid: a.overheid, tekst: `${q}: ${a.titel || a.activiteiten?.[0] || ''}`.trim(), grond: `${a.regelingTitel} ${a.artikel}` });
    }
  }

  const geldend = tam ? nieuw : tijdelijk;
  const klasse = (k) => (k.status === 'gerekend' ? { toegestaan: 'toegestaan', 'niet toegestaan': 'niet', onthouding: 'onthouding' }[k.uitkomst] : k.status === 'geen-plan' ? 'open' : 'open');
  const gemeenteW = geldend.status === 'gerekend' ? (geldend.uitkomst === 'toegestaan' ? true : geldend.uitkomst === 'niet toegestaan' ? false : null) : null;
  const totaal = geldend.status === 'gerekend'
    ? naarUitkomst(en([gemeenteW, ...OVERHEDEN.map((o) => andere[o].uitkomst).filter(Boolean).map((u) => (u === 'toegestaan' ? true : u === 'niet toegestaan' ? false : null))]))
    : null;
  return {
    tijdelijk, nieuw, andere, plichten,
    geldend: klasse(geldend), beoogd: beoogd && !tam ? klasse(nieuw) : null, totaal,
  };
}
