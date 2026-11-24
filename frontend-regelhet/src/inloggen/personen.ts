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
  if (p.soort === 'beheerder') return 'Beheerder';
  return `${hoofdletter(p.niveau ?? 'medewerker')} bij ${p.cel_naam ?? p.cel ?? ''}`.trim();
}

/** What an account of the test login may do, in a line. */
export function personaUitleg(p: Persona): string {
  if (p.soort === 'burger') {
    if (p.label && p.label !== 'BSN') return `Dient aanvragen en bezwaren in namens het bedrijf en ziet het eigen dossier. Het ${p.label} is fictief.`;
    return 'Dient aanvragen en bezwaren in en ziet het eigen dossier. Het BSN is fictief.';
  }
  if (p.soort === 'beheerder') return 'Ziet de kronieken van alle cellen, beslist voor elke cel en zet de testklok.';
  if (p.niveau === 'beslisser') return 'Legt feiten vast voor deze cel, verstuurt afschriften van haar besluiten en neemt besluiten.';
  return 'Ziet de kroniek en de aanvragen van deze cel, maar legt niets vast en beslist niet.';
}
