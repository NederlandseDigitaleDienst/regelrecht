// A clickable word in a RegelRijm reading is a span with role="button". Its
// keys follow a native button: Enter acts on key down, Space on key up, and
// the Space key down only keeps the page from scrolling.
import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import type { Segment } from '@regelrecht/frontend-shared/regelrijm/index';
import RegelRijmText from './RegelRijmText.vue';

const FOLD: Segment = { kind: 'tok', tk: 'fold', text: '(1)', number: '1' };
const WORD: Segment = { kind: 'tok', tk: 'keyword', text: 'maal' };

function reading() {
  const wrapper = mount(RegelRijmText, { props: { segments: ['hoogte wordt ', FOLD, ' ', WORD], interactive: true } });
  return { wrapper, fold: wrapper.find('[data-testid="regelrijm-open-1"]') };
}

describe('RegelRijmText keyboard', () => {
  it('opens on Enter key down', async () => {
    const { wrapper, fold } = reading();
    await fold.trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('open')).toHaveLength(1);
  });

  it('opens on Space key up, not key down', async () => {
    const { wrapper, fold } = reading();
    const down = new KeyboardEvent('keydown', { key: ' ', bubbles: true, cancelable: true });
    fold.element.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
    expect(wrapper.emitted('open')).toBeUndefined();
    await fold.trigger('keyup', { key: ' ' });
    expect(wrapper.emitted('open')).toHaveLength(1);
  });

  it('ignores a Space key up that went down elsewhere', async () => {
    const { wrapper, fold } = reading();
    await fold.trigger('keyup', { key: ' ' });
    expect(wrapper.emitted('open')).toBeUndefined();
  });

  it('leaves Space alone on a word that does nothing', () => {
    const { wrapper } = reading();
    const word = wrapper.find('.rr-keyword');
    const down = new KeyboardEvent('keydown', { key: ' ', bubbles: true, cancelable: true });
    word.element.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(false);
    expect(word.attributes('role')).toBeUndefined();
  });
});
