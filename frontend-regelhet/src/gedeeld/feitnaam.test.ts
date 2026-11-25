import { describe, expect, it } from 'vitest';
import { feitNaam, vastlegNaam } from './feitnaam';

describe('the name of a fact', () => {
  it('reads the fact in words', () => {
    expect(feitNaam('beleid#13#bericht_verzonden')).toBe('Bericht verzonden');
  });

  it('puts the participle first for something to record', () => {
    expect(vastlegNaam('beleid#12#verzoek_tot_aanvulling_verzonden')).toBe('Verzonden verzoek tot aanvulling');
    expect(vastlegNaam('beleid#13#bericht_verzonden')).toBe('Verzonden bericht');
    expect(vastlegNaam('beleid#13#bericht_doorgestuurd')).toBe('Doorgestuurd bericht');
  });

  it('keeps a name of one word', () => {
    expect(vastlegNaam('beleid#1#ontvangen')).toBe('Ontvangen');
  });
});
