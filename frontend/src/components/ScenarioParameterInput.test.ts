import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ScenarioParameterInput from './ScenarioParameterInput.vue';

// nldd-* tags render as raw custom elements (vite.config.js isCustomElement),
// so we drive the component by dispatching the CustomEvents the real NDD
// components emit (detail.checked / detail.value) and assert the typed value
// the component emits via `update`.
type InputProps = InstanceType<typeof ScenarioParameterInput>['$props'];

function mountInput(props: InputProps) {
  return mount(ScenarioParameterInput, { attachTo: document.body, props });
}

function lastUpdate(wrapper: ReturnType<typeof mountInput>): unknown {
  const events = wrapper.emitted<[unknown]>('update');
  if (!events?.length) throw new Error('no update event emitted');
  // Non-empty, checked above.
  return events[events.length - 1]![0];
}

describe('ScenarioParameterInput', () => {
  describe('control selection per datatype', () => {
    const cases: [string, string][] = [
      ['boolean', 'nldd-switch-field'],
      ['number', 'nldd-number-field'],
      ['amount', 'nldd-number-field'],
      ['date', 'nldd-date-field'],
      ['string', 'nldd-text-field'],
    ];
    for (const [type, tag] of cases) {
      it(`renders ${tag} for type=${type}`, () => {
        const wrapper = mountInput({ type, value: '' });
        expect(wrapper.find(tag).exists()).toBe(true);
      });
    }

    it('falls back to text-field for an unknown type', () => {
      const wrapper = mountInput({ type: 'mystery', value: '' });
      expect(wrapper.find('nldd-text-field').exists()).toBe(true);
    });
  });

  describe('boolean', () => {
    it('renders a string "true" value as checked', () => {
      const wrapper = mountInput({ type: 'boolean', value: 'true' });
      expect(wrapper.find('nldd-switch-field').attributes('checked')).toBeDefined();
    });

    it('renders a falsy value as unchecked', () => {
      const wrapper = mountInput({ type: 'boolean', value: 'false' });
      expect(wrapper.find('nldd-switch-field').attributes('checked')).toBeUndefined();
    });

    it('emits a real boolean on toggle', () => {
      const wrapper = mountInput({ type: 'boolean', value: 'false' });
      wrapper.find('nldd-switch-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { checked: true } }),
      );
      expect(lastUpdate(wrapper)).toBe(true);
    });
  });

  describe('number', () => {
    it('emits a real number', () => {
      const wrapper = mountInput({ type: 'number', value: '' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: '42' } }),
      );
      expect(lastUpdate(wrapper)).toBe(42);
    });

    it('emits empty string for a cleared field', () => {
      const wrapper = mountInput({ type: 'number', value: '7' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '' } }),
      );
      expect(lastUpdate(wrapper)).toBe('');
    });
  });

  describe('amount', () => {
    it('displays eurocent values in euros', () => {
      const wrapper = mountInput({ type: 'amount', unit: 'eurocent', value: 109171 });
      expect(wrapper.find('nldd-number-field').attributes('value')).toBe('1091.71');
    });

    it('emits eurocents when the unit is eurocent', () => {
      const wrapper = mountInput({ type: 'amount', unit: 'eurocent', value: '' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '1500' } }),
      );
      expect(lastUpdate(wrapper)).toBe(150000);
    });

    it('emits the raw number when no unit is declared', () => {
      const wrapper = mountInput({ type: 'amount', unit: null, value: '' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '1500' } }),
      );
      expect(lastUpdate(wrapper)).toBe(1500);
    });
  });

  // RFC-036: a stated absence shows as the word `null`, in a text field
  // whatever the declared type - a number field cannot hold it, a switch
  // cannot show it, a date picker would reject it.
  describe('a stated absence (null)', () => {
    for (const type of ['number', 'amount', 'date', 'boolean', 'string']) {
      it(`shows the word null in a text field for type=${type}`, () => {
        const wrapper = mountInput({ type, unit: type === 'amount' ? 'eurocent' : null, value: 'null' });
        const text = wrapper.find('nldd-text-field');
        expect(text.exists()).toBe(true);
        expect(text.attributes('value')).toBe('null');
        expect(wrapper.find('nldd-date-field').exists()).toBe(false);
        expect(wrapper.find('nldd-number-field').exists()).toBe(false);
        expect(wrapper.find('nldd-switch-field').exists()).toBe(false);
      });
    }

    it('treats a JS null from a typed record the same way', () => {
      const wrapper = mountInput({ type: 'amount', unit: 'eurocent', value: null });
      expect(wrapper.find('nldd-text-field').attributes('value')).toBe('null');
    });

    it('does not mistake a blank or the text "nul" for an absence', () => {
      expect(mountInput({ type: 'number', value: '' }).find('nldd-number-field').exists()).toBe(true);
      expect(mountInput({ type: 'string', value: 'nul' }).find('nldd-text-field').attributes('value')).toBe('nul');
    });

    it('lets the author type over the null, emitting the new text', () => {
      const wrapper = mountInput({ type: 'number', value: 'null' });
      wrapper.find('nldd-text-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: '' } }),
      );
      expect(lastUpdate(wrapper)).toBe('');
    });

    it('carries the invalid state and the unmet ids onto the text field', () => {
      const wrapper = mountInput({ type: 'number', value: 'null', invalid: true, unmet: 'e1' });
      const text = wrapper.find('nldd-text-field');
      expect(text.attributes('invalid')).toBeDefined();
      expect(text.attributes('unmet')).toBe('e1');
    });
  });

  describe('date / string', () => {
    it('emits the date string', () => {
      const wrapper = mountInput({ type: 'date', value: '' });
      wrapper.find('nldd-date-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: '2025-01-01' } }),
      );
      expect(lastUpdate(wrapper)).toBe('2025-01-01');
    });

    it('emits a date picked in the calendar, which fires only change', () => {
      const wrapper = mountInput({ type: 'date', value: '' });
      wrapper.find('nldd-date-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '2025-01-01' } }),
      );
      expect(lastUpdate(wrapper)).toBe('2025-01-01');
    });

    it('emits the string value', () => {
      const wrapper = mountInput({ type: 'string', value: '' });
      wrapper.find('nldd-text-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: 'hello' } }),
      );
      expect(lastUpdate(wrapper)).toBe('hello');
    });
  });
});
