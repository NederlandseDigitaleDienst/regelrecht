// The browser jobs in ci.yml run inside the Playwright image
// (mcr.microsoft.com/playwright), which ships one browser build per Playwright
// release under /ms-playwright. The `playwright` package from the lockfile
// looks there for exactly the revision it was released with. When the two
// drift apart, the jobs fail with "Executable doesn't exist" (or, worse,
// quietly run a browser the lockfile never asked for), and Dependabot bumps
// the lockfile without touching the image tag. This test binds them.
//
// Node's built-in runner, no dependency: this runs in the Pre-commit job, which
// has Node but no node_modules.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const read = (path) => readFileSync(join(root, path), 'utf8');

const WORKFLOW = '.github/workflows/ci.yml';
// Every lockfile a job in that image installs Playwright from: the root
// workspace (editor e2e, walkthrough replay) and the docs site (mermaid
// rendering during the build).
const LOCKFILES = ['package-lock.json', 'docs/package-lock.json'];

const IMAGE_REF = /mcr\.microsoft\.com\/playwright:([^\s"']+)/g;
const PINNED = /^v(\d+\.\d+\.\d+)-noble@sha256:[0-9a-f]{64}$/;

const imageRefs = [...read(WORKFLOW).matchAll(IMAGE_REF)].map((m) => m[1]);

/** Every playwright-core version a lockfile resolves, nested copies included. */
function lockedVersions(lockfile) {
  const packages = JSON.parse(read(lockfile)).packages ?? {};
  const versions = Object.entries(packages)
    .filter(([path]) => path === 'node_modules/playwright-core' || path.endsWith('/node_modules/playwright-core'))
    .map(([, entry]) => entry.version);
  assert.ok(versions.length > 0, `${lockfile} resolves no playwright-core`);
  return [...new Set(versions)];
}

test('ci.yml runs at least one job in the Playwright image', () => {
  // Guards the guard: if the image disappears from ci.yml this file tests
  // nothing, and it should then be deleted on purpose rather than pass.
  assert.ok(imageRefs.length > 0, `no mcr.microsoft.com/playwright image in ${WORKFLOW}`);
});

test('every Playwright image is pinned by version and digest', () => {
  for (const ref of imageRefs) {
    assert.match(
      ref,
      PINNED,
      `${ref} must look like vX.Y.Z-noble@sha256:<digest>; a moving tag can change under a green run`,
    );
  }
});

test('every lockfile resolves the same Playwright as the image', () => {
  const versions = new Set(imageRefs.map((ref) => PINNED.exec(ref)?.[1]));
  assert.equal(versions.size, 1, `ci.yml uses several Playwright images: ${[...versions].join(', ')}`);
  const [image] = versions;
  for (const lockfile of LOCKFILES) {
    const locked = lockedVersions(lockfile);
    assert.deepEqual(
      locked,
      [image],
      `${lockfile} resolves playwright-core ${locked.join(', ')}, the image in ${WORKFLOW} is v${image}. ` +
        'Keep both lockfiles on one version and move the image tag and digest to it. The digest is ' +
        "the Docker-Content-Digest header of `curl -sI -H 'Accept: application/vnd.oci.image.index.v1+json' " +
        `https://mcr.microsoft.com/v2/playwright/manifests/v${locked[0]}-noble\`.`,
    );
  }
});

test('no job in ci.yml downloads Playwright browsers itself', () => {
  // The image carries the browsers and their system libraries. A job that
  // still runs `playwright install` pays for the apt round-trip the image
  // exists to avoid.
  // Comments may still name the command to explain why it is gone.
  const code = read(WORKFLOW)
    .split('\n')
    .filter((line) => !/^\s*#/.test(line))
    .join('\n');
  assert.doesNotMatch(code, /playwright install/);
});

test('no job in ci.yml runs apt-get', () => {
  // apt in the image goes through the Azure Ubuntu mirror, which once took 22
  // minutes for a C compiler and pushed a merge-queue entry past its check
  // timeout. The engine is built on the runner by the wasm job, and the docs
  // accessibility gate installs its packages with script/install-a11y-debs.sh.
  const code = read(WORKFLOW)
    .split('\n')
    .filter((line) => !/^\s*#/.test(line))
    .join('\n');
  assert.doesNotMatch(code, /apt-get/);
});

test('the .debs for the docs accessibility gate were picked in the pinned image', () => {
  // script/install-a11y-debs.sh installs the exact packages apt chose in one
  // image. After an image bump those may no longer be what apt would pick, and
  // dpkg would install them anyway; this makes the bump look at the list.
  const script = read('script/install-a11y-debs.sh');
  const made = /^# Made for: mcr\.microsoft\.com\/playwright:(v\d+\.\d+\.\d+-noble)$/m.exec(script);
  assert.ok(made, 'script/install-a11y-debs.sh does not name the image its .debs were picked in');
  const images = new Set(imageRefs.map((ref) => ref.split('@')[0]));
  assert.deepEqual(
    [...images],
    [made[1]],
    `script/install-a11y-debs.sh lists .debs for ${made[1]}, ci.yml runs ${[...images].join(', ')}. ` +
      'Refresh the list as its header describes, then update the "Made for" line.',
  );
});
