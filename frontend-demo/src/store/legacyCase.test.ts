/**
 * Een zaak die al in localStorage stond, van vóór de vertaling.
 *
 * Zulke zaken dragen een kant-en-klare Nederlandse `text` en geen sleutel. Die
 * tekst weggooien zou de geschiedenis van een lopende demo wissen, en vertalen
 * kan niet: de woorden zijn het enige wat ervan over is. Hij blijft dus staan,
 * ook in het Engels, en dat ligt hier vast zodat een latere opruimactie hem
 * niet per ongeluk weghaalt.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { caseReason, eventText } from './demoStore';
import type { DemoCase } from './demoStore';
import { adoptLocale } from '../i18n/index.js';

describe('een zaak van vóór de vertaling', () => {
  afterEach(() => adoptLocale('nl'));

  it('houdt zijn opgeslagen tekst, in beide talen', () => {
    const legacy = { at: '2026-01-01T00:00:00Z', type: 'DECIDED', text: 'Automatisch toegekend.' };
    expect(eventText(legacy)).toBe('Automatisch toegekend.');
    adoptLocale('en');
    expect(eventText(legacy)).toBe('Automatisch toegekend.');
  });

  it('vertaalt een gebeurtenis die wel een sleutel draagt', () => {
    const fresh = { at: 'x', type: 'DECIDED', key: 'case.event.granted_auto' };
    expect(eventText(fresh)).toBe('Automatisch toegekend.');
    adoptLocale('en');
    expect(eventText(fresh)).toBe('Granted automatically.');
  });

  it('doet hetzelfde met de reden onder een zaak', () => {
    expect(caseReason({ reason: 'Oude reden.' })).toBe('Oude reden.');
    expect(caseReason({ reasonKey: 'case.reason.granted_by_law' })).toBe('Automatisch toegekend op basis van de wet.');
    adoptLocale('en');
    expect(caseReason({ reasonKey: 'case.reason.granted_by_law' })).toBe('Granted automatically under the law.');
  });

  it('valt terug op niets in plaats van te breken', () => {
    expect(eventText(null)).toBe('');
    expect(eventText({ type: 'X' })).toBe('');
    expect(caseReason(null)).toBe(null);
    expect(caseReason({})).toBe(null);
  });
});

describe('de reden onder een automatisch besluit', () => {
  afterEach(() => adoptLocale('nl'));

  it('komt uit `reasonKey` en niet uit `reason`', () => {
    // De store schrijft `reasonKey` op het automatische pad en `reason` alleen
    // wanneer een behandelaar beslist. Een scherm dat `c.reason` leest toont
    // dus niets voor elke zaak die de wet zelf afdeed, en dat is de
    // standaardstroom van de demo. Geen foutmelding, alleen een lege regel.
    const auto: Pick<DemoCase, 'approved' | 'reasonKey' | 'reason'> = { approved: true, reasonKey: 'case.reason.granted_by_law' };
    expect(auto.reason).toBeUndefined();
    expect(caseReason(auto)).toBe('Automatisch toegekend op basis van de wet.');
    adoptLocale('en');
    expect(caseReason(auto)).toBe('Granted automatically under the law.');
  });

  it('laat een reden van een behandelaar staan zoals hij is', () => {
    // Die is ingetypt door een mens en hoort niet vertaald te worden.
    expect(caseReason({ reason: 'Onvoldoende onderbouwd.' })).toBe('Onvoldoende onderbouwd.');
  });
});
