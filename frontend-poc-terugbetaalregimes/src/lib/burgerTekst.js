/**
 * B1-teksten voor de burger-view, in de tweede persoon ("je"). Houdt de
 * microcopy op één plek zodat de view zelf leesbaar blijft.
 *
 * Vertaalt regime-codes naar begrijpelijke zinnen en bouwt de rustige
 * onder-de-hero-regel per situatie (aflopend vs. doorlopend vs. veel
 * kwijtschelding).
 */
import { euro, euroWhole } from './format.js';

/** Eén zin: welke regels gelden voor deze persoon. Geen jargon. */
const REGIME_ZIN = {
  SF15_OUD: 'Voor jou gelden de terugbetaalregels van vóór 2009 (SF15-oud).',
  SF15_NIEUW: 'Voor jou gelden de terugbetaalregels van 2015–2023 (SF15-nieuw).',
  SF15_LLLK: 'Je betaalt een levenlanglerenkrediet terug (SF15-LLLK).',
  SF35: 'Voor jou gelden de regels van het studievoorschot (SF35).',
};

export function regimeZin(regime) {
  return REGIME_ZIN[regime] ?? '';
}

/**
 * De rustige regel onder het hero-bedrag. Kiest de formulering op basis van
 * de uitkomst: doorlopend (levenslang), aflopend met kwijtschelding, of
 * gewoon afbetaald.
 *
 * @param {object} totals - uit simulate()
 * @param {object} choices - de nu gekozen opties (voor de partner-opt-out-zin)
 */
export function heroOnderregel(totals, choices) {
  const restschuld = euroWhole(totals.startSchuld);

  if (totals.levenslang) {
    return `Je hebt nog ${restschuld} schuld. Je blijft betalen zolang het inkomen van je partner niet meetelt.`;
  }

  const eind = totals.eindejaar;
  // Alleen noemen als er een noemenswaardig bedrag wordt kwijtgescholden;
  // een paar centen afrondingsruis is geen kwijtschelding.
  if (totals.kwijtgescholden >= 100) {
    // Betaal je niets, dan ben je niet "klaar" maar loopt de termijn af. Dat
    // verschil is groot genoeg om anders te benoemen: je schuld groeit
    // ondertussen door met rente.
    if (totals.totaalBetaald < 100) {
      return `Je hebt ${restschuld} schuld. Je inkomen is te laag om te betalen. In ${eind} stopt je termijn en `
        + `hoef je de schuld (dan ${euroWhole(totals.kwijtgescholden)} met rente) niet meer te betalen.`;
    }
    return `Je hebt nog ${restschuld} schuld. In ${eind} ben je klaar. Wat er dan nog staat (${euroWhole(totals.kwijtgescholden)}) hoef je niet meer te betalen.`;
  }
  return `Je hebt nog ${restschuld} schuld. In ${eind} ben je klaar.`;
}

/**
 * Volzin-effect van een keuze, in de tweede persoon. Beschrijft het nieuwe
 * maandbedrag, de einddatum en eventuele kwijtschelding.
 */
/**
 * Vertelt het VERSCHIL tussen de huidige situatie (base) en de situatie mét
 * de keuze (met): eerst het voordeel (bijv. lager maandbedrag), daarna de
 * keerzijden (langer betalen, meer rente, schuld niet zelf aflossen, of bij
 * de partneropt-out: geen kwijtschelding en mogelijk levenslang). Alleen de
 * dimensies die echt veranderen; twee tot drie korte zinnen, tweede persoon.
 *
 * @param {object} base - summarize() zonder de keuze
 * @param {object} met - summarize() mét de keuze
 * @param {string} choice - het keuzetype (voor de partner-specifieke keerzijde)
 */
