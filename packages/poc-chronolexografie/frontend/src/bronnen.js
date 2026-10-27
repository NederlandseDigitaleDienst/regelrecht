/**
 * De bronnen van de app: corpora die het poc-portaal ophaalt terwijl de app
 * draait (`sources` van chronolexografie in pocs/registry.yaml), zoals de
 * NAPP-bron met een casus, haar regeling en haar cel. Welke er zijn, schrijft
 * scripts/copy-assets.js naar public/bronnen.json; elke bron staat op
 * `bron/<naam>`, achter hetzelfde wachtwoord als de app.
 *
 * Een bron die er niet is (geen token, lokaal geen POC_BRON_<NAAM>, GitHub
 * plat) is geen fout van de app: de casus van die bron toont dan "bron niet
 * beschikbaar", en de andere casussen werken.
 */

/** Een bron zoals de cel haar neemt (src/bundel.rs, `Bron`). */
export async function haalBron(naam, url, haal = fetch) {
  try {
    const res = await haal(url);
    if (res.status === 401) {
      // Het cookie van het portaal is verlopen; de body is dan het inlogscherm.
      return { naam, fout: 'bron niet beschikbaar: log opnieuw in op deze poc' };
    }
    if (!res.ok) {
      const tekst = (await res.text()).trim();
      const fout = tekst.startsWith('bron ') ? tekst : `bron niet beschikbaar: ${res.status} ${tekst || res.statusText}`;
      return { naam, fout };
    }
    const b = await res.json();
    return { naam, versie: b.versie ?? null, bestanden: b.bestanden ?? {}, melding: b.melding ?? null };
  } catch (e) {
    return { naam, fout: `bron niet beschikbaar: ${e?.message ?? e}` };
  }
}

/**
 * Elke bron uit `namen`, elk op `url(naam)`, tegelijk opgehaald. Een lijst
 * die niet te lezen is (een oude build zonder bronnen.json), is geen bron.
 */
export async function haalBronnen(namenUrl, url, haal = fetch) {
  let namen = [];
  try {
    const res = await haal(namenUrl);
    if (res.ok) namen = await res.json();
  } catch {
    namen = [];
  }
  return Promise.all(namen.map((naam) => haalBron(naam, url(naam), haal)));
}

/**
 * Wat het keuzescherm per bron meldt, uit wat de cel ervan maakte
 * (`casussen().bronnen`, src/bundel.rs `BronStand`): een bron die er niet is
 * of niet laadt is kritiek, een casus die niet laadt of een casus-id die al
 * bestaat een waarschuwing. Een bron zonder iets te
 * melden staat er niet in. De regels zijn de meldingen zelf, met het
 * bestand dat ze noemen.
 */
export function bronMeldingen(bronnen = []) {
  const uit = [];
  for (const b of bronnen) {
    if (b.fout) {
      // Niet beschikbaar: geen token of map, of het portaal kon de branch
      // niet ophalen (GitHub plat, een verlopen token). Anders laadt de bron
      // wel, maar een bestand erin niet.
      const weg = /^bron (niet beschikbaar|kon niet worden opgehaald)/.test(b.fout);
      uit.push({
        naam: b.naam,
        variant: weg ? 'warning' : 'critical',
        tekst: weg ? `Bron ${b.naam} niet beschikbaar` : `Bron ${b.naam} laadt niet`,
        regels: [b.fout],
      });
      continue;
    }
    const regels = [...(b.fouten ?? []), ...(b.meldingen ?? [])];
    if (regels.length) {
      uit.push({
        naam: b.naam,
        variant: b.fouten?.length ? 'critical' : 'warning',
        tekst: `Bron ${b.naam}${b.versie ? ` (${b.versie})` : ''}: ${regels.length === 1 ? 'één melding' : `${regels.length} meldingen`}`,
        regels,
      });
    }
  }
  return uit;
}

/** Waar een casus vandaan komt, voor op haar kaart; leeg voor een eigen casus. */
export function herkomst(casus, bronnen = []) {
  if (!casus.bron) return '';
  const b = bronnen.find((x) => x.naam === casus.bron);
  return ` · uit bron ${casus.bron}${b?.versie ? ` (${b.versie})` : ''}`;
}
