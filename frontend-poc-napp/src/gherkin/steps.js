/**
 * Step definitions for the NAPP scenario suite — the in-browser counterpart
 * of backend/tests/bdd/steps/. Patterns must match scenarios/*.feature.
 */

export function parseValue(str) {
  // Text in double quotes is that text, as in the Rust harness and the
  // engine's own runner: an identifier of digits stays a string that way.
  if (typeof str === 'string' && str.length >= 2 && str.startsWith('"') && str.endsWith('"')) {
    return str.slice(1, -1);
  }
  if (str === 'true') return true;
  if (str === 'false') return false;
  if (str === 'null') return null;
  if (/^-?\d+$/.test(str)) {
    const n = parseInt(str, 10);
    if (Number.isSafeInteger(n)) return n;
  }
  if (/^-?\d+\.\d+$/.test(str)) {
    const f = parseFloat(str);
    if (Number.isFinite(f)) return f;
  }
  return str;
}

const WPP_ID = 'wet_op_de_politieke_partijen';

/**
 * Artikel 15 vraagt alle parameters; scenario's die de ledencomponent of de
 * neveninstellingen niet testen mogen die tabelrijen weglaten (gespiegeld
 * aan apply_besluit_defaults in de Rust-runner).
 */
function withBesluitDefaults(parameters) {
  return {
    totaal_aantal_betalende_leden: 0,
    heeft_wetenschappelijk_instituut: false,
    heeft_jongerenorganisatie: false,
    aantal_leden_jongerenorganisatie: 0,
    heeft_instelling_buitenland: false,
    ...parameters,
  };
}

function getOutput(ctx, name) {
  if (!ctx.result || !ctx.result.outputs) {
    throw new Error(
      `Geen outputs beschikbaar (${ctx.error ? `fout: ${ctx.error}` : 'niet uitgevoerd'})`,
    );
  }
  return ctx.result.outputs[name];
}

function assertOutput(ctx, name, expected) {
  const actual = getOutput(ctx, name);
  const equal =
    actual === expected ||
    (typeof actual === 'number' &&
      typeof expected === 'number' &&
      Math.abs(actual - expected) < 1e-9);
  if (!equal) {
    throw new Error(
      `Verwacht dat output "${name}" gelijk is aan ${JSON.stringify(expected)}, maar kreeg: ${JSON.stringify(actual)}`,
    );
  }
}

