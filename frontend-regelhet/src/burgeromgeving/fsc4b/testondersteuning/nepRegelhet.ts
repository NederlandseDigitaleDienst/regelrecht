// A RegelHet with FSC voor Burgers in test mode, in memory, for the tests of
// the app's client: the demo Stelsel, the test Activatiedienst,
// CreateSession and the citizen's routes, after packages/regelhet/src/fsc4b/.
// It checks what the server checks: the Stelseldocument signed by the
// Stelselbeheerder, the credential bound to the app key, the nonce signed
// for the organization's OIN, the body under the session key. It is a
// fake: what the citizen's routes answer is up to the test (`routes`).
import { CompactEncrypt, CompactSign, compactDecrypt, compactVerify, decodeJwt, exportJWK, exportSPKI, importJWK } from 'jose';
import type { JWK } from 'jose';
import type { Haal } from '../client';

const tekst = new TextEncoder();
const lees = new TextDecoder();

async function sleutelpaar() {
  return crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
}

async function onderteken(sleutel: CryptoKey, claims: unknown): Promise<string> {
  return new CompactSign(tekst.encode(JSON.stringify(claims))).setProtectedHeader({ alg: 'ES256', typ: 'jwt' }).sign(sleutel);
}

async function geverifieerd(jws: string, jwk: JWK): Promise<Record<string, unknown>> {
  const { payload } = await compactVerify(jws, await importJWK(jwk, 'ES256'));
  return JSON.parse(lees.decode(payload)) as Record<string, unknown>;
}

const antwoord = (status: number, body: string) => new Response(body, { status });

export interface NepRegelhet {
  adres: string;
  oin: string;
  haal: Haal;
  /** Per citizen's route what it answers: the status and the JSON, given the decrypted request and the session's BSN. */
  routes: Record<string, (verzoek: Record<string, unknown>, bsn: string) => { status: number; body: unknown }>;
  /** End every session, as fifteen minutes would. */
  sessiesVerlopen(): void;
  /** How many sessions were made. */
  sessies(): number;
  /** Whether the demo is off: its routes answer 404. */
  zonderDemo: boolean;
  /** What `GET /fsc4b/regelwerken` serves. */
  regelwerken: Record<string, string>;
  /** Whether CreateSession refuses every credential, as after a new Stelsel. */
  weigertCredentials: boolean;
  /** How the organization signs: for a test of signed data. */
  ondertekenAlsOrganisatie(claims: unknown): Promise<string>;
}

