// Het poc-portaal zet boven elke pagina een strook met het voorbehoud (packages/poc-portal,
// pagina.rs). Die strook plakt bovenaan en ligt boven alles. Deze app vult het scherm, dus hij
// meet de strook en zet de hoogte in --portaal-strook, zodat de app en de presentatie eronder
// beginnen in plaats van eronder te verdwijnen. Zonder portaal (lokaal) is de hoogte 0.
export function volgPortaalStrook(doc = document) {
  const strook = doc.querySelector('[data-poc-portaal]');
  const zet = () => doc.documentElement.style.setProperty('--portaal-strook', `${strook?.offsetHeight ?? 0}px`);
  zet();
  if (strook && typeof ResizeObserver !== 'undefined') new ResizeObserver(zet).observe(strook);
}
