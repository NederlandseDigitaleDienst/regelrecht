import { afterEach, describe, expect, it, vi } from 'vitest';
import { key, setValue } from './actions';

/** A host element standing in for a design-system sheet, with its `hide()`. */
function makeSheet(): HTMLElement & { hide: ReturnType<typeof vi.fn> } {
  const sheet = document.createElement('rr-test-sheet') as HTMLElement & { hide: ReturnType<typeof vi.fn> };
  sheet.hide = vi.fn();
  return sheet;
}

describe('a replayed Escape', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('closes the sheet it is pressed in, through the sheet itself', () => {
    const sheet = makeSheet();
    const dialog = document.createElement('dialog');
    dialog.setAttribute('open', '');
    sheet.attachShadow({ mode: 'open' }).appendChild(dialog);
    document.body.appendChild(sheet);
    key(dialog, 'Escape');
    expect(sheet.hide).toHaveBeenCalledOnce();
  });

  it('leaves it open when the page took the key, and for any other key', () => {
    const sheet = makeSheet();
    const dialog = document.createElement('dialog');
    dialog.setAttribute('open', '');
    sheet.attachShadow({ mode: 'open' }).appendChild(dialog);
    document.body.appendChild(sheet);
    key(dialog, 'Enter');
    dialog.addEventListener('keydown', (e) => e.preventDefault());
    key(dialog, 'Escape');
    expect(sheet.hide).not.toHaveBeenCalled();
  });
});

describe('a replayed value', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('goes into the field inside a design-system element', () => {
    const host = document.createElement('rr-test-field');
    const input = document.createElement('input');
    host.attachShadow({ mode: 'open' }).appendChild(input);
    document.body.appendChild(host);
    const seen = vi.fn();
    input.addEventListener('input', seen);
    setValue(host, 'abc');
    expect(input.value).toBe('abc');
    expect(seen).toHaveBeenCalledOnce();
  });

  it('is set as a plain property on an element without a field', () => {
    const host = document.createElement('rr-test-plain') as HTMLElement & { value?: string };
    document.body.appendChild(host);
    const seen = vi.fn();
    host.addEventListener('input', seen);
    setValue(host, 'abc');
    expect(host.value).toBe('abc');
    expect(seen).toHaveBeenCalledOnce();
  });
});
