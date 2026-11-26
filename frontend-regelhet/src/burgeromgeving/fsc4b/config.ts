// The RegelHets this app knows: `regelhets.json` beside the app, which the
// deployment sets (deploy/burgerapp/). Later a shared Stelseldocument lists
// them, with a `discovery_url` per organization; until then this file is the
// list.
import { DEMO_ORGANIZATION_OIN } from './client';
import type { Regelhet } from './kern';

/** The list in `tekst`, checked: every entry has an id, a name and an https or localhost address. */
export function leesConfig(tekst: string): Regelhet[] {
  const ruw = JSON.parse(tekst) as { regelhets?: unknown };
  if (!Array.isArray(ruw.regelhets)) throw new Error('regelhets.json heeft geen lijst `regelhets`');
  const ids = new Set<string>();
  return ruw.regelhets.map((r: Record<string, unknown>, i) => {
    const { id, naam, adres } = r;
    if (typeof id !== 'string' || !/^[a-z0-9-]+$/.test(id)) throw new Error(`regelhets[${i}]: id is a-z, 0-9 en -`);
    if (ids.has(id)) throw new Error(`regelhets[${i}]: id '${id}' staat er twee keer`);
    ids.add(id);
    if (typeof naam !== 'string' || !naam) throw new Error(`regelhets[${i}]: geen naam`);
    if (typeof adres !== 'string' || !/^(https:\/\/|http:\/\/localhost[:/])/.test(adres)) {
      throw new Error(`regelhets[${i}]: het adres is https, of http op localhost`);
    }
    const oin = typeof r.oin === 'string' ? r.oin : DEMO_ORGANIZATION_OIN;
    return { id, naam, adres: adres.replace(/\/+$/, ''), oin };
  });
}

/** The list the app is served with, under `basis`. */
export async function laadConfig(basis: string): Promise<Regelhet[]> {
  const antwoord = await fetch(`${basis}regelhets.json`, { cache: 'no-cache' });
  if (!antwoord.ok) throw new Error(`regelhets.json laadt niet (${antwoord.status})`);
  return leesConfig(await antwoord.text());
}
