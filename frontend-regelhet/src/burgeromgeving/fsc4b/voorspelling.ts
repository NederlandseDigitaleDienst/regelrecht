// What the prediction screen does before the citizen applies: fetch the
// signed source data and the regelwerken of the chosen RegelHet, replay the
// records in the kern on this device, compare its answers with the signed
// ones, and predict the decision on what the form holds. The form's values
// go nowhere but into the kern here.
import type { Indieningsvorm } from '../../gedeeld/types';
import { bundelHash, leesGegevens } from './gegevens';
import type { Gegevens } from './gegevens';
import type { Burgerapp } from './kern';
import { Rekenkern } from './rekenen';
import type { MaakPoc, Vergelijkingen, Voorspelling } from './rekenen';

/** The kern of one RegelHet, ready to predict, with the data it was replayed from. */
export interface Voorbereid {
  gegevens: Gegevens;
  kern: Rekenkern;
  vergelijking: Vergelijkingen;
}

/** The signed data of the citizen at `id`, checked; the regelwerken loaded into the kern, the records replayed, the answers compared. */
export async function bereidVoor(app: Burgerapp, id: string, maak: MaakPoc, vorm: Indieningsvorm): Promise<Voorbereid> {
  const status = await app.status(id);
  if (!status.holder) throw new Error('De app is bij deze regeling niet geactiveerd.');
  const [ruw, organisatie, bestanden] = await Promise.all([app.roep(id, 'gegevens', {}), app.organisatie(id), app.regelwerken(id)]);
  const gegevens = await leesGegevens(ruw, organisatie.organisatiesleutel, organisatie.oin, status.holder.bsn);
  // The regelwerken come unsigned; the signed data names their hash, and the
  // app computes on nothing else.
  if ((await bundelHash(bestanden)) !== gegevens.bundel_hash) {
    throw new Error('De regels die de app ophaalde, zijn niet de regels die de regeling ondertekende. De app voorspelt daarom niets.');
  }
  const kern = Rekenkern.laad(maak, bestanden, gegevens.casus);
  try {
    kern.naspelen(gegevens.records);
    return { gegevens, kern, vergelijking: kern.vergelijk(gegevens, inlogveld(gegevens, vorm)) };
  } catch (e) {
    kern.sluit();
    throw e;
  }
}

/**
 * The field the login fills with the citizen's number: the one parameter the
 * server filled in for its answers, or else the field of the form the login
 * fills without naming a claim.
 */
export function inlogveld(gegevens: Gegevens, vorm: Indieningsvorm): string {
  return (
    Object.keys(Object.values(gegevens.antwoorden)[0]?.parameters ?? {})[0] ??
    Object.entries(vorm.systeem ?? {}).find(([, hoe]) => hoe.soort === 'inloggen' && !hoe.inlog)?.[0] ??
    ''
  );
}

/**
 * The fields of the application as the server would record them: what the
 * citizen filled in (`velden`), and every field the system fills, from what
 * the server said of it in the forms (`waarde`), the login (the BSN, or the
 * name in the credential) or the day. A field the system fills that the app
 * cannot know stays out, and the kern says what it misses.
 */
export function lokaleVelden(
  vorm: Indieningsvorm,
  velden: Record<string, unknown>,
  wie: { bsn: string; naam: string | null; datum: string },
): Record<string, unknown> {
  const uit: Record<string, unknown> = { ...velden };
  for (const [naam, hoe] of Object.entries(vorm.systeem ?? {})) {
    if ('waarde' in hoe && hoe.waarde !== undefined) uit[naam] = hoe.waarde;
    else if (hoe.soort === 'inloggen') {
      const waarde = hoe.inlog === 'naam' ? wie.naam : hoe.inlog ? undefined : wie.bsn;
      if (waarde) uit[naam] = waarde;
    } else if (hoe.soort === 'datum') uit[naam] = wie.datum;
  }
  return uit;
}

/** The prediction on `voorbereid` for an application under `artikel` with the form's `velden`. */
export function voorspel(
  voorbereid: Voorbereid,
  vorm: Indieningsvorm,
  artikel: string,
  velden: Record<string, unknown>,
  naam: string | null,
): Voorspelling {
  const { gegevens, kern } = voorbereid;
  return kern.voorspel({
    artikel,
    velden: lokaleVelden(vorm, velden, { bsn: gegevens.sub, naam, datum: gegevens.peildatum }),
    datum: gegevens.peildatum,
    bijAanvraag: gegevens.bij_aanvraag,
    inlogveld: inlogveld(gegevens, vorm),
    bsn: gegevens.sub,
  });
}
