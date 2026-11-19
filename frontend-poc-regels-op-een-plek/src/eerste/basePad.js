// Overgenomen uit NederlandseDigitaleDienst/regelrecht, frontend-poc-terugbetaalregimes (EUPL-1.2).
// Elke URL die de app zelf ophaalt, relatief aan waar de app gemonteerd is: lokaal op `/`,
// achter het poc-portaal op `/<slug>/`. `base` in vite.config.js dekt een fetch() niet.
export const b = (pad) => import.meta.env.BASE_URL + String(pad).replace(/^\//, '');
