import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import InviteMembersSheet from './InviteMembersSheet.vue';

// The sheet loads the traject (apiFetchJson) for its name and posts each invite
// through the same function; the real composable runs on top of this mock.
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', () => ({
  apiFetch: vi.fn(),
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
}));

const TRAJECT_ID = '11111111-2222-3333-4444-555555555555';

async function flush(w: VueWrapper) {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await w.vm.$nextTick();
  }
}

let mounted: VueWrapper | null = null;
async function mountSheet() {
  mounted = mount(InviteMembersSheet, { props: { trajectId: TRAJECT_ID }, attachTo: document.body });
  await flush(mounted);
  return mounted;
}

// The sheet teleports to the body; take the elements of the sheet mounted last.
function lastInBody(selector: string): HTMLElement {
  const el = [...document.body.querySelectorAll(selector)].at(-1);
  if (!el) throw new Error(`no ${selector}`);
  return el as HTMLElement;
}

// Type an address the way nldd-token-field reports uncommitted input.
function typeAddress(value: string): void {
  lastInBody('nldd-token-field').dispatchEvent(
    new CustomEvent('input', { detail: { value }, bubbles: true, composed: true }),
  );
}

function submit(): void {
  lastInBody('form').dispatchEvent(new Event('submit', { cancelable: true }));
}

const postedUrls = () =>
  apiFetchJson.mock.calls
    .filter(([, options]) => (options as { method?: string } | undefined)?.method === 'POST')
    .map(([url]) => url);

beforeEach(() => {
  apiFetchJson.mockReset().mockImplementation(async (_url: string, options?: { method?: string; body?: string }) =>
    options?.method === 'POST'
      ? { status: 'pending', email: JSON.parse(options.body ?? '{}').email }
      : { id: TRAJECT_ID, name: 'Voorbeeldtraject', members: [], pending_invites: [] },
  );
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
});

describe('InviteMembersSheet without a traject id', () => {
  // The sheet stays open when the active traject disappears (its id prop goes
  // to null). Submitting then has no traject to invite into and must not post
  // to `/api/trajects/null/members`.
  it('posts nothing when the traject is gone by the time the form is submitted', async () => {
    const w = await mountSheet();
    typeAddress('nieuw@example.org');
    await w.setProps({ trajectId: null });

    submit();
    await flush(w);

    expect(postedUrls()).toEqual([]);
  });

  it('still invites while the traject is there', async () => {
    const w = await mountSheet();
    typeAddress('nieuw@example.org');

    submit();
    await flush(w);

    expect(postedUrls()).toEqual([`/api/trajects/${TRAJECT_ID}/members`]);
  });
});
