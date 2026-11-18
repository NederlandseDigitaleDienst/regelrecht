import { onMounted, onUnmounted, ref } from 'vue';

/**
 * Is het venster smal genoeg om de weergave uit te dunnen?
 *
 * Eén drempel voor de hele demo, zodat het portaal, de simulatie en de
 * werkbalk niet elk op een eigen breedte omklappen. 900px valt samen met het
 * punt waarop `nldd-navigation-split-view` zijn zijpaneel niet meer naast de
 * inhoud kwijt kan.
 *
 * Dit gaat bewust niet via een CSS-mediaquery: de delen die verdwijnen zitten
 * in slots van `nldd-title`, en die component zet er in zijn shadow-DOM een
 * eigen display op die een regel van buiten niet overstemt.
 */
export const NARROW_BREAKPOINT = 900;

export function useNarrow(breakpoint = NARROW_BREAKPOINT) {
  const narrow = ref(typeof window === 'undefined' ? false : window.innerWidth < breakpoint);
  function onResize() {
    narrow.value = window.innerWidth < breakpoint;
  }
  onMounted(() => window.addEventListener('resize', onResize));
  onUnmounted(() => window.removeEventListener('resize', onResize));
  return narrow;
}