export function keuzeEffectZin(base, met, choice) {
  const delen = [];

  // Voordeel eerst: verandering in het maandbedrag. Blijft het bedrag gelijk
  // maar veranderen andere dimensies wel, open dan met een inleidende clause
  // zodat een keerzijde-zin niet met een kaal "Maar ..." begint.
  const maandbedragVerandert = met.maandbedragNu !== base.maandbedragNu;
  if (maandbedragVerandert) {
    delen.push(`Dan betaal je ${euro(met.maandbedragNu)} per maand in plaats van ${euro(base.maandbedragNu)}.`);
  } else {
    delen.push('Je maandbedrag blijft gelijk,');
  }
  // "Maar" met hoofdletter na een zin, met kleine letter na de inleidende clause.
  const maar = maandbedragVerandert ? 'Maar' : 'maar';

  // Keerzijde: partneropt-out laat de aflossing (mogelijk levenslang) doorlopen
  // en laat kwijtschelding vervallen.
  if (choice === 'partner_meetellen') {
    if (met.levenslang && !base.levenslang) {
      delen.push(`${maar} je blijft dan betalen zolang het inkomen van je partner niet meetelt. Misschien je leven lang. En aan het eind hoef je niets kwijtgescholden te krijgen.`);
    } else if (met.eindejaar && base.eindejaar && met.eindejaar > base.eindejaar) {
      delen.push(`${maar} je betaalt langer door: tot ${met.eindejaar} in plaats van ${base.eindejaar}. En aan het eind moet je alles zelf betaald hebben.`);
    }
    return delen.join(' ');
  }

  // Keerzijde: langere looptijd.
  if (met.eindejaar && base.eindejaar && met.eindejaar > base.eindejaar) {
    delen.push(`${maar} je betaalt langer door: tot ${met.eindejaar} in plaats van ${base.eindejaar}.`);
  }

  // Keerzijde: hogere totale kosten (vooral rente).
  const meerBetaald = met.totaalBetaald - base.totaalBetaald;
  const meerRente = met.totaalRente - base.totaalRente;
  if (meerBetaald >= 5000) {
    const renteZin = meerRente >= 5000 ? ' Dat komt vooral door de rente.' : '';
    delen.push(`Bij elkaar betaal je ${euroWhole(meerBetaald)} meer.${renteZin}`);
  }

  // Kwijtschelding: je lost je schuld dan niet zelf helemaal af.
  if (met.kwijtgescholden >= 100 && met.kwijtgescholden > base.kwijtgescholden) {
    delen.push(`Wat er aan het eind nog staat (${euroWhole(met.kwijtgescholden)}) hoef je niet te betalen. Je betaalt je schuld dus niet helemaal zelf af.`);
  }

  return delen.join(' ');
}

/**
 * Wat peiljaarverlegging (artikel 6.12) voor deze debiteur betekent, in
 * gewone taal en op basis van de doorrekening, niet op basis van een
 * algemene aanname.
 *
 * Uit de simulatie over alle persona's blijkt dat niemand er in totaal op
 * achteruitgaat, maar dat het effect sterk verschilt: bij sommigen daalt het
 * maandbedrag en wordt er meer kwijtgescholden, bij één duurt het een jaar
 * langer, en zonder draagkrachtmeting doet het niets. Vandaar dat de zin uit
 * de uitkomst komt.
 *
 * @param {object} zonder - samenvatting zonder verlegging
 * @param {object} met - samenvatting mét verlegging
 * @param {boolean} draagkrachtGevraagd - is er een draagkrachtmeting?
 * @returns {{ soort: 'geen'|'winst'|'langer', zin: string }}
 */
