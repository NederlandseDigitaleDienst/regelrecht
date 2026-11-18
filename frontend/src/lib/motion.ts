/**
 * How a programmatic scroll should move: smooth, unless the viewer asked their
 * system for less motion. The browser does not apply that setting to
 * `scrollIntoView({ behavior: 'smooth' })` by itself.
 */
export function scrollBehavior(): ScrollBehavior {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth';
}
