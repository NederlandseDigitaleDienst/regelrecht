/**
 * The "why" button: a plain-language explanation of one outcome, written by a
 * language model from the engine's trace (server/why.mjs).
 *
 * The demo has no backend of its own, so this is the one feature that needs
 * one, and it has to degrade to nothing: without a server behind /api/why (a
 * plain `just demo`, or a deploy without a token) the probe fails and neither
 * the menu item nor the button shows.
 *
 * The password lives under its own localStorage key and not in the demo state.
 * "Reset demo" puts the demo back to its starting position for the next
 * audience; logging the presenter out of a feature they unlocked on purpose is
 * not part of that.
 */
import { computed, ref } from 'vue';

export const PASSWORD_KEY = 'rr-demo-why-password';

function readPassword() {
  try {
    return window.localStorage?.getItem(PASSWORD_KEY) ?? '';
  } catch {
    return '';
  }
}
function storePassword(value: string) {
  try {
    if (value) window.localStorage?.setItem(PASSWORD_KEY, value);
    else window.localStorage?.removeItem(PASSWORD_KEY);
  } catch {
    /* storage unavailable: the unlock holds for this session only */
  }
}

/** A server answered the probe. */
export const whyAvailable = ref(false);
const password = ref(readPassword());
export const whyUnlocked = computed(() => whyAvailable.value && !!password.value);

/**
 * Ask whether a server is there. Checked on the content type too: under
 * `vite` without a proxy target, or on a static host, /api/why can come back
 * as index.html with a 200.
 */
export async function probeWhy(fetchImpl: typeof fetch = fetch): Promise<boolean> {
  try {
    const res = await fetchImpl('/api/why', { headers: { accept: 'application/json' } });
    const json: { available?: unknown } | null = res.ok && (res.headers.get('content-type') ?? '').includes('application/json') ? await res.json() : null;
    whyAvailable.value = json?.available === true;
  } catch {
    whyAvailable.value = false;
  }
  return whyAvailable.value;
}

export async function unlockWhy(candidate: string, fetchImpl: typeof fetch = fetch): Promise<'ok' | 'wrong' | 'unavailable'> {
  // A header carries Latin-1 only; fetch throws on anything else before a
  // request goes out, which would read as "no server". No such password was
  // ever set, so it is simply wrong.
  if (/[^\u0000-\u00ff]/.test(candidate)) return 'wrong';
  try {
    const res = await fetchImpl('/api/why/check', { method: 'POST', headers: { 'x-demo-password': candidate } });
    if (res.status === 204) {
      password.value = candidate;
      storePassword(candidate);
      return 'ok';
    }
    return res.status === 401 ? 'wrong' : 'unavailable';
  } catch {
    return 'unavailable';
  }
}

export function lockWhy() {
  password.value = '';
  storePassword('');
}

export type WhyErrorKind = 'locked' | 'busy' | 'failed';

export class WhyError extends Error {
  kind: WhyErrorKind;

  constructor(kind: WhyErrorKind, message?: string) {
    super(message ?? kind);
    this.kind = kind;
  }
}

/**
 * Stream an explanation. `onText` gets the whole text so far on every chunk,
 * so the caller can just render it.
 */
export interface ExplainOptions {
  onText?: (text: string) => void;
  signal?: AbortSignal;
  fetchImpl?: typeof fetch;
}

export async function explainWhy(payload: unknown, { onText, signal, fetchImpl = fetch }: ExplainOptions = {}): Promise<string> {
  const res = await fetchImpl('/api/why', {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-demo-password': password.value },
    body: JSON.stringify(payload),
    signal,
  });
  if (res.status === 401) {
    // The password changed on the server. Forget it, so the menu offers the
    // unlock again instead of a button that keeps failing.
    lockWhy();
    throw new WhyError('locked');
  }
  if (res.status === 429) throw new WhyError('busy');
  if (!res.ok || !res.body) throw new WhyError('failed', `HTTP ${res.status}`);

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let text = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    text += decoder.decode(value, { stream: true });
    onText?.(text);
  }
  text += decoder.decode();
  onText?.(text);
  return text;
}

/**
 * The model's text as paragraphs and lists. The prompt asks for paragraphs
 * separated by a blank line and allows a short list with '- '; in practice a
 * list follows its lead sentence without a blank line, so lines are grouped
 * one by one rather than per block.
 */
export type WhyBlock = { text: string } | { items: string[] };

export function toBlocks(text: string | null | undefined): WhyBlock[] {
  const blocks: WhyBlock[] = [];
  // The block being filled; one of the two fields is set.
  let current: { text?: string; items?: string[] } | null = null;
  for (const raw of String(text ?? '').split('\n')) {
    const line = raw.trim();
    if (!line) {
      current = null;
      continue;
    }
    if (line.startsWith('- ')) {
      if (!current?.items) blocks.push((current = { items: [] as string[] }));
      // Either checked on the line above or just created with a list.
      current.items!.push(line.slice(2));
    } else if (!current || current.items) {
      blocks.push((current = { text: line }));
    } else {
      current.text += ` ${line}`;
    }
  }
  return blocks;
}
