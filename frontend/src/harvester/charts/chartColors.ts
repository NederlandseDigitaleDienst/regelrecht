/**
 * Resolve the NLDD design tokens the charts need to concrete rgb() strings.
 *
 * ECharts renders to canvas, so it can't consume CSS custom properties or
 * light-dark() directly. Each token is resolved by computing it on a probe
 * element and normalizing through a 1×1 canvas - which also converts oklch()
 * (the NLDD primitive format, unsupported by zrender) to plain rgb.
 *
 * The probe hangs off document.body, NOT the chart's own container: freshly
 * mounted NLDD subtrees compute `color-scheme: normal` until the custom
 * elements upgrade, and under `normal` every light-dark() is invalid and
 * resolves to black. body already carries the design system's `light dark`
 * at mount, and it follows the data-scheme switch, so probing there is both
 * correct and available immediately.
 */

// succeeded/failed are the NLDD success and critical primitives. The job-status
// bar in the grouped jobs view is an nldd-progress-bar and takes the design
// system's semantic category colors, a shade darker than these. added is
// hemelblauw rather than the accent: the accent
// blue fails the chart-palette checks (chroma floor, dark-mode contrast),
// hemelblauw is the nearest passing NLDD hue.
const TOKEN_EXPRESSIONS = {
  succeeded:
    'light-dark(var(--primitives-color-success-450), var(--primitives-color-success-550))',
  failed: 'var(--primitives-color-critical-450)',
  added:
    'light-dark(var(--primitives-color-hemelblauw-500), var(--primitives-color-hemelblauw-550))',
  text: 'var(--semantics-content-color)',
  textSecondary: 'var(--semantics-content-secondary-color)',
  grid: 'light-dark(var(--primitives-color-neutral-150), var(--primitives-color-neutral-250))',
};

type ChartToken = keyof typeof TOKEN_EXPRESSIONS;

/** Concrete rgb() (or computed-style fallback) strings, one per chart slot. */
export type ChartColors = Record<ChartToken | 'surface', string>;

/**
 * @returns {object|null} The resolved colors, or null when the page's color
 *   scheme isn't established yet (caller should retry on a later frame).
 */
export function resolveChartColors(): ChartColors | null {
  const probe = document.createElement('span');
  probe.style.display = 'none';
  document.body.appendChild(probe);

  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });

  const colors: Partial<ChartColors> = {};
  try {
    if (ctx && getComputedStyle(document.body).colorScheme === 'normal') {
      // light-dark() is invalid under color-scheme: normal - the design
      // system's scheme isn't applied yet. Signal "not ready".
      return null;
    }

    const toRgb = (resolved: string): string => {
      if (!ctx) {
        // No 2D context (test environments): fall back to the computed string.
        return resolved;
      }
      ctx.clearRect(0, 0, 1, 1);
      // Canvas silently keeps the previous fillStyle on a parse failure; seed
      // a sentinel so an unparseable value falls back to the computed string
      // instead of leaking the previous token's color.
      ctx.fillStyle = '#010203';
      ctx.fillStyle = resolved;
      if (ctx.fillStyle === '#010203' && resolved !== '#010203') {
        return resolved;
      }
      ctx.fillRect(0, 0, 1, 1);
      const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
      return `rgb(${r}, ${g}, ${b})`;
    };

    for (const [key, expression] of Object.entries(TOKEN_EXPRESSIONS) as [
      ChartToken,
      string,
    ][]) {
      probe.style.color = expression;
      colors[key] = toRgb(getComputedStyle(probe).color);
    }
    // The charts sit on transparent nldd-cards, so the visible surface behind
    // the marks is the page background (there is no NLDD surface token that
    // resolves at :root level).
    colors.surface = toRgb(getComputedStyle(document.body).backgroundColor);
  } finally {
    probe.remove();
  }
  // Every token key plus surface was assigned above.
  return colors as ChartColors;
}
