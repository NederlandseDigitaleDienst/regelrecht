import { describe, it, expect } from 'vitest';
import { urenCompact } from './format.js';

// Deze test bestaat om één fout: de eerste versie liep via number(), en die
// rondt af op hele getallen. 1,42 en 0,97 mln lazen daardoor allebei als
// "1 mln uur", en het verschil tussen de kolommen was onzichtbaar terwijl de
// onderliggende waarden wel degelijk verschilden.
describe('urenCompact', () => {
  it('houdt twee decimalen bij miljoenen, zodat kolommen te onderscheiden zijn', () => {
    expect(urenCompact(1424000)).toBe('1,42 mln uur');
    expect(urenCompact(1358000)).toBe('1,36 mln uur');
    expect(urenCompact(1424000)).not.toBe(urenCompact(1358000));
  });

  it('schrijft duizenden zonder loze decimaal', () => {
    expect(urenCompact(970000)).toBe('970 dzd uur');
    expect(urenCompact(66000)).toBe('66 dzd uur');
  });

  it('schrijft kleine aantallen voluit', () => {
    expect(urenCompact(450)).toBe('450 uur');
    expect(urenCompact(0)).toBe('0 uur');
  });

  it('zet geen euroteken bij uren', () => {
    expect(urenCompact(1424000)).not.toContain('€');
  });

  it('geeft een streepje bij een ontbrekende waarde', () => {
    expect(urenCompact(null)).toBe('—');
    expect(urenCompact(undefined)).toBe('—');
    expect(urenCompact(NaN)).toBe('—');
  });
});
