import { describe, expect, it } from 'vitest';
import { CHANGE_TYPES, changeTypeById, claimsFromAnswers, eventUnsupported, fieldsOf, typeLabel } from './changeWizard';
import type { ChangeType } from './changeWizard';

/** A change type that has to exist: the test fails here, not on a null further down. */
function existingType(id: string): ChangeType {
  const type = changeTypeById(id);
  if (!type) throw new Error(`no change type ${id}`);
  return type;
}

describe('CHANGE_TYPES', () => {
  it('wijst elke soort wijziging aan de wet die het gegeven bezit', () => {
    expect(CHANGE_TYPES.map((t) => [t.id, t.law])).toEqual([
      ['inkomen', 'wet_inkomstenbelasting'],
      ['huurprijs', 'wet_op_de_huurtoeslag'],
      ['woonadres', 'wet_brp'],
      ['huishouden', 'wet_brp'],
    ]);
  });

  it('vindt een soort op id en geeft niets terug voor een onbekende', () => {
    expect(typeLabel(changeTypeById('inkomen'))).toBe('Mijn inkomen of vermogen');
    expect(changeTypeById('bestaat-niet')).toBeNull();
  });
});

describe('claimsFromAnswers voor bedragen', () => {
  const inkomen = changeTypeById('inkomen');

  it('rekent hele euro’s om naar de centen die het register gebruikt', () => {
    const claims = claimsFromAnswers(inkomen, { loon_uit_dienstbetrekking: '32000' });
    expect(claims).toEqual([
      { law: 'wet_inkomstenbelasting', input: 'loon_uit_dienstbetrekking', value: 3200000, label: 'Loon uit dienstbetrekking' },
    ]);
  });

  it('laat een leeg veld ongemoeid: leeg is ongewijzigd, niet nul', () => {
    const claims = claimsFromAnswers(inkomen, { loon_uit_dienstbetrekking: '', spaargeld: '  ', beleggingen: null });
    expect(claims).toEqual([]);
  });

  it('neemt een uitdrukkelijke nul wél mee', () => {
    const claims = claimsFromAnswers(inkomen, { winst_uit_onderneming: '0' });
    expect(claims).toEqual([
      { law: 'wet_inkomstenbelasting', input: 'winst_uit_onderneming', value: 0, label: 'Winst uit onderneming' },
    ]);
  });

  it('leest een bedrag met een komma als decimaalteken', () => {
    const claims = claimsFromAnswers(inkomen, { spaargeld: '1234,56' });
    expect(claims[0]?.value).toBe(123456);
  });

  it('dient meerdere gewijzigde bedragen samen in', () => {
    const claims = claimsFromAnswers(inkomen, { loon_uit_dienstbetrekking: '30000', schulden: '5000' });
    expect(claims.map((c) => c.input)).toEqual(['loon_uit_dienstbetrekking', 'schulden']);
  });

  it('laat een onleesbaar bedrag weg in plaats van NaN op te sturen', () => {
    expect(claimsFromAnswers(inkomen, { spaargeld: 'veel' })).toEqual([]);
  });

  it('dekt alle velden die de Wet inkomstenbelasting uit de registers leest', () => {
    expect(fieldsOf(inkomen).map((f) => f.name)).toEqual([
      'loon_uit_dienstbetrekking',
      'uitkeringen_en_pensioenen',
      'winst_uit_onderneming',
      'resultaat_overige_werkzaamheden',
      'eigen_woning',
      'reguliere_voordelen',
      'vervreemdingsvoordelen',
      'spaargeld',
      'beleggingen',
      'onroerend_goed',
      'schulden',
    ]);
  });
});

describe('claimsFromAnswers voor de huur', () => {
  it('stuurt de huur naar de Wet op de huurtoeslag, in centen', () => {
    const claims = claimsFromAnswers(changeTypeById('huurprijs'), { huurprijs: '850', servicekosten: '48' });
    expect(claims).toEqual([
      { law: 'wet_op_de_huurtoeslag', input: 'huurprijs', value: 85000, label: 'Kale huurprijs per maand' },
      { law: 'wet_op_de_huurtoeslag', input: 'servicekosten', value: 4800, label: 'Servicekosten per maand' },
    ]);
  });
});

