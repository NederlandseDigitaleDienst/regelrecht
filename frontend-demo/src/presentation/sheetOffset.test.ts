import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { RULES, addOffsetRule } from './sheetOffset';

// The workaround styles internal classes of two design-system components. If
// an update renames one, its rule matches nothing and the sheet silently
// opens over the deck rail again. This reads the components' own templates,
// so such an update fails here instead.
const here = dirname(fileURLToPath(import.meta.url));
const layout = join(here, '..', '..', '..', 'node_modules', '@nldd', 'design-system', 'dist', 'components', 'layout');

describe('sheet offset workaround', () => {
  it('still matches the design system sheet', () => {
    const template = readFileSync(join(layout, 'sheet', 'sheet.template.js'), 'utf8');
    const styles = readFileSync(join(layout, 'sheet', 'sheet.styles.js'), 'utf8');
    expect(template).toMatch(/<dialog class="sheet"/);
    expect(styles).toMatch(/:host\(\[placement="left"\]\) \.sheet/);
    // The open and closing states the replacement animation hooks into.
    expect(styles).toMatch(/&\[open\][\s\S]*sheet-slide-in-left/);
    expect(styles).toMatch(/&\.is-closing[\s\S]*sheet-slide-out-left/);
  });

  it("still matches the split view's sidebar sheet", () => {
    const template = readFileSync(join(layout, 'split-views', 'navigation-split-view', 'navigation-split-view.template.js'), 'utf8');
    expect(template).toMatch(/<dialog class="navigation-split-view__primary-sidebar-sheet"/);
  });

  it('adds its rule once per component', () => {
    const host = document.createElement('nldd-sheet');
    host.attachShadow({ mode: 'open' });
    addOffsetRule(host);
    addOffsetRule(host);
    const root = host.shadowRoot;
    expect(root).not.toBeNull();
    // `!`: asserted on the line above.
    const rules = root!.querySelectorAll('style[data-rr-sheet-offset]');
    expect(rules).toHaveLength(1);
    expect(rules[0]?.textContent).toBe(RULES['nldd-sheet']);
  });
});
