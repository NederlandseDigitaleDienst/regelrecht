// What an app does to reach one RegelHet over FSC voor Burgers, step by
// step, after the reference client in packages/fsc4b/src/client.rs; the
// names of the wire are those of packages/fsc4b/src/wire.rs.
//
// 1. keep a P-256 key pair (`nieuweSleutel` in jose.ts, kept in opslag.ts);
// 2. get a credential for it from the test Activatiedienst of the demo
//    Stelsel (`demoCredential`);
// 3. read the Stelseldocument, check it under the Stelselbeheerder's key, and
//    find the organization's key and services (`ontdek`);
// 4. run CreateSession (`maakSessie`): a challenge checked against the
//    organization's key, the nonce signed, the presentation encrypted to the
//    ephemeral key, the session token decrypted;
// 5. call the citizen's service under the session key (`roep`).
import { naarBase64, nieuweSessiesleutel, onderteken, ontsleutel, publiekeJwk, verifieer, versleutel, versleutelNaarDer } from './jose';
import type { PubliekeJwk } from './jose';

export const APP_MANAGER_CERTIFICATE = 'app_manager_jwt_certificate';
export const SELF_SIGNED_CERTIFICATE = 'certificate_type_self_signed';
export const CHALLENGE_RESPONSE = 'challenge_response';
export const CHALLENGE_SUB = 'challenge';
export const SESSION_SERVICE = 'sessionApi';
export const CITIZEN_SERVICE = 'regelhetBurgerApi';
export const IDEMPOTENCY_MEMBER = 'idempotency_key';
export const DEMO_STELSELDOCUMENT = '/fsc4b/demo/stelseldocument';
export const DEMO_STELSELBEHEERDER = '/fsc4b/demo/stelselbeheerder';
export const DEMO_CREDENTIAL = '/fsc4b/demo/activatiedienst/v1/credential';
export const DEMO_ORGANIZATION_OIN = '00000099000000000001';
/** The made-up citizens of the demo, from the test range of BSNs. */
export const TEST_CITIZENS = ['999990032', '999990020'] as const;

/** How the client reaches a server: the browser's `fetch`, or a fake one in a test. */
export type Haal = (url: string, init?: RequestInit) => Promise<Response>;

/** Why a step failed: the network, a status of the server, or an answer that breaks the protocol. */
export class ClientFout extends Error {
  constructor(
    readonly soort: 'netwerk' | 'status' | 'protocol',
    bericht: string,
    readonly status?: number,
    readonly body?: string,
  ) {
    super(bericht);
    this.name = 'ClientFout';
  }
}

async function stuur(haal: Haal, url: string, init?: RequestInit): Promise<Response> {
  try {
    return await haal(url, init);
  } catch (e) {
    throw new ClientFout('netwerk', `${url}: ${e instanceof Error ? e.message : String(e)}`);
  }
}

async function okTekst(haal: Haal, url: string, init?: RequestInit): Promise<string> {
  const antwoord = await stuur(haal, url, init);
  const body = await antwoord.text();
  if (!antwoord.ok) throw new ClientFout('status', `${url} answered ${antwoord.status}`, antwoord.status, body);
  return body;
}

const protocol = (bericht: string) => new ClientFout('protocol', bericht);

/** The self-signed certificate (§4.3.4.1) of the app key: what the test Activatiedienst takes as proof of holding it. */
export async function zelfOndertekend(paar: CryptoKeyPair): Promise<string> {
  return onderteken(paar.privateKey, { sub: SELF_SIGNED_CERTIFICATE, cnf: { jwk: await publiekeJwk(paar) } });
}

