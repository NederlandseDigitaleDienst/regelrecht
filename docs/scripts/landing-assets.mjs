/*
 * Put the landing page's runtime assets in place before the site is built.
 *
 * The run panel executes the zorgtoeslag scenario in the visitor's browser, so
 * the build needs the laws, the scenario and the canonical-grammar runner
 * inside this project. script/landing-laws.sh assembles them out of the
 * repository; this wrapper is what `npm run build` calls, so that every way of
 * building the site gets them: CI's accessibility gate, the Docker image, and a
 * plain `npm run build` on a laptop.
 *
 * Wiring it here rather than in each of those places is the point. The assets
 * are gitignored build products, and a job that builds without them fails on a
 * missing import, which is exactly how the accessibility gate broke: it ran
 * `astro build` in a fresh checkout where docs/src/lib/gherkin did not exist.
 *
 * Two situations are not errors:
 *   - the image has already run the script, with the repository laid out
 *     differently (/app and /corpus), so the assets are there and the corpus is
 *     not where a checkout would have it;
 *   - someone builds the docs from a copy of docs/ alone.
 * Both are recognised by the assets already being present. Anything else fails
 * loudly: a site that builds without them is a site whose run panel is broken.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const docs = dirname(dirname(fileURLToPath(import.meta.url)));
const repo = dirname(docs);

const script = join(repo, 'script', 'landing-laws.sh');
// What the build imports and fetches; both have to exist for the run panel.
const produced = [join(docs, 'src', 'lib', 'gherkin', 'index.ts'), join(docs, 'public', 'laws', 'manifest.json')];
const ready = produced.every((p) => existsSync(p));

if (!existsSync(script)) {
  if (ready) {
    console.log('landing-assets: assets present, no script to run');
    process.exit(0);
  }
  console.error(
    `landing-assets: ${script} not found and the assets are missing.\n` +
      'The landing page cannot run the scenario without them.',
  );
  process.exit(1);
}

try {
  execFileSync('bash', [script], { stdio: 'inherit' });
} catch (err) {
  if (ready) {
    // Already assembled (the image does this in its own layout); a failure to
    // redo it here is not a reason to fail a build that has what it needs.
    console.log('landing-assets: assets present, keeping them');
    process.exit(0);
  }
  console.error(`landing-assets: ${script} failed (${err.status ?? err.message})`);
  process.exit(1);
}
