// The accounts of the test login in words, for the login screen. The server
// derives them from the case (`GET /auth/testpersonen`); this module only
// names them.
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import type { Persona } from './api';

/** The name an account of the test login shows, and is logged in under. */
export function personaNaam(p: Persona): string {
  if (p.soort === 'burger') {
    const label = p.label ?? 'BSN';
    const wie = label === 'BSN' ? 'Burger' : 'Bedrijf';
    return p.naam ? `${p.naam}, ${label} ${p.identiteit ?? ''}` : `${wie} met ${label} ${p.identiteit ?? ''}`.trim();
  }
  if (p.naam) return p.naam;
  if (p.soort === 'beheerder') return p.partij && p.cel_naam ? `Beheerder bij ${p.cel_naam}` : 'Beheerder';
  if (p.partij) return `Medewerker bij ${p.cel_naam ?? p.cel ?? ''}`.trim();
  return `${hoofdletter(p.niveau ?? 'medewerker')} bij ${p.cel_naam ?? p.cel ?? ''}`.trim();
}

/** The role of an account, for the tag after its name: an applicant is a citizen or a business. */
export function personaRol(p: Persona): string {
  if (p.soort === 'burger') return p.label && p.label !== 'BSN' ? 'Bedrijf' : 'Burger';
  if (p.soort === 'beheerder') return 'Beheerder';
  if (p.partij) return 'Medewerker';
  return hoofdletter(p.niveau ?? 'medewerker');
}

/** A colour of its own per role, from the Rijkshuisstijl palette: a semantic colour would say something is wrong. */
const ROLKLEUREN: Record<string, string> = {
  Burger: 'hemelblauw',
  Bedrijf: 'oranje',
  Medewerker: 'donkergeel',
  Beslisser: 'groen',
  Beheerder: 'robijnrood',
};

/** The colour of the tag with an account's role. */
export function personaKleur(p: Persona): string {
  return ROLKLEUREN[personaRol(p)] ?? 'neutral';
}

/** What an account of the test login may do, in a line. */
export function personaUitleg(p: Persona): string {
  if (p.soort === 'burger') {
    if (p.label && p.label !== 'BSN') return `Dient aanvragen en bezwaren in namens het bedrijf en ziet het eigen dossier. Het ${p.label} is fictief.`;
    return 'Dient aanvragen en bezwaren in en ziet het eigen dossier. Het BSN is fictief.';
  }
  const cel = p.cel_naam ?? 'deze cel';
  if (p.partij && p.soort === 'beheerder') return `Beantwoordt verzoeken aan ${p.cel_naam ?? 'deze organisatie'}, zet de testklok en maakt de testomgeving leeg.`;
  if (p.partij) return `Ziet de verzoeken aan ${p.cel_naam ?? 'deze organisatie'} en legt de antwoorden vast.`;
  if (p.soort === 'beheerder') return 'Ziet de kronieken van alle cellen, beslist voor elke cel en zet de testklok.';
  if (p.niveau === 'beslisser') return `Legt feiten vast voor ${cel}, verstuurt afschriften van haar besluiten en neemt besluiten.`;
  return `Ziet de kroniek en de aanvragen van ${cel}, maar legt niets vast en beslist niet.`;
}

/** The icon of an account's avatar when it has no initials: an admin's for who decides or administers, a plain user's for the others. */
export function personaIcoon(p: Persona): 'user-admin' | 'user' {
  return p.soort === 'beheerder' || (p.soort !== 'burger' && p.niveau === 'beslisser') ? 'user-admin' : 'user';
}
