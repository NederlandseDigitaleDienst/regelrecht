// How a screen of the back office names a fact: the last part of
// `<regelwerk>#<article>#<fact>`, in words.
import { hoofdletter, leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';

/** The name of the fact `feit`, such as "Geneeskundig advies uitgebracht". */
export const feitNaam = (feit: string): string => hoofdletter(leesbaar(feit.split('#').at(-1) ?? feit));

/**
 * The fact `feit` as a thing to record, the participle first: "Verzonden
 * bericht" for "Bericht verzonden". The name does not say whether a weak
 * participle takes an -e before its noun ("vastgestelde gegevens"), so it
 * stays as the name has it.
 */
export function vastlegNaam(feit: string): string {
  const woorden = leesbaar(feit.split('#').at(-1) ?? feit).split(' ');
  const deelwoord = woorden.pop();
  return woorden.length && deelwoord ? hoofdletter(`${deelwoord} ${woorden.join(' ')}`) : feitNaam(feit);
}