export async function nepRegelhet(adres = 'https://regelhet.example.org', oin = '00000099000000000001'): Promise<NepRegelhet> {
  const beheerder = await sleutelpaar();
  const organisatie = await sleutelpaar();
  const activatiedienst = await sleutelpaar();
  const beheerderJwk = await exportJWK(beheerder.publicKey);
  const organisatieJwk = await exportJWK(organisatie.publicKey);
  const activatiedienstJwk = await exportJWK(activatiedienst.publicKey);
  const uitdagingen = new Map<string, CryptoKey>();
  const sessies = new Map<string, { sleutel: Uint8Array; bsn: string }>();
  let gemaakt = 0;

  const nep: NepRegelhet = {
    adres,
    oin,
    routes: {},
    zonderDemo: false,
    weigertCredentials: false,
    regelwerken: {},
    sessiesVerlopen: () => sessies.clear(),
    sessies: () => gemaakt,
    ondertekenAlsOrganisatie: (claims) => onderteken(organisatie.privateKey, claims),
    haal: async (url, init) => {
      const pad = url.startsWith(adres) ? url.slice(adres.length) : url;
      const body = typeof init?.body === 'string' ? init.body : '';
      if (pad.startsWith('/fsc4b/demo/') && nep.zonderDemo) return antwoord(404, 'no demo');
      if (pad === '/fsc4b/demo/stelselbeheerder') return antwoord(200, JSON.stringify(beheerderJwk));
      if (pad === '/fsc4b/demo/stelseldocument') {
        const document = { organizations: [{ oin, public_key: organisatieJwk, discovery_url: `${adres}/fsc4b/discovery` }] };
        return antwoord(200, await onderteken(beheerder.privateKey, document));
      }
      if (pad === '/fsc4b/regelwerken') return antwoord(200, JSON.stringify(nep.regelwerken));
      if (pad === '/fsc4b/discovery') {
        return antwoord(200, JSON.stringify({ sessionApi: { v1: `${adres}/fsc4b/session` }, regelhetBurgerApi: { v1: `${adres}/fsc4b/burger` } }));
      }
      if (pad === '/fsc4b/demo/activatiedienst/v1/credential') {
        const { certificate, bsn } = JSON.parse(body) as { certificate: string; bsn: string };
        const zelf = decodeJwt(certificate) as { sub: string; cnf: { jwk: JWK } };
        await geverifieerd(certificate, zelf.cnf.jwk);
        if (zelf.sub !== 'certificate_type_self_signed') return antwoord(400, '{"fout":"no self-signed certificate"}');
        const exp = Math.floor(Date.now() / 1000) + 3600;
        return antwoord(200, await onderteken(activatiedienst.privateKey, { bsn, exp, given_name: 'Test', family_name: 'Burger', cnf: { jwk: zelf.cnf.jwk } }));
      }
      if (pad === '/fsc4b/session/v1/challenge') {
        const efemeer = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']);
        const nonce = crypto.randomUUID();
        uitdagingen.set(nonce, efemeer.privateKey);
        const der = (await exportSPKI(efemeer.publicKey)).replace(/-----[^-]+-----|\s/g, '');
        return antwoord(200, await onderteken(organisatie.privateKey, { sub: 'challenge', nonce, ephemeral_organization_ec_public_key: der }));
      }
      if (pad === '/fsc4b/session/v1/complete') {
        const { nonce, encrypted_challenge_result } = JSON.parse(body) as { nonce: string; encrypted_challenge_result: string };
        const efemeer = uitdagingen.get(nonce);
        uitdagingen.delete(nonce);
        if (!efemeer || nep.weigertCredentials) return antwoord(401, '{"fout":"refused"}');
        try {
          const vp = JSON.parse(lees.decode((await compactDecrypt(encrypted_challenge_result, efemeer)).plaintext)) as Record<string, string>;
          const credential = await geverifieerd(vp.certificate ?? '', activatiedienstJwk);
          const handtekening = await geverifieerd(vp.app_nonce_signature ?? '', (credential.cnf as { jwk: JWK }).jwk);
          if (handtekening.sub !== 'challenge_response' || handtekening.aud !== oin || handtekening.nonce !== nonce) throw new Error('nonce');
          const sleutel = Uint8Array.from(atob(vp.session_aes_key ?? ''), (c) => c.charCodeAt(0));
          const token = crypto.randomUUID().replace(/-/g, '');
          sessies.set(token, { sleutel, bsn: String(credential.bsn) });
          gemaakt += 1;
          return antwoord(200, await new CompactEncrypt(tekst.encode(token)).setProtectedHeader({ alg: 'dir', enc: 'A256GCM' }).encrypt(sleutel));
        } catch {
          return antwoord(401, '{"fout":"refused"}');
        }
      }
      const route = /^\/fsc4b\/burger\/v1\/([a-z]+)$/.exec(pad)?.[1];
      if (route) {
        const token = (init?.headers as Record<string, string> | undefined)?.authorization ?? '';
        const sessie = sessies.get(token);
        if (!sessie) return antwoord(401, '{"fout":"start a session first"}');
        const verzoek = JSON.parse(lees.decode((await compactDecrypt(body, sessie.sleutel)).plaintext)) as Record<string, unknown>;
        const doe = nep.routes[route];
        const uit = doe ? doe(verzoek, sessie.bsn) : { status: 404, body: { fout: 'no such route' } };
        const jwe = await new CompactEncrypt(tekst.encode(JSON.stringify(uit.body))).setProtectedHeader({ alg: 'dir', enc: 'A256GCM' }).encrypt(sessie.sleutel);
        return antwoord(uit.status, jwe);
      }
      return antwoord(404, 'not found');
    },
  };
  return nep;
}
