import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import AbsenceToggle from './AbsenceToggle.vue';

// RFC-036: the "afwezig" checkbox is the explicit way to state an absence
// for a field of any type. Checked <-> the value is the word `null`;
// checking stores `null`, unchecking stores a blank (unknown), never a value.
// nldd-* tags render as raw custom elements, so the change is driven by the
// CustomEvent the real nldd-checkbox-field emits (detail.checked).
const mountToggle = (props: { value: unknown; disabled?: boolean }) => mount(AbsenceToggle, { props });
type Wrapper = ReturnType<typeof mountToggle>;
const box = (w: Wrapper) => w.find('nldd-checkbox-field');
const change = (w: Wrapper, checked: boolean) =>
  box(w).element.dispatchEvent(new CustomEvent('change', { detail: { checked } }));

describe('AbsenceToggle', () => {
  it('renders the design-system checkbox field labelled afwezig', () => {
    const w = mountToggle({ value: '' });
    expect(box(w).exists()).toBe(true);
    expect(box(w).attributes('label')).toBe('afwezig');
  });

  it('is unchecked for a blank and for a value', () => {
    expect(box(mountToggle({ value: '' })).attributes('checked')).toBeUndefined();
    expect(box(mountToggle({ value: '1500' })).attributes('checked')).toBeUndefined();
    expect(box(mountToggle({ value: 0 })).attributes('checked')).toBeUndefined();
    expect(box(mountToggle({ value: 'nul' })).attributes('checked')).toBeUndefined();
  });

  it('is checked for the word null and for a JS null', () => {
    expect(box(mountToggle({ value: 'null' })).attributes('checked')).toBeDefined();
    expect(box(mountToggle({ value: null })).attributes('checked')).toBeDefined();
  });

  it('emits the word null when checked', () => {
    const w = mountToggle({ value: '1500' });
    change(w, true);
    expect(w.emitted('update')).toEqual([['null']]);
  });

  it('emits a blank (unknown), not a value, when unchecked', () => {
    const w = mountToggle({ value: 'null' });
    change(w, false);
    expect(w.emitted('update')).toEqual([['']]);
  });

  it('forwards the disabled state', () => {
    expect(box(mountToggle({ value: '', disabled: true })).attributes('disabled')).toBeDefined();
    expect(box(mountToggle({ value: '' })).attributes('disabled')).toBeUndefined();
  });
});
