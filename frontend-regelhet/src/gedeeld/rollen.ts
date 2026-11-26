// The roles of a UI spec (`role` of an element) on the two sides of RegelHet.
// A UI spec names its roles in its own words; RegelHet knows a citizen and a
// ladder of employees (reader < decider < admin). A role of the general
// administrative law that names whoever applies or objects is the citizen's,
// and so is the role of an element that fills in an element of such a role
// (`rol_generiek`, from `builds_on`): an executor may call its applicant what
// it likes. A role that decides is the decider's; every other role, such as
// one that examines an application, is for every employee, the reader
// included.

/** The roles that are the citizen's side. */
export const BURGERROLLEN = ['aanvrager', 'belanghebbende', 'burger', 'indiener'];

/** The roles of a UI spec that take the decision. */
export const BESLISROLLEN = ['beslisser', 'bestuursorgaan'];

/** Whether an element of role `rol` (filling in one of `rolGeneriek`) is for the citizen. Without a role it is for no side in particular. */
export const voorBurger = (rol: string | undefined, rolGeneriek?: string): boolean =>
  (!!rol && BURGERROLLEN.includes(rol)) || (!!rolGeneriek && BURGERROLLEN.includes(rolGeneriek));

/**
 * Whether an employee at `niveau` (`lezer`, `beslisser`, `admin`) sees an
 * element of role `rol`: a citizen's element never, a deciding one from the
 * decider up, any other one always.
 */
export function voorMedewerker(rol: string | undefined, niveau: string | null | undefined, rolGeneriek?: string): boolean {
  if (!rol || voorBurger(rol, rolGeneriek)) return false;
  if (BESLISROLLEN.includes(rol)) return niveau === 'beslisser' || niveau === 'admin';
  return !!niveau;
}
