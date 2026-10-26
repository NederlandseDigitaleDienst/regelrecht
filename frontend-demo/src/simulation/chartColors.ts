/**
 * Resolve the design tokens the charts need to plain rgb() strings.
 *
 * ECharts draws on a canvas and cannot read CSS custom properties or
 * light-dark(); each token is computed on a probe element under document.body
 * (which carries the design system's colour scheme) and normalised through a
 * 1x1 canvas, which also turns oklch() into rgb. Same approach as the editor's
 * harvester charts.
 */
// Eén token per kleur, zonder `light-dark()` eromheen: de schalen keren zelf al
// om tussen licht en donker, dus een token in `light-dark()` zetten kiest in
// donkere modus juist de verkeerde kant (zie de toelichting bij de graaf-kleuren
// in css/main.css). `success-*` en `neutral-*` bestonden bovendien niet — de
// neutrale schaal heet hier `coolgray`, en groen is gewoon `groen`.
const TOKEN_EXPRESSIONS = {
  primary: 'var(--primitives-color-donkerblauw-600)',
  secondary: 'var(--primitives-color-hemelblauw-500)',
  tertiary: 'var(--primitives-color-mintgroen-600)',
  quaternary: 'var(--primitives-color-oranje-600)',
  success: 'var(--primitives-color-groen-600)',
  text: 'var(--semantics-content-color)',
  textSecondary: 'var(--semantics-content-secondary-color)',
  grid: 'var(--primitives-color-coolgray-200)',
} as const;

/** A colour role the charts use, such as `primary` or `grid`. */
export type ChartColorKey = keyof typeof TOKEN_EXPRESSIONS;

/** Every chart colour role resolved to an `rgb(r, g, b)` string. */
export type ChartColors = Record<ChartColorKey, string>;

export const SERIES_KEYS: readonly ChartColorKey[] = ['primary', 'secondary', 'tertiary', 'quaternary'];

/**
 * One CSS colour expression (`var(--primitives-color-oranje-500)`) as rgb(),
 * for a series that brings its own colour. Same probe as below, so it follows
 * the colour scheme the same way.
 */
export function resolveCssColor(expr: string): string | null {
  if (typeof document === 'undefined') return null;
  const probe = document.createElement('span');
  probe.style.display = 'none';
  probe.style.color = expr;
  document.body.appendChild(probe);
  try {
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    const resolved = getComputedStyle(probe).color;
    if (!ctx) return resolved;
    ctx.fillStyle = resolved;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return `rgb(${r}, ${g}, ${b})`;
  } finally {
    probe.remove();
  }
}

/** Returns null while the colour scheme has not been applied yet. */
export function resolveChartColors(): ChartColors | null {
  if (typeof document === 'undefined') return null;
  const probe = document.createElement('span');
  probe.style.display = 'none';
  document.body.appendChild(probe);
  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  try {
    if (getComputedStyle(document.body).colorScheme === 'normal') return null;
    const toRgb = (resolved: string): string => {
      if (!ctx) return resolved;
      ctx.clearRect(0, 0, 1, 1);
      ctx.fillStyle = resolved;
      ctx.fillRect(0, 0, 1, 1);
      const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
      return `rgb(${r}, ${g}, ${b})`;
    };
    // Filled for every key by the loop below.
    const out = {} as ChartColors;
    for (const [key, expr] of Object.entries(TOKEN_EXPRESSIONS) as [ChartColorKey, string][]) {
      probe.style.color = expr;
      out[key] = toRgb(getComputedStyle(probe).color);
    }
    return out;
  } finally {
    probe.remove();
  }
}
