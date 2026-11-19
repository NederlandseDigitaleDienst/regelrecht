// De index van de regelingen en de varianten, uit de bron, voor toets en simulatie.
import { json } from '../../bron.js';

let belofte = null;

export function laadRegelingen() {
  belofte ??= Promise.resolve().then(() => ({ index: json('laws/index.json'), varianten: json('laws/varianten.json') }));
  return belofte;
}
