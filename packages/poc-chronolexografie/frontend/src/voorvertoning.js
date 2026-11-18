/**
 * Een voorvertoning die meeloopt met wat het bestuursorgaan invult. Na elke
 * wijziging plant `plan()` een verzoek; pas als er `wacht` ms niets meer
 * veranderde, gaat het de deur uit. Komt een antwoord binnen nadat er
 * intussen opnieuw gepland is, dan hoort het bij invoer die er niet meer
 * staat en wordt het weggegooid: alleen het laatste verzoek telt.
 *
 * `vraag()` doet het verzoek; `toon()` krijgt `{ antwoord }` of `{ fout }`.
 */
export function volger(vraag, toon, wacht = 300) {
  let klok = null;
  let nummer = 0;
  return {
    plan() {
      clearTimeout(klok);
      const dit = ++nummer;
      klok = setTimeout(async () => {
        let uit;
        try {
          uit = { antwoord: await vraag() };
        } catch (e) {
          uit = { fout: e.message };
        }
        if (dit === nummer) toon(uit);
      }, wacht);
    },
    /** Niets meer tonen, ook niet wat nog onderweg is. */
    stop() {
      clearTimeout(klok);
      nummer++;
    },
  };
}
