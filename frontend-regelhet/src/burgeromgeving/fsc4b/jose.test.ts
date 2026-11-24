// @vitest-environment node
// The JOSE formats of the app against fixed vectors and round trips with
// test keys made here. The vectors are those packages/fsc4b/src/jose.rs
// checks the server against, made with panva/jose: the library this app uses
// and the Rust server read each other's tokens.
import { CompactEncrypt, compactDecrypt, exportSPKI } from 'jose';
import { describe, expect, it } from 'vitest';
import { naarBase64, nieuweSessiesleutel, nieuweSleutel, onderteken, ontsleutel, onverifieerd, publiekeJwk, uitBase64, verifieer, versleutel, versleutelNaarDer } from './jose';

const b64url = (s: string) => btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

describe('the JOSE formats of the app', () => {
  it('decrypts the dir vector of jose.rs under the key 0, 1, ..., 31', async () => {
    const jwe = 'eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..pJNbCxUHtvUfMwnJ.xkxscLpKqlKicNRY.VHrdMwCA0-0k_7j0JA0xiw';
    const sleutel = Uint8Array.from({ length: 32 }, (_, i) => i);
    expect(await ontsleutel(sleutel, jwe)).toBe('{"dir":true}');
  });

  it('verifies the ES256 vector of jose.rs under its public key', async () => {
    const jws = 'eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJ4In0.8YphPVp0HC167dMVYuvTaFcf5XuSBBXJa9ae-5yHa_7CIV2v0TbJFb8JHB84YHeF0RSx-Cd1uM0qoKRFQ9R31Q';
    const jwk = { kty: 'EC', crv: 'P-256', x: 'CwwzmkN7xLORvytlmxGT80wLhuaJlzULkWLQP81Xj0o', y: 'XGFCJMYwzxVVoX_Yb-ueiG2sj-NGlRqS8L47wMp9sdQ' };
    expect(await verifieer(jws, jwk)).toEqual({ sub: 'x' });
  });

  it('keeps the private half of the app key out of reach', async () => {
    const paar = await nieuweSleutel();
    expect(paar.privateKey.extractable).toBe(false);
    await expect(crypto.subtle.exportKey('jwk', paar.privateKey)).rejects.toThrow();
    expect(Object.keys(await publiekeJwk(paar)).sort()).toEqual(['crv', 'kty', 'x', 'y']);
  });

  it('signs with the app key so that only its public key verifies', async () => {
    const paar = await nieuweSleutel();
    const ander = await nieuweSleutel();
    const jws = await onderteken(paar.privateKey, { sub: 'challenge_response', nonce: 'n' });
    expect(await verifieer(jws, await publiekeJwk(paar))).toEqual({ sub: 'challenge_response', nonce: 'n' });
    expect(onverifieerd(jws)).toEqual({ sub: 'challenge_response', nonce: 'n' });
    await expect(verifieer(jws, await publiekeJwk(ander))).rejects.toThrow();
  });

  it('refuses a token that asks for another algorithm or a crit header', async () => {
    const paar = await nieuweSleutel();
    const jwk = await publiekeJwk(paar);
    const claims = b64url('{"sub":"x"}');
    await expect(verifieer(`${b64url('{"alg":"none"}')}.${claims}.`, jwk)).rejects.toThrow();
    await expect(verifieer(`${b64url('{"alg":"ES256","crit":["exp"]}')}.${claims}.AAAA`, jwk)).rejects.toThrow(/crit/);
    const sleutel = nieuweSessiesleutel();
    const a128 = await new CompactEncrypt(new TextEncoder().encode('x'))
      .setProtectedHeader({ alg: 'dir', enc: 'A128GCM' })
      .encrypt(sleutel.slice(0, 16));
    await expect(ontsleutel(sleutel, a128)).rejects.toThrow();
  });

  it('encrypts under the session key so that only that key opens it, untampered', async () => {
    const sleutel = nieuweSessiesleutel();
    const jwe = await versleutel(sleutel, '{"casus":true}');
    expect(await ontsleutel(sleutel, jwe)).toBe('{"casus":true}');
    await expect(ontsleutel(nieuweSessiesleutel(), jwe)).rejects.toThrow();
    const delen = jwe.split('.');
    const anders = delen[3]?.startsWith('A') ? `B${delen[3].slice(1)}` : `A${delen[3]?.slice(1)}`;
    await expect(ontsleutel(sleutel, [delen[0], delen[1], delen[2], anders, delen[4]].join('.'))).rejects.toThrow();
  });

  it('encrypts the presentation to an ephemeral key given as base64 DER, as the challenge carries it', async () => {
    const efemeer = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']);
    const pem = await exportSPKI(efemeer.publicKey);
    const der = pem.replace(/-----[^-]+-----|\s/g, '');
    const jwe = await versleutelNaarDer(der, '{"vp":1}');
    const { plaintext, protectedHeader } = await compactDecrypt(jwe, efemeer.privateKey);
    expect(protectedHeader.alg).toBe('ECDH-ES');
    expect(protectedHeader.enc).toBe('A256GCM');
    expect(new TextDecoder().decode(plaintext)).toBe('{"vp":1}');
  });

  it('writes the session key in standard base64, as the server reads it', () => {
    const sleutel = Uint8Array.from({ length: 32 }, (_, i) => 255 - i);
    expect(uitBase64(naarBase64(sleutel))).toEqual(sleutel);
    expect(naarBase64(sleutel)).toMatch(/^[A-Za-z0-9+/]+=*$/);
  });
});
