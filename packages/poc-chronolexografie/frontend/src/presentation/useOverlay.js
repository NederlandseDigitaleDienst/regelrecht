/**
 * What a panel over the whole page needs while it is open, shared by the
 * deck (PresentationDeck.vue) and the explanation page (UitlegPagina.vue).
 *
 * Behind the portal a strip with the voorbehoud sits sticky at the top of
 * every page (packages/poc-portal, `data-poc-portaal`), above everything. The
 * panel starts under it (`top`), so the strip stays readable and covers
 * nothing.
 *
 * The panel takes the focus when it opens, so the keys and a screen reader
 * start on it, and gives it back to where it was when it closes. The app
 * under it is inert meanwhile: Tab and a screen reader stay on the panel and
 * the portal's strip above it. `forget` drops the element to give the focus
 * back to, for when it is gone after closing.
 */
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';

export function useOverlay(active, panel) {
  const top = ref(0);
  let strip = null;
  const resize = new ResizeObserver(() => {
    top.value = strip ? Math.max(0, strip.getBoundingClientRect().bottom) : 0;
  });
  function followStrip() {
    strip = document.querySelector('[data-poc-portaal]');
    resize.disconnect();
    if (strip) resize.observe(strip);
    top.value = strip ? Math.max(0, strip.getBoundingClientRect().bottom) : 0;
  }
  onBeforeUnmount(() => resize.disconnect());

  const app = () => document.getElementById('app');
  let before = null;
  watch(active, async (open) => {
    if (open) {
      followStrip();
      before = document.activeElement;
      if (app()) app().inert = true;
      await nextTick();
      panel.value?.focus();
    } else {
      if (app()) app().inert = false;
      if (before?.isConnected) before.focus?.();
      before = null;
    }
  });

  return {
    top,
    forget: () => {
      before = null;
    },
  };
}
