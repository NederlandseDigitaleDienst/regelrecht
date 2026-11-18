import { isApiUrl } from './apiAuthGuard';

// Dev-only artificial latency on API calls, so loading states are observable.
//
// Locally every `/api/*` call resolves in ~10ms, so activity indicators, the
// per-pane "Scenario's laden"/"Tekst laden" states and empty/loading branches
// flash past before you can look at them. This wraps `fetch` and holds
// same-origin `/api/*` responses back by a configurable delay, leaving all
// other traffic (assets, /auth/*, WASM) untouched.
//
// Toggle from the URL, no rebuild and no code edit:
//   ?slow=1500   delay every /api/* response by 1500ms
//   ?slow=0      turn it off again
//
// The value persists in localStorage, so you pass the query param once and it
// survives SPA navigation and reloads until you clear it. Guarded by
// `import.meta.env.DEV`, so it can never reach a production build.

const STORAGE_KEY = 'rr-dev-slow-ms';

/**
 * Resolve the delay: an explicit `?slow=<ms>` wins and is persisted (0 clears),
 * otherwise fall back to whatever was stored earlier.
 * @returns {number} delay in ms; 0 = disabled
 */
function readDelayMs() {
  let stored = 0;
  try {
    stored = Number(localStorage.getItem(STORAGE_KEY)) || 0;
  } catch {
    // Private mode / storage disabled - slow mode simply stays off.
  }

  const param = new URLSearchParams(window.location.search).get('slow');
  if (param === null) return Math.max(0, stored);

  const ms = Math.max(0, Number(param) || 0);
  try {
    if (ms) localStorage.setItem(STORAGE_KEY, String(ms));
    else localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Ignore: the delay still applies for this page load.
  }
  return ms;
}

/**
 * Wrap `window.fetch` to delay `/api/*` responses. Call once at app start,
 * after installApiAuthGuard so the 401 redirect is not held up by the delay.
 */
export function installDevSlowMode() {
  if (!import.meta.env.DEV) return;

  const ms = readDelayMs();
  if (!ms) return;

  const originalFetch = window.fetch.bind(window);
  window.fetch = async (input, init) => {
    const response = await originalFetch(input, init);
    if (isApiUrl(input)) {
      await new Promise((resolve) => setTimeout(resolve, ms));
    }
    return response;
  };

  console.info(
    `[dev] slow mode: /api/* responses delayed by ${ms}ms. Use ?slow=0 to disable.`,
  );
}
