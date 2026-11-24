// A submission waits on the request made for it, and on an open request made
// for an earlier submission of the same person; not on anyone else's.
import { describe, expect, it } from 'vitest';
import type { Gram } from '../gedeeld/types';
import type { EigenVerzoek } from './api';
import { verzoekenBij } from './verzoek';

const aanvraag = (id: string, bsn: string): Gram => ({
  id,
  kind: 'AANVRAAG',
  basis: 'wet#1',
  recorded_on: '2026-10-13',
  recording_actor: 'gemeente',
  fields: { bsn_aanvrager: bsn, kaart: true },
});
const verzoek = (voor: string, bsn: string, open: boolean): EigenVerzoek => ({
  verzoek: {
    id: `v-${voor}`,
    kind: 'EXECUTOGRAM',
    basis: 'beleid#9',
    fact: 'onderzoek_aangevraagd',
    recorded_on: '2026-10-13',
    recording_actor: 'gemeente',
    fields: { bsn_aanvrager: bsn, datum_verzoek: '2026-10-13', aanvraag: voor },
  },
  aan: 'ggd',
  aan_naam: 'GGD Voorbeeld',
  open,
  antwoord_ontvangen: null,
});

describe('the requests a submission waits on', () => {
  it('are the one made for it, and an open one for the same person', () => {
    const alle = [verzoek('a1', '999990020', true), verzoek('x', '999990032', true)];
    expect(verzoekenBij(aanvraag('a1', '999990020'), alle).map((v) => [v.verzoek.id, v.eerder])).toEqual([['v-a1', false]]);
    // A second application of the same person waits on the first request.
    expect(verzoekenBij(aanvraag('a2', '999990020'), alle).map((v) => [v.verzoek.id, v.eerder])).toEqual([['v-a1', true]]);
  });

  it('leave out an answered request of an earlier submission', () => {
    expect(verzoekenBij(aanvraag('a2', '999990020'), [verzoek('a1', '999990020', false)])).toEqual([]);
  });
});
