// Which slots each nldd-* component actually defines, and which ones a source
// tree assigns to. Shared by check-nldd-slots.mjs and its test.
//
// The reason this needs checking at all: a `slot="..."` that a component does
// not define is not an error anywhere. The element stays in the light DOM,
// never gets assigned to a shadow-DOM slot, and lays out as 0x0 — present in
// the DOM, invisible on screen, silent in the console. Two of those sat in the
// demo unnoticed: the persona tags on the portal and the "Start de
// presentatie" button, both on `slot="actions"` of `nldd-title`, which only
// has `overline`, `subtitle` and `end`.

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';

export const EXTENSIONS = ['.vue', '.html', '.js', '.ts'];

/**
 * Every file under `dir` with one of these extensions. A directory that is not
 * there yields nothing: the design-system package is only present after
 * `npm ci`, and this guard has to stay silent where it is not, not crash.
 */
export function sourceFiles(dir, extensions = EXTENSIONS, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === 'dist' || entry.startsWith('.')) continue;
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) sourceFiles(full, extensions, out);
    else if (extensions.some((e) => entry.endsWith(e))) out.push(full);
  }
  return out;
}

/**
 * The slot names a component's template declares, per tag.
 *
 * Templates are `*.template.js` files in the design-system package that render
 * `<slot name="x">`; a component with only a default slot has an entry with an
 * empty set, which is what makes "this tag takes no named slots" checkable.
 *
 * A component whose template builds slot names at runtime (`<slot name=${...}>`)
 * accepts any name by design — `nldd-bar-split-view` names a slot after each
 * child it is given, which is how a page adds its own toolbars. Those map to
 * `null`, meaning "do not check this tag", rather than to a set that would
 * reject every real usage.
 *
 * A component can also read a slot's children itself without rendering a
 * `<slot>` for it: `nldd-toolbar` copies its `overflow` children into its
 * overflow menu. Such a slot is documented in the package's manifest
 * (`custom-elements.json`, next to `dist/`), so the slots listed there count
 * too. The manifest only adds to a tag the templates already describe; a tag
 * without a template stays unchecked, as before.
 */
export function componentSlots(packageDir) {
  const slots = new Map();
  for (const file of sourceFiles(packageDir, ['.template.js'])) {
    // .../components/<group>/<tag>/<tag>.template.js
    const match = /([^/]+)\.template\.js$/.exec(file);
    if (!match) continue;
    const tag = `nldd-${match[1]}`;
    const text = readFileSync(file, 'utf8');
    if (/<slot\s+name=\$\{/.test(text)) {
      slots.set(tag, null);
      continue;
    }
    const names = new Set();
    for (const m of text.matchAll(/<slot\s+name=["']([a-z0-9-]+)["']/gi)) names.add(m[1]);
    // A component can also forward a slot it received; those show up as
    // `slot="x"` on an element inside its own template.
    for (const m of text.matchAll(/\bslot=["']([a-z0-9-]+)["']/gi)) names.add(m[1]);
    slots.set(tag, names);
  }
  for (const [tag, names] of manifestSlots(packageDir)) {
    const known = slots.get(tag);
    if (known) for (const name of names) known.add(name);
  }
  return slots;
}

/**
 * The named slots per tag in the manifest of the package `packageDir` sits in
 * (`<package>/dist/components` → `<package>/custom-elements.json`). Nothing when
 * there is no manifest.
 */
function manifestSlots(packageDir) {
  const file = join(dirname(dirname(packageDir)), 'custom-elements.json');
  const out = new Map();
  if (!existsSync(file)) return out;
  const manifest = JSON.parse(readFileSync(file, 'utf8'));
  for (const mod of manifest.modules ?? []) {
    for (const decl of mod.declarations ?? []) {
      if (!decl.tagName) continue;
      const names = (decl.slots ?? []).map((slot) => slot.name).filter(Boolean);
      out.set(decl.tagName, names);
    }
  }
  return out;
}

/**
 * Every `slot="name"` assignment in a source tree, with the nldd-* parent it
 * lands in.
 *
 * The parent is the nearest enclosing element, found by scanning backwards for
 * the last unclosed opening tag. That is a heuristic, not a parser: it is
 * accurate for the shape these templates actually have (an nldd-* element with
 * slotted children on their own lines) and is deliberately conservative —
 * when the parent cannot be determined the assignment is skipped rather than
 * reported, so this never invents a failure.
 */
export function slotAssignments(dir, extensions = EXTENSIONS) {
  const found = [];
  for (const file of sourceFiles(dir, extensions)) {
    const text = readFileSync(file, 'utf8');
    for (const m of text.matchAll(/<([a-z][a-z0-9-]*)\b([^>]*?)\bslot=["']([a-z0-9-]+)["']/gi)) {
      const parent = enclosingTag(text, m.index);
      if (!parent?.startsWith('nldd-')) continue;
      found.push({
        file,
        line: text.slice(0, m.index).split('\n').length,
        parent,
        child: m[1].toLowerCase(),
        slot: m[3],
      });
    }
  }
  return found;
}

/** The tag of the nearest element still open at `index`. */
function enclosingTag(text, index) {
  const before = text.slice(0, index);
  const stack = [];
  for (const m of before.matchAll(/<(\/?)([a-z][a-z0-9-]*)\b([^>]*)>/gi)) {
    const [, closing, tag, attrs] = m;
    if (closing) {
      const at = stack.lastIndexOf(tag.toLowerCase());
      if (at !== -1) stack.length = at;
    } else if (!attrs.trimEnd().endsWith('/')) {
      stack.push(tag.toLowerCase());
    }
  }
  return stack[stack.length - 1] ?? null;
}

/**
 * Slot assignments whose parent component does not define that slot.
 *
 * A tag the package does not describe is skipped: it is either not a
 * design-system component or one whose template this scan cannot see, and
 * guessing there would produce noise instead of findings. So is a component
 * that names its slots at runtime (`null`), which accepts anything.
 */
export function unknownSlots(assignments, slots) {
  return assignments.filter(({ parent, slot }) => {
    const known = slots.get(parent);
    return known != null && !known.has(slot);
  });
}
