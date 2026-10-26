/**
 * Het contract tussen de demo en `WasmEngine.executeStage` (RFC-008).
 *
 * De binding zelf is niet native te testen: alles erin raakt een JsValue aan en
 * wasm-bindgen aborteert buiten wasm32 (zie `packages/.cargo/mutants.toml`).
 * Wat wél vastligt is wat de demo erin stopt en wat ze van de uitkomst maakt,
 * en dat is precies het stuk dat stuk kan gaan zonder dat iemand het merkt: een
 * veldnaam die verandert van `pending_inputs` naar iets anders, of een zaak
 * zonder verleden die als `undefined` in plaats van `null` naar binnen gaat.
 */
import { describe, expect, it } from 'vitest';
import { executeStage, type DemoEngine, type RawStageResult } from './useDemoEngine';

const law = { id: 'wet_op_de_huurtoeslag' };

/** Een engine die opschrijft waarmee hij is aangeroepen. */
function fakeEngine(result: RawStageResult | Error) {
  const calls: Parameters<DemoEngine['executeStage']>[] = [];
  return {
    calls,
    executeStage: (...args: Parameters<DemoEngine['executeStage']>) => {
      calls.push(args);
      if (result instanceof Error) throw result;
      return result;
    },
  };
}

describe('executeStage', () => {
  it('geeft een zaak zonder verleden als null door, niet als undefined', () => {
    // De engine leest beide als "deze zaak is nog niet begonnen", maar de demo
    // hoort er niet op te vertrouwen dat `undefined` de grens overleeft: door
    // serde-wasm-bindgen zijn dat niet dezelfde dingen.
    const engine = fakeEngine({ complete: false, outputs: {}, pending_inputs: ['aanvraag_datum'], current_stage: 'AANVRAAG', state: {} });
    executeStage(engine, law, 'subsidiebedrag', undefined, { bsn: '999' }, '2026-03-12');
    expect(engine.calls[0]?.[2]).toBeNull();
  });

  it('vertaalt de velden van de engine naar de namen die de demo gebruikt', () => {
    const engine = fakeEngine({
      complete: false,
      outputs: { termijn_weken: 6 },
      state: { current_stage: 'BEKENDMAKING' },
      pending_inputs: ['bekendmaking_datum'],
      current_stage: 'BEKENDMAKING',
    });
    const r = executeStage(engine, law, 'subsidiebedrag', null, {}, '2026-03-12');
    expect(r).toEqual({
      ok: true,
      complete: false,
      outputs: { termijn_weken: 6 },
      state: { current_stage: 'BEKENDMAKING' },
      pendingInputs: ['bekendmaking_datum'],
      currentStage: 'BEKENDMAKING',
    });
  });

  it('leest een afgeronde levensloop als klaar, zonder staat en zonder wachtlijst', () => {
    const engine = fakeEngine({ complete: true, outputs: { termijn_einddatum: '2026-04-23' } });
    const r = executeStage(engine, law, 'subsidiebedrag', null, {}, '2026-03-12');
    if (!r.ok) throw new Error(r.error);
    expect(r.complete).toBe(true);
    expect(r.state).toBeNull();
    expect(r.pendingInputs).toEqual([]);
    expect(r.outputs.termijn_einddatum).toBe('2026-04-23');
  });

  it('maakt van een fout een uitkomst en geen uitzondering', () => {
    // Een levensloop die niet rekent mag de demo niet omver halen; de zaak
    // houdt wat ze had en de fout is te zien waar de uitkomsten staan.
    const r = executeStage(fakeEngine(new Error('wet niet geladen')), law, 'x', null, {}, '2026-03-12');
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.error).toContain('wet niet geladen');
  });

  it('leest ook een fout die de engine als tekst gooit', () => {
    // wasm-bindgen gooit een string, geen Error.
    const engine = { executeStage: () => { throw 'LawNotFound'; } };
    const r = executeStage(engine, law, 'x', null, {}, '2026-03-12');
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.error).toBe('LawNotFound');
  });
});
