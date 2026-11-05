// Doet de slot-guard wat hij belooft: een slot die het component niet heeft
// wordt gevonden, en alles wat legitiem is blijft ongemoeid.
import { strict as assert } from 'node:assert';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { componentSlots, slotAssignments, unknownSlots } from './nldd-slots.mjs';

/** Een nep-ontwerpsysteem met een paar componenten. */
function fakePackage() {
  const dir = mkdtempSync(join(tmpdir(), 'nldd-pkg-'));
  const write = (tag, body) => {
    const sub = join(dir, 'dist', 'components', tag);
    mkdirSync(sub, { recursive: true });
    writeFileSync(join(sub, `${tag}.template.js`), body);
  };
  // Zoals nldd-title: overline, subtitle, end — géén actions.
  write('title', `export function t() { return html\`
    <slot name="overline"></slot><slot></slot><slot name="subtitle"></slot><slot name="end"></slot>\`; }`);
  // Zoals nldd-inline-dialog: heeft juist wél actions.
  write('inline-dialog', `export function t() { return html\`<slot></slot><slot name="actions"></slot>\`; }`);
  // Zoals nldd-bar-split-view: benoemt slots pas als het draait.
  write('bar-split-view', 'export function t(c) { return html`<slot name=${pane.slot}></slot>`; }');
  // Zoals nldd-toolbar: leest de kinderen van overflow zelf, zonder <slot>
  // ervoor; alleen het manifest noemt die slot.
  write('toolbar', 'export function t() { return html`<slot name="start"></slot><slot name="end"></slot>`; }');
  writeFileSync(
    join(dir, 'custom-elements.json'),
    JSON.stringify({
      modules: [
        { declarations: [{ tagName: 'nldd-toolbar', slots: [{ name: 'start' }, { name: 'end' }, { name: 'overflow' }] }] },
        // A tag without a template stays unchecked, also with a manifest entry.
        { declarations: [{ tagName: 'nldd-ghost', slots: [{ name: 'only' }] }] },
      ],
    }),
  );
  return join(dir, 'dist', 'components');
}

/** The package root of the `dist/components` directory fakePackage returns. */
function pkgRoot(componentsDir) {
  return join(componentsDir, '..', '..');
}

function fakeSource(files) {
  const dir = mkdtempSync(join(tmpdir(), 'nldd-src-'));
  for (const [name, body] of Object.entries(files)) writeFileSync(join(dir, name), body);
  return dir;
}

test('vindt een slot die het component niet heeft', () => {
  const pkg = fakePackage();
  const src = fakeSource({
    'Bad.vue': `<template>
      <nldd-title>
        <nldd-container slot="actions"><nldd-tag text="ZZP"></nldd-tag></nldd-container>
      </nldd-title>
    </template>`,
  });
  try {
    const bad = unknownSlots(slotAssignments(src), componentSlots(pkg));
    assert.equal(bad.length, 1);
    assert.equal(bad[0].parent, 'nldd-title');
    assert.equal(bad[0].slot, 'actions');
    assert.equal(bad[0].child, 'nldd-container');
  } finally {
    rmSync(pkgRoot(pkg), { recursive: true, force: true });
    rmSync(src, { recursive: true, force: true });
  }
});

test('laat een slot die het component wél heeft met rust', () => {
  const pkg = fakePackage();
  const src = fakeSource({
    'Good.vue': `<template>
      <nldd-title>
        <span slot="overline">Ingelogd</span>
        <nldd-container slot="end"><nldd-tag text="ZZP"></nldd-tag></nldd-container>
      </nldd-title>
      <nldd-inline-dialog>
        <nldd-button slot="actions" text="Voeg toe"></nldd-button>
      </nldd-inline-dialog>
    </template>`,
  });
  try {
    assert.deepEqual(unknownSlots(slotAssignments(src), componentSlots(pkg)), []);
  } finally {
    rmSync(pkgRoot(pkg), { recursive: true, force: true });
    rmSync(src, { recursive: true, force: true });
  }
});

test('laat een component dat zijn slots pas bij het draaien benoemt met rust', () => {
  const pkg = fakePackage();
  const src = fakeSource({
    'Bars.vue': `<template>
      <nldd-bar-split-view>
        <nldd-container slot="toolbar"></nldd-container>
        <nldd-split-view-pane slot="verzonnen-naam"></nldd-split-view-pane>
      </nldd-bar-split-view>
    </template>`,
  });
  try {
    assert.deepEqual(unknownSlots(slotAssignments(src), componentSlots(pkg)), []);
  } finally {
    rmSync(pkgRoot(pkg), { recursive: true, force: true });
    rmSync(src, { recursive: true, force: true });
  }
});

test('laat een tag die niet uit het ontwerpsysteem komt met rust', () => {
  const pkg = fakePackage();
  const src = fakeSource({
    'Plain.vue': `<template>
      <my-widget><div slot="whatever"></div></my-widget>
    </template>`,
  });
  try {
    assert.deepEqual(unknownSlots(slotAssignments(src), componentSlots(pkg)), []);
  } finally {
    rmSync(pkgRoot(pkg), { recursive: true, force: true });
    rmSync(src, { recursive: true, force: true });
  }
});

test('kent een slot die alleen het manifest noemt', () => {
  const pkg = fakePackage();
  const src = fakeSource({
    'Toolbar.vue': `<template>
      <nldd-toolbar>
        <nldd-menu-item slot="overflow" text="Verrijk"></nldd-menu-item>
        <nldd-menu-item slot="center" text="Fout"></nldd-menu-item>
      </nldd-toolbar>
      <nldd-ghost>
        <span slot="anything"></span>
      </nldd-ghost>
    </template>`,
  });
  try {
    const bad = unknownSlots(slotAssignments(src), componentSlots(pkg));
    assert.deepEqual(bad.map((b) => `${b.parent}:${b.slot}`), ['nldd-toolbar:center']);
  } finally {
    rmSync(pkgRoot(pkg), { recursive: true, force: true });
    rmSync(src, { recursive: true, force: true });
  }
});

test('leest de slots van het echte ontwerpsysteem, met nldd-title als ijkpunt', (t) => {
  const slots = componentSlots('node_modules/@nldd/design-system/dist/components');
  // De pre-commit-job in CI draait geen `npm ci`; daar valt niets te ijken.
  if (slots.size === 0) return t.skip('ontwerpsysteem niet geïnstalleerd');
  const title = slots.get('nldd-title');
  assert.ok(title, 'nldd-title moet gevonden worden');
  assert.ok(title.has('end'), 'nldd-title heeft een end-slot');
  assert.ok(!title.has('actions'), 'nldd-title heeft géén actions-slot — dit is de bug die deze guard vangt');
  // A slot only the manifest names: the toolbar copies these into its menu.
  assert.ok(slots.get('nldd-toolbar').has('overflow'), 'nldd-toolbar heeft een overflow-slot');
  // En het component dat zijn slots pas bij het draaien benoemt.
  assert.equal(slots.get('nldd-bar-split-view'), null);
});
