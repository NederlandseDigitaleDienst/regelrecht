import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import TrajectMembersPane from './TrajectMembersPane.vue';

// The pane talks to the backend through apiFetchJson (the detail GET) and
// apiFetch (role change, removals); the real composables run on top of these.
const apiFetch = vi.fn();
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', () => ({
  apiFetch: (...a: unknown[]) => apiFetch(...a),
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
}));

const TRAJECT_ID = '11111111-2222-3333-4444-555555555555';

// Fictional members: this is a public repository.
function detail() {
  return {
    id: TRAJECT_ID,
    name: 'Voorbeeldtraject',
    role: 'owner',
    members: [
      { account_id: 'acc-owner', email: 'eigenaar@example.org', name: 'Eigenaar', role: 'owner' },
      { account_id: 'acc-contrib', email: 'bijdrager@example.org', name: 'Bijdrager', role: 'contributor' },
    ],
    pending_invites: [{ email: 'genodigde@example.org', role: 'contributor' }],
    sources: [],
  };
}

async function flush(w: VueWrapper) {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await w.vm.$nextTick();
  }
}

let mounted: VueWrapper | null = null;
async function mountPane() {
  apiFetchJson.mockResolvedValue(detail());
  mounted = mount(TrajectMembersPane, { props: { trajectId: TRAJECT_ID }, attachTo: document.body });
  await flush(mounted);
  return mounted;
}

function menuItem(w: VueWrapper, text: string) {
  const item = w.findAll('nldd-menu-item').find((i) => i.attributes('text') === text);
  if (!item) throw new Error(`no menu item "${text}"`);
  return item;
}

function bodyButton(text: string): HTMLElement {
  const button = [...document.body.querySelectorAll('nldd-button')]
    .filter((b) => b.getAttribute('text') === text)
    .at(-1);
  if (!button) throw new Error(`no button "${text}"`);
  return button as HTMLElement;
}

beforeEach(() => {
  apiFetch.mockReset().mockResolvedValue(undefined);
  apiFetchJson.mockReset();
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
});

// The pane stays mounted with the last loaded roster when the active traject
// disappears (its id prop goes to null). An action taken on that stale list
// has no traject to act on and must not send a request to `/api/trajects/null`.
describe('TrajectMembersPane without a traject id', () => {
  it('sends nothing when a role is changed after the traject is gone', async () => {
    const w = await mountPane();
    await w.setProps({ trajectId: null });

    await menuItem(w, 'Maak beheerder').trigger('select');
    await flush(w);

    expect(apiFetch).not.toHaveBeenCalled();
  });

  it('sends nothing when a member is removed after the traject is gone', async () => {
    const w = await mountPane();
    await w.setProps({ trajectId: null });

    await menuItem(w, 'Verwijder lid').trigger('select');
    await flush(w);

    expect(apiFetch).not.toHaveBeenCalled();
  });

  it('sends nothing when retracting an invite is confirmed after the traject is gone', async () => {
    const w = await mountPane();
    await w.findAll('nldd-button').find((b) => b.attributes('text') === 'Intrekken')?.trigger('click');
    await w.setProps({ trajectId: null });

    bodyButton('Trek uitnodiging in').click();
    await flush(w);

    expect(apiFetch).not.toHaveBeenCalled();
  });

  it('still removes a member while the traject is there', async () => {
    const w = await mountPane();

    await menuItem(w, 'Verwijder lid').trigger('select');
    await flush(w);

    expect(apiFetch).toHaveBeenCalledWith(
      `/api/trajects/${TRAJECT_ID}/members/acc-contrib`,
      expect.objectContaining({ method: 'DELETE' }),
    );
  });
});