describe('claimsFromAnswers voor een adres', () => {
  const woonadres = changeTypeById('woonadres');

  it('maakt van de losse velden één adres', () => {
    const claims = claimsFromAnswers(woonadres, {
      straat: 'Kalverstraat',
      huisnummer: '1',
      postcode: '1012 NX',
      woonplaats: 'Amsterdam',
    });
    expect(claims).toEqual([
      {
        law: 'wet_brp',
        input: 'adres',
        value: { straat: 'Kalverstraat', huisnummer: '1', postcode: '1012 NX', woonplaats: 'Amsterdam', type: 'WOONADRES' },
        label: 'Adres',
      },
    ]);
  });

  it('dient alleen de ingevulde adresvelden in', () => {
    const claims = claimsFromAnswers(woonadres, { straat: 'Kalverstraat' });
    expect(claims).toHaveLength(1);
    expect(claims[0]?.input).toBe('adres');
    expect(claims[0]?.value).toEqual({ straat: 'Kalverstraat', type: 'WOONADRES' });
  });

  it('dient niets in als er geen enkel adresveld is ingevuld', () => {
    expect(claimsFromAnswers(woonadres, { straat: '', woonplaats: '  ' })).toEqual([]);
  });
});

describe('claimsFromAnswers voor het huishouden', () => {
  const huishouden = existingType('huishouden');

  it('zet een scheiding om in het beëindigen van het partnerschap', () => {
    const claims = claimsFromAnswers(huishouden, { event: 'scheiden' });
    expect(claims).toEqual([
      { law: 'wet_brp', input: 'partnerschap_type', value: 'GEEN', label: 'Ik ga scheiden of wij gaan uit elkaar' },
      { law: 'wet_brp', input: 'partner_bsn', value: null, label: 'Ik ga scheiden of wij gaan uit elkaar' },
    ]);
  });

  it('dient niets in voor een gebeurtenis die de demo nog niet kan', () => {
    for (const event of ['samenwonen', 'kind', 'iemand-bij', 'iemand-weg', 'overlijden']) {
      expect(claimsFromAnswers(huishouden, { event })).toEqual([]);
    }
  });

  it('houdt een niet-ondersteunde gebeurtenis tegen, ook als zij wél wijzigingen zou kennen', () => {
    // De vorige test bewijst dit niet: geen enkele echte gebeurtenis heeft
    // `unsupported` én `changes`, dus daar houdt het ontbreken van `changes`
    // de melding al tegen. Hier wel allebei, zodat de `unsupported`-toets
    // zelf de doorslag geeft — anders zou een half afgebouwde gebeurtenis
    // stilletjes correcties indienen.
    const halfaf = {
      ...huishouden,
      events: [{ value: 'halfaf', labelKey: 'sheet.change.event.kind', icon: 'person-badge-plus', unsupportedKey: 'sheet.change.event.kind.unsupported', changes: { partnerschap_type: 'GEEN' } }],
    };
    expect(claimsFromAnswers(halfaf, { event: 'halfaf' })).toEqual([]);
  });

  it('zegt per niet-ondersteunde gebeurtenis waaróm het niet kan', () => {
    const events = huishouden.events ?? [];
    expect(events.length).toBeGreaterThan(0);
    for (const e of events.filter((e) => !e.changes)) {
      expect(eventUnsupported(e)).toMatch(/kan in deze demo nog niet|demo nog niet/);
    }
  });

  it('dient niets in zonder gekozen gebeurtenis', () => {
    expect(claimsFromAnswers(huishouden, {})).toEqual([]);
  });
});

describe('claimsFromAnswers zonder soort', () => {
  it('geeft een lege lijst terug', () => {
    expect(claimsFromAnswers(null, { van: 'alles' })).toEqual([]);
  });
});
