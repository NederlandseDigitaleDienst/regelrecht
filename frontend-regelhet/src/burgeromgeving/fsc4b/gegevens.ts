// The citizen's source data from the RegelHet: `POST /fsc4b/burger/v1/gegevens`,
// signed by the organization. What the executor knows about the citizen
// and uses for this case: the answers of its cell for the citizen's BSN,
// the dossier records those answers rest on, and the facts that come only
// after an application (`bij_aanvraag`).
//
// The answer is read from the signature, not beside it: the claims of the
// JWS, checked under the organization's key from the Stelseldocument, are
// what the app uses. The unsigned copy beside it must say the same, or the
// app refuses both.

import { verifieer } from './jose';
import type { PubliekeJwk } from './jose';

/**
 * Where a cell's answer comes from: the cell, its answer, and the reduction
 * article and version behind it. Without how far the cell read its chronicle,
 * which would name records of other people.
 */
export interface Herkomst {
  door: string;
  antwoord: string;
  reductie: string;
  artikel: string;
  geldig_vanaf: string;
}

/** One answer of the executor's cell. `waarde: null` means the cell is silent, not `false`. */
export interface Antwoord {
  waarde: unknown;
  /** What the server filled in: the login field with the session's BSN. */
  parameters: Record<string, unknown>;
  herkomst: Herkomst[];
}

/** A dossier record as the server keeps it. The app hands it to the kern untouched. */
export type Record_ = Record<string, unknown> & { id: string; recording_actor: string };

/** A fact that comes only after an application, such as the GGD's advice. */
export interface NaAanvraag {
  feit: string;
  taak: string;
}

/** The payload the organization signs. */
export interface Gegevens {
  iss: string;
  sub: string;
  iat: number;
  /** The server's day (in test mode its test clock). */
  peildatum: string;
  casus: string;
  cel: string;
  regelwerken: { law_id: string; valid_from: string; hash: string }[];
  /** The hash of the whole source `GET /fsc4b/regelwerken` serves (`bundelHash`). */
  bundel_hash: string;
  antwoorden: Record<string, Antwoord>;
  records: Record_[];
  bij_aanvraag: NaAanvraag[];
}

/** JSON with the members of every object in one order, to compare two values for equality. */
export function canoniek(waarde: unknown): string {
  if (Array.isArray(waarde)) return `[${waarde.map(canoniek).join(',')}]`;
  if (waarde && typeof waarde === 'object') {
    const leden = Object.keys(waarde as object)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canoniek((waarde as Record<string, unknown>)[k])}`);
    return `{${leden.join(',')}}`;
  }
  return JSON.stringify(waarde ?? null);
}

/**
 * The data in `antwoord` (what `gegevens` answered), checked: signed under
 * `organisatiesleutel`, issued by `oin`, about `bsn`, and the same as the
 * copy beside the signature. Anything else throws, with the reason.
 */
export async function leesGegevens(antwoord: unknown, organisatiesleutel: PubliekeJwk, oin: string, bsn: string): Promise<Gegevens> {
  const a = antwoord as { ondertekend?: unknown; gegevens?: unknown } | null;
  if (!a || typeof a.ondertekend !== 'string') throw new Error('het antwoord bevat geen ondertekende gegevens');
  let claims: Record<string, unknown>;
  try {
    claims = await verifieer(a.ondertekend, organisatiesleutel);
  } catch {
    throw new Error('de handtekening onder de gegevens klopt niet met de sleutel van de organisatie');
  }
  if (claims.iss !== oin) throw new Error(`de gegevens komen van ${String(claims.iss)}, niet van ${oin}`);
  if (claims.sub !== bsn) throw new Error('de gegevens gaan over iemand anders dan wie de app activeerde');
  if (a.gegevens !== undefined && canoniek(a.gegevens) !== canoniek(claims)) {
    throw new Error('de gegevens naast de handtekening wijken af van wat ondertekend is');
  }
  const g = claims as unknown as Gegevens;
  if (!g.antwoorden || typeof g.antwoorden !== 'object' || typeof g.casus !== 'string' || typeof g.bundel_hash !== 'string' || !Array.isArray(g.regelwerken) || !Array.isArray(g.records) || !Array.isArray(g.bij_aanvraag) || typeof g.peildatum !== 'string') {
    throw new Error('de ondertekende gegevens missen een onderdeel');
  }
  return g;
}

/**
 * The hash of a source as `GET /fsc4b/regelwerken` serves it, as the
 * RegelHet signs it in `bundel_hash`: per path in sorted order, the UTF-8 of
 * `<bytes of the path>:<path><bytes of the text>:<text>`, all concatenated,
 * SHA-256, as `sha256:<hex>`.
 */
export async function bundelHash(bestanden: Record<string, string>): Promise<string> {
  const enc = new TextEncoder();
  const delen = Object.keys(bestanden)
    .sort()
    .flatMap((pad) => {
      const p = enc.encode(pad);
      const t = enc.encode(bestanden[pad] ?? '');
      return [enc.encode(`${p.length}:`), p, enc.encode(`${t.length}:`), t];
    });
  const alles = new Uint8Array(delen.reduce((n, d) => n + d.length, 0));
  let plek = 0;
  for (const d of delen) {
    alles.set(d, plek);
    plek += d.length;
  }
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', alles));
  return `sha256:${[...hash].map((b) => b.toString(16).padStart(2, '0')).join('')}`;
}