export function verleggingEffect(zonder, met, draagkrachtGevraagd) {
  if (!zonder || !met) return { soort: 'geen', zin: '' };

  const zelfdeBedrag = met.maandbedrag === zonder.maandbedrag;
  const zelfdeEinde = met.einde === zonder.einde;
  const zelfdeKwijt = met.kwijt === zonder.kwijt;

  if (zelfdeBedrag && zelfdeEinde && zelfdeKwijt) {
    return {
      soort: 'geen',
      zin: draagkrachtGevraagd
        ? 'Dit levert je nu niets op. Je betaalt al het lage bedrag dat hoort bij je schuld.'
        : 'DUO kijkt nu niet naar je inkomen. Je betaalt een vast bedrag. Zet hierboven aan dat DUO naar je inkomen kijkt. Dan zie je wat dit je oplevert.',
    };
  }

  const minder = zonder.maandbedrag - met.maandbedrag;
  const kop = minder > 0
    ? `Je maandbedrag gaat van ${euro(zonder.maandbedrag)} naar ${euro(met.maandbedrag)}.`
    : '';

  if (!zelfdeEinde) {
    return {
      soort: 'langer',
      zin: `${kop} Je bent wel langer bezig: tot ${met.einde} in plaats van ${zonder.einde}. `
        + 'Bij elkaar betaal je niet meer. Wat er aan het eind nog staat, hoef je niet te betalen.',
    };
  }

  if (met.kwijt > zonder.kwijt) {
    return {
      soort: 'winst',
      zin: `${kop} Je bent even lang bezig. Aan het eind hoef je ${euroWhole(met.kwijt)} niet te betalen, `
        + `in plaats van ${euroWhole(zonder.kwijt)}.`,
    };
  }

  return {
    soort: 'winst',
    zin: `${kop} Je bent even lang bezig. Verder verandert er niets aan je schuld.`,
  };
}

/**
 * Wat er níet gebeurt bij peiljaarverlegging. Dit is wat mensen tegenhoudt,
 * en artikel 6.12 kent geen sanctie: alleen de eis van 15% inkomensdaling.
 * Alleen tonen als de doorrekening dat ook laat zien.
 */
export const VERLEGGING_GERUSTSTELLING = [
  'Je krijgt geen boete.',
  'Je houdt het recht dat je aan het eind niet alles hoeft te betalen.',
  'Het geldt voor dit jaar. Verdien je volgend jaar meer? Dan rekent DUO daar weer mee.',
  'Je mag je inkomen schatten. DUO kijkt later naar je echte inkomen en verrekent het verschil.',
];

/**
 * Is de draagkrachtmeting een keuze voor deze debiteur? `sim` is de
 * doorrekening zónder aanvraag. Is de meting dan niet van toepassing, dan
 * valt de debiteur onder hoofdstuk 10a (artikel 10a.7) en moet hij hem zelf
 * aanvragen. Staat hij ook zonder aanvraag al aan, dan gaat het ambtshalve
 * (hoofdstuk 6) en valt er niets te kiezen. Een onbekende uitkomst is geen
 * keuze.
 */
export function draagkrachtmetingIsKeuze(sim) {
  return sim?.timeline?.[0]?.draagkrachtmeting_van_toepassing === false;
}

/**
 * De waarschuwing over het budget bij het eerste jaar van de doorrekening,
 * of null zonder betalingsprobleem.
 *
 * Capaciteit nul betekent: na de vaste lasten volgens het
 * Nibud-referentiebudget blijft er niets over. Dan helpt "kijk of je minder
 * kunt betalen" niet, want ook een lager bedrag knelt. Bij een bedrag van een
 * paar euro is die tekst bovendien ongeloofwaardig.
 */
export function budgetSignaal(t0) {
  if (!t0?.betalingsprobleem) return null;
  if (!t0.afloscapaciteit) {
    return {
      variant: 'warning',
      tekst: `Je inkomen is te laag voor de vaste lasten die het Nibud voor jouw huishouden rekent. Ook ${euro(t0.maandbedrag)} per maand is dan lastig. Kom je er niet uit, neem dan contact op met DUO of met schuldhulpverlening in je gemeente.`,
    };
  }
  return {
    variant: 'warning',
    tekst: `Dit maandbedrag ligt boven wat je volgens het Nibud kunt missen (ongeveer ${euro(t0.afloscapaciteit)} per maand). Kijk bij je keuzes of je minder kunt betalen.`,
  };
}
