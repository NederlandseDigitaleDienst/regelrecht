// @vitest-environment node
// The signed source data as the app reads them, and the hash of the
// regelwerken it checks them against before it computes anything.
import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { leesConfig } from './config';
import { bundelHash, leesGegevens } from './gegevens';
import { Burgerapp } from './kern';
import { nieuweSleutel, onderteken, publiekeJwk } from './jose';
import { geheugenOpslag } from './opslag';
import { nepRegelhet } from './testondersteuning/nepRegelhet';
import { bereidVoor } from './voorspelling';

const OIN = '00000099000000000001';
const BSN = '999990032';

function gegevens(extra: Record<string, unknown> = {}) {
  return {
    iss: OIN,
    sub: BSN,
    iat: 1,
    peildatum: '2026-10-13',
    casus: 'gehandicaptenparkeerkaart',
    cel: 'gemeente_wilgendaal',
    regelwerken: [],
    bundel_hash: 'sha256:00',
    antwoorden: {},
    records: [],
    bij_aanvraag: [],
    ...extra,
  };
}

describe('the hash of the regelwerken', () => {
  it('is the SHA-256 of each sorted path and text, with their lengths in bytes', async () => {
    const sha = (s: string) => `sha256:${createHash('sha256').update(Buffer.from(s, 'utf8')).digest('hex')}`;
    expect(await bundelHash({ 'a.yaml': 'x' })).toBe(sha('6:a.yaml1:x'));
    // Sorted by path, and a length in bytes: "é" is two.
    expect(await bundelHash({ 'b.yaml': 'é', 'a.yaml': '' })).toBe(sha('6:a.yaml0:6:b.yaml2:é'));
    expect(await bundelHash({})).toBe(sha(''));
  });
});

describe('the signed source data', () => {
  it('are read from the signature, issued by the organization, about the citizen', async () => {
    const paar = await nieuweSleutel();
    const jwk = await publiekeJwk(paar);
    const g = gegevens();
    const ondertekend = await onderteken(paar.privateKey, g);
    expect(await leesGegevens({ ondertekend, gegevens: g }, jwk, OIN, BSN)).toEqual(g);
  });

  it('are refused under another key, from another issuer, about someone else, or unlike the copy beside them', async () => {
    const paar = await nieuweSleutel();
    const ander = await publiekeJwk(await nieuweSleutel());
    const jwk = await publiekeJwk(paar);
    const g = gegevens();
    const ondertekend = await onderteken(paar.privateKey, g);
    await expect(leesGegevens({ ondertekend }, ander, OIN, BSN)).rejects.toThrow(/handtekening/);
    await expect(leesGegevens({ ondertekend }, jwk, '123', BSN)).rejects.toThrow(/komen van/);
    await expect(leesGegevens({ ondertekend }, jwk, OIN, '999990020')).rejects.toThrow(/iemand anders/);
    await expect(leesGegevens({ ondertekend, gegevens: { ...g, sub: 'x' } }, jwk, OIN, BSN)).rejects.toThrow(/wijken af/);
    const zonderHash = await onderteken(paar.privateKey, { ...g, bundel_hash: undefined });
    await expect(leesGegevens({ ondertekend: zonderHash }, jwk, OIN, BSN)).rejects.toThrow(/missen/);
  });

  it('stop the prediction when the regelwerken fetched are not the ones signed', async () => {
    const server = await nepRegelhet();
    server.regelwerken = { 'casussen/gehandicaptenparkeerkaart.yaml': 'cel: gemeente_wilgendaal\n' };
    server.routes.gegevens = () => ({ status: 200, body: {} });
    const app = new Burgerapp(leesConfig(JSON.stringify({ regelhets: [{ id: 'p', naam: 'P', adres: server.adres }] })), geheugenOpslag(), server.haal);
    await app.activeren('p', BSN);
    const g = gegevens({ bundel_hash: await bundelHash({ ...server.regelwerken, 'extra.yaml': 'x' }) });
    const ondertekend = await server.ondertekenAlsOrganisatie(g);
    server.routes.gegevens = () => ({ status: 200, body: { ondertekend, gegevens: g } });
    const maak = () => {
      throw new Error('the kern must not load');
    };
    const vorm = { soort: 'AANVRAAG', basis: 'x#1', velden: [], besluit: 'x#1', cel: 'c' };
    await expect(bereidVoor(app, 'p', maak, vorm)).rejects.toThrow(/niet de regels die de regeling ondertekende/);
  });
});
