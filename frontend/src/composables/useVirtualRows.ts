/**
 * Render only the rows of a long list that are in view (and a margin around
 * them), so a list of thousands of rows costs what a list of a few dozen
 * does. Below `threshold` rows every row renders, exactly as without it.
 *
 * Built for the article list in the library until the design system offers
 * this itself (design-system#409). Every row of an `nldd-list` is several
 * custom elements, and with six thousand of them each change to the list
 * costs a layout of all of them (design-system#408).
 *
 * The list scrolls with an `nldd-page`: inside the page on a wide screen,
 * with the document when the split view collapses to one column. Its
 * `scrollTarget` says which. The scroll listener goes on that element (or on
 * the window for the document), and moves when the page's size changes,
 * since that is when the mode can change. A listener on the document would
 * not do: a scroll inside the page's shadow root never reaches it.
 *
 * Row heights vary (the supporting text wraps), so rows are measured once
 * rendered, with a ResizeObserver: it reports sizes after layout and forces
 * none of its own. A row not measured yet counts as the average of the
 * measured ones; when that estimate changes, the scroll position moves along
 * so the rows in view stay put.
 *
 * Limits: Home, End and the wrap from the last row go to the ends of the
 * rendered rows, not of the whole list, and find-in-page only finds rows
 * that are rendered. The rows carry `aria-setsize` and `aria-posinset` (see
 * the template), so a screen reader still hears where a row is.
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { Ref } from 'vue';
import { rowWindow } from '../lib/virtualRows';
import type { RowWindow } from '../lib/virtualRows';

/** What an `nldd-page` exposes about its scrolling. */
export interface PageScroller {
  scrollTarget: HTMLElement;
}

export interface VirtualRowsOptions {
  /** Above this many rows the list renders only a window. */
  threshold: number;
  /** Assumed height of a row before any is measured, in px. */
  estimate: number;
  /** Rendered beyond the view, above and below, in px. */
  margin: number;
}

/** The attribute that tells the measuring observer which row an element is. */
export const ROW_KEY_ATTRIBUTE = 'data-row-key';

/** Rows to render before there is a viewport to measure (first paint, tests). */
const ROWS_WITHOUT_VIEWPORT = 40;