export const stepDefinitions = [
  {
    pattern: /^the calculation date is "([^"]+)"$/,
    execute: (ctx, _engine, match) => {
      ctx.calculationDate = match[1];
    },
  },
  {
    pattern: /^an application with the following data:$/,
    execute: (ctx, _engine, _match, step) => {
      if (!step.dataTable) return;
      for (const row of step.dataTable) {
        ctx.parameters[row[0]] = parseValue(row[1] ?? '');
      }
    },
  },
  {
    // Feiten uit de orchestratie (databronnen) als wet-parameters; zelfde
    // mechaniek als de aanvraagdata.
    pattern: /^the following facts:$/,
    execute: (ctx, _engine, _match, step) => {
      if (!step.dataTable) return;
      for (const row of step.dataTable) {
        ctx.parameters[row[0]] = parseValue(row[1] ?? '');
      }
    },
  },
  {
    pattern: /^the subsidiebesluit is executed$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          WPP_ID,
          ['subsidie_toegekend', 'subsidiebedrag'],
          withBesluitDefaults(ctx.parameters),
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    pattern: /^the subsidiebedragen of artikel 14 are calculated$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          WPP_ID,
          [
            'subsidie_partij',
            'subsidie_wetenschappelijk_instituut',
            'subsidie_jongerenorganisatie',
            'subsidie_buitenland',
            'subsidie_landelijk',
          ],
          withBesluitDefaults(ctx.parameters),
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    pattern: /^the verleningstermijnen are calculated$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          WPP_ID,
          ['aanvraagtermijn_einddatum', 'beslistermijn_einddatum', 'voorschotpercentage'],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    pattern: /^the termijnverlenging is calculated$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          'algemene_termijnenwet',
          ['verlengde_einddatum'],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    pattern: /^the beslistermijn is calculated including the termijnenwet$/,
    execute: (ctx, engine) => {
      try {
        const awb = engine.executeMultiple(
          'algemene_wet_bestuursrecht',
          ['beslistermijn_einddatum'],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.result = engine.executeMultiple(
          'algemene_termijnenwet',
          ['verlengde_einddatum'],
          { ...ctx.parameters, einddatum: awb.outputs.beslistermijn_einddatum },
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    pattern: /^the bezwaartermijn is calculated including the termijnenwet$/,
    execute: (ctx, engine) => {
      try {
        const awb = engine.executeMultiple(
          'algemene_wet_bestuursrecht',
          ['bezwaartermijn_startdatum', 'bezwaartermijn_einddatum'],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.result = engine.executeMultiple(
          'algemene_termijnenwet',
          ['verlengde_einddatum'],
          { ...ctx.parameters, einddatum: awb.outputs.bezwaartermijn_einddatum },
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    // Wpp art. 13: eenmalige verstrekking per subsidiejaar.
    pattern: /^the beschikbaarheid of artikel 13 is evaluated$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          WPP_ID,
          ['onderdeel_beschikbaar'],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    // Wpp art. 27: rekening op naam van de rechtspersoon.
    pattern: /^the rekening-regels of artikel 27 are evaluated$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          WPP_ID,
          ['rekening_aanvaardbaar', 'mag_rekening_wijzigen', 'uitbetaling_aangehouden'],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    // Kieswet G 1: registratie-eisen voor een aanduiding.
    pattern: /^the registratie-eisen of Kieswet G 1 are evaluated$/,
    execute: (ctx, engine) => {
      try {
        ctx.result = engine.executeMultiple(
          'kieswet',
          [
            'voldoet_aan_registratie_eisen',
            'voldoet_eis_inschrijving',
            'voldoet_eis_rechtsvorm',
            'voldoet_eis_naam',
          ],
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    // Generieke AWB-evaluatie: de gevraagde outputs, kommagescheiden.
    pattern: /^the AWB outputs "([^"]+)" are evaluated$/,
    execute: (ctx, engine, match) => {
      try {
        ctx.result = engine.executeMultiple(
          'algemene_wet_bestuursrecht',
          match[1].split(',').map((s) => s.trim()),
          ctx.parameters,
          ctx.calculationDate ?? '2026-06-01',
        );
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.result = null;
      }
      ctx.executed = true;
    },
  },
  {
    // Generieke boolean-assertie op een wet-output.
    pattern: /^the output "([^"]+)" is (true|false)$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, match[1], match[2] === 'true'),
  },
  {
    pattern: /^the verlengde einddatum is "([^"]+)"$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, 'verlengde_einddatum', match[1]),
  },
  {
    pattern: /^the subsidie is toegekend$/,
    execute: (ctx) => assertOutput(ctx, 'subsidie_toegekend', true),
  },
  {
    pattern: /^the subsidie is afgewezen$/,
    execute: (ctx) => assertOutput(ctx, 'subsidie_toegekend', false),
  },
  {
    pattern: /^the subsidiebedrag is "(-?\d+)" eurocent$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, 'subsidiebedrag', parseInt(match[1], 10)),
  },
  {
    pattern: /^a betaalopdracht of "(-?\d+)" eurocent is required$/,
    execute: (ctx, _engine, match) => {
      assertOutput(ctx, 'betaalopdracht_vereist', true);
      assertOutput(ctx, 'betaalopdracht_bedrag', parseInt(match[1], 10));
    },
  },
  {
    pattern: /^no betaalopdracht is required$/,
    execute: (ctx) => assertOutput(ctx, 'betaalopdracht_vereist', false),
  },
  {
    pattern: /^the bezwaartermijn is "(\d+)" weken$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, 'bezwaartermijn_weken', parseInt(match[1], 10)),
  },
  {
    pattern: /^motivering is vereist$/,
    execute: (ctx) => assertOutput(ctx, 'motivering_vereist', true),
  },
  {
    pattern: /^the output "([^"]+)" is "(-?\d+)" eurocent$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, match[1], parseInt(match[2], 10)),
  },
  {
    pattern: /^the aanvraagtermijn ends on "([^"]+)"$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, 'aanvraagtermijn_einddatum', match[1]),
  },
  {
    pattern: /^the beslistermijn ends on "([^"]+)"$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, 'beslistermijn_einddatum', match[1]),
  },
  {
    pattern: /^the voorschotpercentage is "(\d+)"$/,
    execute: (ctx, _engine, match) =>
      assertOutput(ctx, 'voorschotpercentage', parseInt(match[1], 10)),
  },
];
