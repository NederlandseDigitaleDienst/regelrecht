// How a screen of the back office names a fact: the last part of
// `<regelwerk>#<article>#<fact>`, in words.
import { hoofdletter, leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';

/** The name of the fact `feit`, such as "Geneeskundig advies uitgebracht". */
export const feitNaam = (feit: string): string => hoofdletter(leesbaar(feit.split('#').at(-1) ?? feit));