/** A credential for the app key and the test BSN `bsn` from the test Activatiedienst at `adres`. */
export async function demoCredential(haal: Haal, adres: string, paar: CryptoKeyPair, bsn: string): Promise<string> {
  const body = JSON.stringify({ certificate: await zelfOndertekend(paar), bsn });
  return (await okTekst(haal, `${adres}${DEMO_CREDENTIAL}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body })).trim();
}

/** What the app needs to know of one organization: its OIN, its key, and where its two services are. */
export interface Adresboek {
  oin: string;
  organisatiesleutel: PubliekeJwk;
  sessieApi: string;
  burgerApi: string;
}

/**
 * The organization with OIN `oin` in the demo Stelsel at `adres`. The demo
 * serves the Stelselbeheerder's key itself, so trusting it is trusting the
 * server; a real app has that key built in.
 */
export async function ontdek(haal: Haal, adres: string, oin: string): Promise<Adresboek> {
  const beheerder: unknown = JSON.parse(await okTekst(haal, `${adres}${DEMO_STELSELBEHEERDER}`));
  const document = await okTekst(haal, `${adres}${DEMO_STELSELDOCUMENT}`);
  const claims = await verifieer(document, beheerder as PubliekeJwk);
  const organisaties = Array.isArray(claims.organizations) ? (claims.organizations as Record<string, unknown>[]) : [];
  const organisatie = organisaties.find((o) => o.oin === oin);
  if (!organisatie) throw protocol(`the Stelseldocument does not list ${oin}`);
  if (typeof organisatie.discovery_url !== 'string') throw protocol('no discovery_url');
  const ontdekking = JSON.parse(await okTekst(haal, organisatie.discovery_url)) as Record<string, Record<string, unknown>>;
  const dienst = (naam: string): string => {
    const url = ontdekking[naam]?.v1;
    if (typeof url !== 'string') throw protocol(`the discovery configuration lacks ${naam} v1`);
    return url;
  };
  return {
    oin,
    organisatiesleutel: organisatie.public_key as PubliekeJwk,
    sessieApi: dienst(SESSION_SERVICE),
    burgerApi: dienst(CITIZEN_SERVICE),
  };
}

/** A challenge, checked against the organization's key. */
export interface Uitdaging {
  nonce: string;
  efemeerDer: string;
}

export async function uitdaging(haal: Haal, boek: Adresboek): Promise<Uitdaging> {
  const jws = await okTekst(haal, `${boek.sessieApi}/v1/challenge`, { method: 'POST' });
  const claims = await verifieer(jws, boek.organisatiesleutel);
  if (claims.sub !== CHALLENGE_SUB) throw protocol('the challenge has the wrong `sub`');
  if (typeof claims.nonce !== 'string') throw protocol('the challenge has no nonce');
  const der = claims.ephemeral_organization_ec_public_key;
  if (typeof der !== 'string') throw protocol('the challenge has no ephemeral key');
  return { nonce: claims.nonce, efemeerDer: der };
}

/** The presentation of `credential` for `u`, signed for `aud`, with the session key, encrypted to the challenge's ephemeral key. */
export async function presentatie(
  paar: CryptoKeyPair,
  u: Uitdaging,
  credential: string,
  aud: string,
  sessiesleutel: Uint8Array,
): Promise<string> {
  const vp = {
    app_nonce_signature: await onderteken(paar.privateKey, { sub: CHALLENGE_RESPONSE, aud, nonce: u.nonce }),
    certificate_type: APP_MANAGER_CERTIFICATE,
    certificate: credential,
    session_aes_key: naarBase64(sessiesleutel),
  };
  return versleutelNaarDer(u.efemeerDer, JSON.stringify(vp));
}

/** A session: its token, its key, and where the citizen's service is. It lives in memory only. */
export interface Sessie {
  token: string;
  sleutel: Uint8Array;
  burgerApi: string;
}

/** CreateSession from start to end for the app key `paar` with `credential`. */
export async function maakSessie(haal: Haal, boek: Adresboek, paar: CryptoKeyPair, credential: string): Promise<Sessie> {
  const u = await uitdaging(haal, boek);
  const sleutel = nieuweSessiesleutel();
  const jwe = await presentatie(paar, u, credential, boek.oin, sleutel);
  const versleuteld = await okTekst(haal, `${boek.sessieApi}/v1/complete`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ nonce: u.nonce, encrypted_challenge_result: jwe }),
  });
  let token: string;
  try {
    token = await ontsleutel(sleutel, versleuteld);
  } catch {
    throw protocol('the session token does not decrypt under the session key');
  }
  return { token, sleutel, burgerApi: boek.burgerApi };
}

/**
 * `payload` to the citizen's route `/v1/{op}`, encrypted under the session
 * key, with the idempotency key inside it; the status and the decrypted
 * answer. Only a 401 and a 500 may come unencrypted: every other answer must
 * decrypt, or it did not come from the server that holds the session.
 */
export async function roep(
  haal: Haal,
  sessie: Sessie,
  op: string,
  payload: Record<string, unknown>,
  idempotentiesleutel?: string,
): Promise<{ status: number; waarde: unknown }> {
  const inhoud = idempotentiesleutel ? { ...payload, [IDEMPOTENCY_MEMBER]: idempotentiesleutel } : payload;
  const antwoord = await stuur(haal, `${sessie.burgerApi}/v1/${op}`, {
    method: 'POST',
    headers: { authorization: sessie.token, 'content-type': 'application/jose' },
    body: await versleutel(sessie.sleutel, JSON.stringify(inhoud)),
  });
  const tekst = await antwoord.text();
  let platte: string;
  try {
    platte = await ontsleutel(sessie.sleutel, tekst);
  } catch {
    // A 401 and a 500 of the RegelHet come unencrypted (citizen.rs); so does
    // an error page of a proxy in front of it, which is no compact JWE.
    if (antwoord.status === 401 || antwoord.status === 500 || (antwoord.status >= 500 && tekst.trim().split('.').length !== 5)) {
      let waarde: unknown = tekst;
      try {
        waarde = JSON.parse(tekst);
      } catch {
        // Kept as text.
      }
      return { status: antwoord.status, waarde };
    }
    throw protocol(`the answer of ${op} does not decrypt under the session key`);
  }
  try {
    return { status: antwoord.status, waarde: JSON.parse(platte) as unknown };
  } catch {
    throw protocol(`the answer of ${op} decrypts to no JSON`);
  }
}
