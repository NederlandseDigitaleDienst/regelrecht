/**
 * Which rows of a long list to render: the ones in view, plus a margin above
 * and below. The rows outside that window are replaced by padding of their
 * combined height, so the scrollbar and the scroll position stay as if every
 * row were there.
 *
 * Pure arithmetic over row heights, so it is testable without a browser;
 * `useVirtualRows` supplies the heights and the scroll position.
 */

/** The rows to render, `start` inclusive and `end` exclusive, and the space the others take. */
export interface RowWindow {
  start: number;
  end: number;
  padStart: number;
  padEnd: number;
}

/**
 * The window of rows that overlaps `[top - margin, top + viewport + margin]`,
 * where `top` is how far the list has scrolled past its own start (negative
 * while the list starts below the top of the view).
 *
 * `heights` holds one entry per row. With no viewport yet (`viewport <= 0`,
 * before the first layout or in a test DOM) it renders `fallbackRows` rows
 * from the start, so the list is never empty.
 */
export function rowWindow(
  heights: readonly number[],
  top: number,
  viewport: number,
  margin: number,
  fallbackRows: number,
): RowWindow {
  const count = heights.length;
  if (viewport <= 0) {
    const end = Math.min(count, fallbackRows);
    return { start: 0, end, padStart: 0, padEnd: sum(heights, end, count) };
  }
  const from = Math.max(0, top - margin);
  const to = top + viewport + margin;

  let start = 0;
  let offset = 0;
  while (start < count && offset + (heights[start] ?? 0) <= from) {
    offset += heights[start] ?? 0;
    start++;
  }
  // Scrolled past the end (a filter shortened the list before the position
  // was read again): keep the last row, so the list never renders empty.
  if (start === count && count > 0) {
    start = count - 1;
    offset -= heights[start] ?? 0;
  }
  const padStart = offset;
  let end = start;
  while (end < count && (offset < to || end === start)) {
    offset += heights[end] ?? 0;
    end++;
  }
  return { start, end, padStart, padEnd: sum(heights, end, count) };
}

function sum(heights: readonly number[], from: number, to: number): number {
  let total = 0;
  for (let i = from; i < to; i++) total += heights[i] ?? 0;
  return total;
}