export function useVirtualRows<T>(
  items: Ref<readonly T[]>,
  keyOf: (item: T) => string,
  page: Ref<(HTMLElement & Partial<PageScroller>) | null>,
  list: Ref<HTMLElement | null>,
  options: VirtualRowsOptions,
) {
  const active = computed(() => items.value.length > options.threshold);

  // Heights by row key. Not reactive itself: a measurement round bumps
  // `measuredVersion` once, so a few hundred rows reporting at once cost one
  // recompute.
  const measured = new Map<string, number>();
  const measuredVersion = ref(0);
  function forgetHeights(): void {
    if (!measured.size) return;
    measured.clear();
    measuredVersion.value++;
  }
  const heights = computed(() => {
    void measuredVersion.value;
    const average = measured.size
      ? [...measured.values()].reduce((a, b) => a + b, 0) / measured.size
      : options.estimate;
    return items.value.map((item) => measured.get(keyOf(item)) ?? average);
  });

  // How far the list has scrolled past its own start, and how tall the view is.
  const top = ref(0);
  const viewport = ref(0);

  const range = computed((): RowWindow => {
    const count = items.value.length;
    if (!active.value) return { start: 0, end: count, padStart: 0, padEnd: 0 };
    return rowWindow(heights.value, top.value, viewport.value, options.margin, ROWS_WITHOUT_VIEWPORT);
  });

  /** The rows to render, each with its position in the whole list. */
  const rows = computed(() => {
    const { start, end } = range.value;
    return items.value.slice(start, end).map((item, offset) => ({ item, index: start + offset }));
  });

  /** Padding that stands in for the rows outside the window. */
  const listStyle = computed(() =>
    active.value
      ? {
          paddingBlockStart: `${range.value.padStart}px`,
          paddingBlockEnd: `${range.value.padEnd}px`,
        }
      : undefined,
  );

  /** The element that scrolls, and whether that is the document itself. */
  function scroller(): { el: HTMLElement; root: boolean } | null {
    const el = page.value?.scrollTarget;
    if (!el) return null;
    return { el, root: el === document.scrollingElement };
  }

  function readScroll(): void {
    const s = scroller();
    const el = list.value;
    if (!s || !el) return;
    // The list's top edge is where row 0 would start: the padding above the
    // window is part of the list. When the document scrolls, the view starts
    // at the top of the window.
    const viewTop = s.root ? 0 : s.el.getBoundingClientRect().top;
    top.value = viewTop - el.getBoundingClientRect().top;
    viewport.value = s.root ? globalThis.innerHeight : s.el.clientHeight;
  }

  let frame = 0;
  function onScroll(): void {
    if (frame || !active.value) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      readScroll();
    });
  }

  // When the estimate for rows above the window changes, the rows in view
  // would move with it. Move the scroll position by the same amount instead.
  // Without this a single measurement can shift the view by hundreds of
  // pixels deep into the list, and the browser's own scroll anchoring does
  // not step in for padding.
  watch(
    () => [range.value.start, range.value.padStart] as const,
    ([start, padStart], previous) => {
      if (!previous || start !== previous[0] || padStart === previous[1]) return;
      const s = scroller();
      if (!s) return;
      s.el.scrollTop += padStart - previous[1];
    },
    { flush: 'post' },
  );

  const rowObserver =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver((entries) => {
          let changed = false;
          for (const entry of entries) {
            const target = entry.target as HTMLElement;
            // A row the window dropped: stop watching it, or the observer
            // keeps it alive for as long as the view lives.
            if (!target.isConnected) {
              rowObserver?.unobserve(target);
              continue;
            }
            const key = target.getAttribute(ROW_KEY_ATTRIBUTE);
            const height = entry.borderBoxSize?.[0]?.blockSize;
            if (key === null || !height || measured.get(key) === height) continue;
            measured.set(key, height);
            changed = true;
          }
          if (changed) measuredVersion.value++;
        });

  // The scroll listener sits on whatever scrolls right now.
  let scrollSource: EventTarget | null = null;
  function listenToScroll(): void {
    const s = scroller();
    const source = s ? (s.root ? globalThis : s.el) : null;
    if (source === scrollSource) return;
    scrollSource?.removeEventListener('scroll', onScroll);
    source?.addEventListener('scroll', onScroll, { passive: true });
    scrollSource = source;
  }

  // The page's size is the view's size; a new width also rewraps the rows, so
  // their heights no longer hold, and can switch the page between scrolling
  // itself and letting the document scroll.
  let pageWidth = 0;
  const pageObserver =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver((entries) => {
          const width = entries[0]?.contentRect.width ?? 0;
          if (pageWidth && width !== pageWidth) forgetHeights();
          pageWidth = width;
          listenToScroll();
          onScroll();
        });

  // Keyboard focus on a row that leaves the window would fall to the body,
  // and the arrow keys would stop working. Move it to the first row in view.
  let focusInList = false;
  function onFocusIn(): void {
    focusInList = true;
  }
  function onFocusOut(event: FocusEvent): void {
    const next = event.relatedTarget as Node | null;
    if (next && list.value?.contains(next)) return;
    // A row the window removes loses focus too, with no `relatedTarget`.
    // That is not the user leaving the list: only once the row is still
    // there did focus really go elsewhere.
    const row = event.target as Node;
    queueMicrotask(() => {
      if (row.isConnected) focusInList = false;
    });
  }

  watch(
    rows,
    () => {
      const el = list.value;
      if (!active.value || !el) return;
      if (rowObserver) {
        for (const row of el.querySelectorAll(`[${ROW_KEY_ATTRIBUTE}]`)) rowObserver.observe(row);
      }
      if (focusInList && !el.contains(document.activeElement)) {
        const first = firstRowInView(el);
        if (first) first.focus({ preventScroll: true });
        else focusInList = false;
      }
    },
    { flush: 'post' },
  );

  // A different filter or law: the window has to be placed again.
  watch(items, () => onScroll());

  // Wire up for the current page and list while there is a window to keep.
  let wiredList: HTMLElement | null = null;
  let wiredPage: HTMLElement | null = null;
  function unwire(): void {
    scrollSource?.removeEventListener('scroll', onScroll);
    scrollSource = null;
    globalThis.removeEventListener('resize', onScroll);
    wiredList?.removeEventListener('focusin', onFocusIn);
    wiredList?.removeEventListener('focusout', onFocusOut);
    if (wiredPage) pageObserver?.unobserve(wiredPage);
    wiredList = null;
    wiredPage = null;
  }
  watch(
    [page, list, active],
    ([pageEl, listEl, isActive]) => {
      // A new list is another law (or the same one opened again): heights
      // measured for the old one say nothing about this one.
      if (listEl !== wiredList) forgetHeights();
      unwire();
      if (!isActive || !pageEl || !listEl) return;
      listenToScroll();
      globalThis.addEventListener('resize', onScroll, { passive: true });
      listEl.addEventListener('focusin', onFocusIn);
      listEl.addEventListener('focusout', onFocusOut);
      pageObserver?.observe(pageEl);
      wiredList = listEl;
      wiredPage = pageEl;
      onScroll();
    },
    { flush: 'post', immediate: true },
  );

  onBeforeUnmount(() => {
    unwire();
    rowObserver?.disconnect();
    pageObserver?.disconnect();
    if (frame) cancelAnimationFrame(frame);
  });

  return { active, rows, listStyle };
}

/** The first rendered row whose bottom is below the top of the view. */
function firstRowInView(list: HTMLElement): HTMLElement | null {
  const rows = list.querySelectorAll<HTMLElement>(`[${ROW_KEY_ATTRIBUTE}]`);
  for (const row of rows) if (row.getBoundingClientRect().bottom > 0) return row;
  return rows[0] ?? null;
}
