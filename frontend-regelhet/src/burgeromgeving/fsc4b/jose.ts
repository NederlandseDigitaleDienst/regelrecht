// The JOSE formats of FSC voor Burgers in the browser, the counterpart of
// packages/fsc4b/src/jose.rs: ES256 signatures, ECDH-ES to the challenge's
// ephemeral key, and `dir` with A256GCM under the session key. WebCrypto
// does the cryptography; the `jose` library puts it in the compact forms.
//
// What is accepted is as narrow as on the server: a token with another
// `alg` or `enc`, or with a `crit` or `zip` header, is refused before a key
// touches it.
import {
  CompactEncrypt,
  CompactSign,
  calculateJwkThumbprint,
  compactDecrypt,
  compactVerify,
  decodeProtectedHeader,
  exportJWK,
  importJWK,
  importSPKI,
} from 'jose';
import type { JWK } from 'jose';

export const JWS_ALG = 'ES256';
export const JWE_ENC = 'A256GCM';
/** The length of a session key for A256GCM, in bytes. */
export const AES_KEY_LEN = 32;

/** A public P-256 key as a JWK: its curve and coordinates. */
export interface PubliekeJwk {
  kty: 'EC';
  crv: 'P-256';
  x: string;
  y: string;
}

const tekst = new TextEncoder();
const lees = new TextDecoder();

/** The bytes of standard base64 with padding, as the protocol writes the session key and the ephemeral key. */
export function uitBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}

export function naarBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes));
}

/**
 * A new ES256 key pair whose private half WebCrypto never hands out
 * (`extractable: false`): the app can sign with it, and nothing can read
 * it, not even this code. It can be kept in IndexedDB as it is.
 */
export function nieuweSleutel(): Promise<CryptoKeyPair> {
  return crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign', 'verify']);
}

/** The public half of `paar` as a JWK, with only the members a P-256 key has. */
export async function publiekeJwk(paar: CryptoKeyPair): Promise<PubliekeJwk> {
  const jwk = await exportJWK(paar.publicKey);
  if (jwk.kty !== 'EC' || jwk.crv !== 'P-256' || !jwk.x || !jwk.y) throw new Error('the app key is no P-256 key');
  return { kty: 'EC', crv: 'P-256', x: jwk.x, y: jwk.y };
}

/** The RFC 7638 thumbprint (SHA-256) of a key, as the server names an app by it. */
export const duimafdruk = (jwk: JWK): Promise<string> => calculateJwkThumbprint(jwk, 'sha256');

/** A compact JWS of `claims` with `typ: jwt` and `alg: ES256`, as `sign_jws` makes it. */
export function onderteken(sleutel: CryptoKey, claims: Record<string, unknown>): Promise<string> {
  return new CompactSign(tekst.encode(JSON.stringify(claims)))
    .setProtectedHeader({ typ: 'jwt', alg: JWS_ALG })
    .sign(sleutel);
}

function weigerKopRegels(kop: Record<string, unknown>): void {
  if ('crit' in kop) throw new Error('a token with a `crit` header is refused');
  if ('zip' in kop) throw new Error('a token with a `zip` header is refused');
}

/** The claims of `jws` when the key `jwk` signed it with ES256; anything else throws. */
export async function verifieer(jws: string, jwk: JWK): Promise<Record<string, unknown>> {
  weigerKopRegels(decodeProtectedHeader(jws));
  // As jose.rs: a key marked for another use or algorithm does not verify.
  if ((jwk.use && jwk.use !== 'sig') || (jwk.alg && jwk.alg !== JWS_ALG && jwk.alg !== 'ECDH-ES')) {
    throw new Error('the key is not one for ES256 signatures');
  }
  const sleutel = await importJWK({ kty: jwk.kty, crv: jwk.crv, x: jwk.x, y: jwk.y }, JWS_ALG);
  const { payload } = await compactVerify(jws.trim(), sleutel, { algorithms: [JWS_ALG] });
  return claims(payload);
}

/**
 * The claims of `jws` without checking its signature: only to read what the
 * app itself holds, such as the end of its credential, which the server
 * checks on every session.
 */
export function onverifieerd(jws: string): Record<string, unknown> {
  const delen = jws.trim().split('.');
  if (delen.length !== 3 || !delen[1]) throw new Error('a JWS has three segments');
  const b64 = delen[1].replace(/-/g, '+').replace(/_/g, '/');
  return claims(uitBase64(b64 + '='.repeat((4 - (b64.length % 4)) % 4)));
}

function claims(bytes: Uint8Array): Record<string, unknown> {
  const waarde: unknown = JSON.parse(lees.decode(bytes));
  if (!waarde || typeof waarde !== 'object' || Array.isArray(waarde)) throw new Error('the claims are no JSON object');
  return waarde as Record<string, unknown>;
}

/** A P-256 public key as base64 DER (SubjectPublicKeyInfo), as the challenge carries it, in PEM. */
function alsPem(derB64: string): string {
  const regels = derB64.match(/.{1,64}/g) ?? [];
  return `-----BEGIN PUBLIC KEY-----\n${regels.join('\n')}\n-----END PUBLIC KEY-----`;
}

/** `tekst` as a compact JWE to the key in `derB64`, with ECDH-ES and A256GCM (§4.3.4.2.2.7). */
export async function versleutelNaarDer(derB64: string, inhoud: string): Promise<string> {
  const sleutel = await importSPKI(alsPem(derB64), 'ECDH-ES');
  return new CompactEncrypt(tekst.encode(inhoud)).setProtectedHeader({ alg: 'ECDH-ES', enc: JWE_ENC }).encrypt(sleutel);
}

/** A fresh session key: 32 bytes from the browser's CSPRNG. */
export const nieuweSessiesleutel = (): Uint8Array => crypto.getRandomValues(new Uint8Array(AES_KEY_LEN));

/** `inhoud` as a compact JWE under the session key, `alg: dir`. */
export function versleutel(sleutel: Uint8Array, inhoud: string): Promise<string> {
  return new CompactEncrypt(tekst.encode(inhoud)).setProtectedHeader({ alg: 'dir', enc: JWE_ENC }).encrypt(sleutel);
}

/** The text of a compact JWE made under the session key, `alg: dir`; anything else throws. */
export async function ontsleutel(sleutel: Uint8Array, jwe: string): Promise<string> {
  weigerKopRegels(decodeProtectedHeader(jwe.trim()));
  const { plaintext } = await compactDecrypt(jwe.trim(), sleutel, {
    keyManagementAlgorithms: ['dir'],
    contentEncryptionAlgorithms: [JWE_ENC],
  });
  return lees.decode(plaintext);
}
