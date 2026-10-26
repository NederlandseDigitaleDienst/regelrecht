import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { volger } from '../src/voorvertoning.js';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

/** Een verzoek dat pas antwoordt als de test het zegt. */
function verzoeken() {
  const open = [];
  const vraag = vi.fn(
    () =>
      new Promise((goed, fout) => {
        open.push({ goed, fout });
      }),
  );
  return { vraag, open };
}

describe('volger', () => {
  it('vraagt pas na 300 ms zonder wijziging, en dan één keer', async () => {
    const { vraag, open } = verzoeken();
    const toon = vi.fn();
    const v = volger(vraag, toon);
    v.plan();
    await vi.advanceTimersByTimeAsync(200);
    v.plan();
    await vi.advanceTimersByTimeAsync(299);
    expect(vraag).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(vraag).toHaveBeenCalledTimes(1);
    open[0].goed('verleend');
    await vi.runAllTimersAsync();
    expect(toon).toHaveBeenCalledWith({ antwoord: 'verleend' });
  });

  it('gooit een antwoord weg dat binnenkomt na een nieuwere wijziging', async () => {
    const { vraag, open } = verzoeken();
    const toon = vi.fn();
    const v = volger(vraag, toon);
    v.plan();
    await vi.advanceTimersByTimeAsync(300);
    v.plan();
    await vi.advanceTimersByTimeAsync(300);
    expect(vraag).toHaveBeenCalledTimes(2);
    // Het nieuwste antwoordt eerst, het oude daarna: het oude telt niet.
    open[1].goed('geweigerd');
    open[0].goed('verleend');
    await vi.runAllTimersAsync();
    expect(toon).toHaveBeenCalledTimes(1);
    expect(toon).toHaveBeenCalledWith({ antwoord: 'geweigerd' });
  });

  it('geeft een mislukt verzoek als fout, en toont na stop() niets meer', async () => {
    const { vraag, open } = verzoeken();
    const toon = vi.fn();
    const v = volger(vraag, toon);
    v.plan();
    await vi.advanceTimersByTimeAsync(300);
    open[0].fout(new Error('the cell did not load: Failed to fetch'));
    await vi.runAllTimersAsync();
    expect(toon).toHaveBeenCalledWith({ fout: 'the cell did not load: Failed to fetch' });

    v.plan();
    await vi.advanceTimersByTimeAsync(300);
    v.stop();
    open[1].goed('verleend');
    await vi.runAllTimersAsync();
    expect(toon).toHaveBeenCalledTimes(1);
  });
});
