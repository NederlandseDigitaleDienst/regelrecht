/**
 * Check that the recorded walkthrough still replays in the demo as it is now.
 *
 * The walkthrough is the presenter's voice plus a log of what he did; the
 * replay does it again in the live app. A change to the demo (a renamed
 * button, a moved panel, a law that is gone) can leave an action with nothing
 * to act on, and the walkthrough then shows something other than what the
 * voice says. This finds that before it ships: it opens /rondleiding on a dev
 * server in a headless, muted Chromium, jumps to the end of every chapter
 * (which applies all of its actions, fast) and reports each action whose
 * element was not found, per chapter, in two window sizes. A chapter with a
 * miss is replayed once at the recorded pace before it counts, so a slow
 * machine does not turn the check red. Every other language the walkthrough
 * plays in (`locales` in the timeline) is checked too, in the first window
 * size: there the replay finds each element through the demo's translations,
 * and a label without one shows up here as a miss.
 *
 *   --locale CODE   check only that language (while working on a translation)
 *   --shard K/N     check only every N-th (size, language) run, starting at
 *                   the K-th; CI splits the check over N jobs this way
 *
 *   node frontend-demo/scripts/verify-walkthrough.mjs [--url URL] [--size WxH ...]
 *
 * Without --url it starts the demo's dev server itself (the replay's test
 * hook, `window.__rrReplay`, exists only in a dev build) and fetches the
 * voice it needs from the release. No timeline.json: nothing to check.
 * Exit code 1 when anything was missed. Run by CI and by
 * `just walkthrough verify`.
 */
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { fetchMedia } from './fetch-walkthrough-media.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(here, '..');
const timelinePath = resolve(appRoot, '..', 'corpus', 'demo', 'walkthrough', 'timeline.json');
const PORT = 7413;

function args(argv) {
  const out = { url: null, sizes: [], locale: null, shard: [1, 1] };
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--url') out.url = argv[++i];
    else if (argv[i] === '--size') out.sizes.push(argv[++i]);
    else if (argv[i] === '--locale') out.locale = argv[++i];
    else if (argv[i] === '--shard') {
      const [k, n] = String(argv[++i]).split('/').map(Number);
      if (!(Number.isInteger(k) && Number.isInteger(n) && k >= 1 && k <= n)) {
        throw new Error(`--shard verwacht K/N met 1 <= K <= N, kreeg ${argv[i]}`);
      }
      out.shard = [k, n];
    }
  }
  if (!out.sizes.length) out.sizes = ['1600x1000', '1280x800'];
  return out;
}

async function waitFor(url, ms) {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    try {
      if ((await fetch(url)).ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`dev-server op ${url} kwam niet op`);
}

/** The dev server, with the predev steps (corpus copy, codegen) it needs. */
function startDevServer() {
  const child = spawn('npm', ['run', 'dev', '--', '--port', String(PORT), '--strictPort', '--host', '127.0.0.1'], {
    cwd: appRoot,
    stdio: 'ignore',
    detached: true,
  });
  return {
    url: `http://127.0.0.1:${PORT}`,
    stop: () => {
      try {
        process.kill(-child.pid);
      } catch {
        /* already gone */
      }
    },
  };
}

async function launch() {
  const { chromium } = await import('playwright');
  // Muted: a voice out of nowhere in someone's meeting is not a test result.
  const opts = { headless: true, args: ['--mute-audio'] };
  // On a laptop the installed Chrome is there already; CI runs in the
  // Playwright image, which carries Chromium.
  if (!process.env.CI) {
    try {
      return await chromium.launch({ ...opts, channel: 'chrome' });
    } catch {
      /* fall through to the bundled Chromium */
    }
  }
  return chromium.launch(opts);
}

async function verify(browser, base, timeline, size, locale = 'nl') {
  const [width, height] = size.split('x').map(Number);
  const page = await browser.newPage({ viewport: { width, height } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  const tracks = [['main', null, timeline.main], ...(timeline.faq ?? []).map((f) => [`faq-${f.id}`, f.id, f])];
  // The walkthrough's address in `locale`, from the app's own route table.
  const pathOf = async (faqId) => {
    if (locale === 'nl') return `/rondleiding${faqId ? `/${faqId}` : ''}`;
    if (!page.url().startsWith(base)) {
      await page.goto(`${base}/rondleiding`);
      await page.waitForFunction(() => window.__rrReplay?.replay.active, null, { timeout: 120_000 });
    }
    return page.evaluate(([code, id]) => window.__rrReplay.pathFor(code, id), [locale, faqId]);
  };
  const label = locale === 'nl' ? size : `${size}, ${locale}`;
  let missed = 0;
  for (const [name, faqId, track] of tracks) {
    await page.goto(`${base}${await pathOf(faqId)}`);
    await page.waitForFunction(() => window.__rrReplay?.replay.active, null, { timeout: 120_000 });
    const playing = await page.evaluate(() => window.__rrReplay.replay.demoLocale);
    if (playing !== locale) {
      console.log(`${name} (${label}): de demo speelt in ${playing}, niet in ${locale}`);
      missed += 1;
      continue;
    }
    console.log(`${name} (${label})`);
    let seen = 0;
    let seenLoose = 0;
    for (const [i, c] of track.chapters.entries()) {
      const end = Math.round((c.end - 0.3) * 1000) / 1000;
      await page.evaluate((t) => window.__rrReplay.seek(t, { play: false }), end);
      await page.waitForFunction((t) => Math.abs(window.__rrReplay.replay.now - t) < 0.05, end, { timeout: 300_000 });
      const readMisses = () => page.evaluate(() => window.__rrReplay.replay.misses.map((m) => ({ t: m.t, type: m.type })));
      let misses = await readMisses();
      let fresh = misses.slice(seen);
      // Fast, a click can come before the demo is ready for it (a request
      // that waits for the engine), and how often depends on the machine.
      // A chapter that missed something is played once more at the recorded
      // pace; only what is missed then counts.
      if (fresh.length) {
        seen = misses.length;
        await page.evaluate((t) => window.__rrReplay.seek(t, { play: false, careful: true }), end);
        await page.waitForFunction((t) => Math.abs(window.__rrReplay.replay.now - t) < 0.05, end, { timeout: 900_000 });
        misses = await readMisses();
        fresh = misses.slice(seen);
        console.log(`     (hoofdstuk ${i} opnieuw, op tempo van de opname)`);
      }
      seen = misses.length;
      missed += fresh.length;
      const loose = (await page.evaluate(() => window.__rrReplay.replay.loose.map((m) => ({ t: m.t, type: m.type, what: m.what })))).slice(seenLoose);
      seenLoose += loose.length;
      const path = await page.evaluate(() => location.pathname);
      const slide = `${c.slide?.title ?? `dia ${c.slideIndex + 1}`}${locale === 'nl' ? '' : ` [${locale}]`}`;
      const mark = fresh.length ? `${fresh.length} gemist: ${fresh.map((m) => `${m.type} @ ${m.t.toFixed(1)}s`).join(', ')}` : 'ok';
      console.log(`  ${i}. ${slide}  ${c.start.toFixed(0)}-${c.end.toFixed(0)}s  ${path}  ${mark}`);
      // Found, but only with the numbers in its description masked: probably
      // a count that moved, possibly something else that now carries those
      // words. Not a failure; a person should look.
      for (const m of loose) {
        console.log(`     let op: ${m.type} @ ${m.t.toFixed(1)}s vond "${m.what}" alleen losser`);
        if (process.env.GITHUB_ACTIONS) {
          console.log(`::warning title=Rondleiding: losse match::Hoofdstuk "${slide}" (${size}): ${m.type} op "${m.what}" vond zijn element alleen met de getallen weggelaten. Klopt het nog met wat de stem zegt?`);
        }
      }
      if (fresh.length && process.env.GITHUB_ACTIONS) {
        console.log(
          `::error title=Rondleiding speelt niet terug::Hoofdstuk "${slide}" (${size}): ${fresh.length} handeling(en) vonden hun element niet. ` +
            'Een wijziging in de demo heeft iets weggehaald of hernoemd waar de opname op klikt. Herstel dat in de demo, of neem dit hoofdstuk opnieuw op (zie de walkthrough-skill).',
        );
      }
    }
  }
  await page.close();
  for (const e of errors) console.log(`paginafout: ${e}`);
  return missed + errors.length;
}

async function main() {
  if (!existsSync(timelinePath)) {
    console.log('walkthrough: geen timeline.json, niets te controleren');
    return 0;
  }
  const { url, sizes, locale, shard } = args(process.argv.slice(2));
  const timeline = JSON.parse(readFileSync(timelinePath, 'utf8'));
  // The replay waits for the voice to load; the video it never plays.
  await fetchMedia({ audioOnly: true });
  const server = url ? null : startDevServer();
  const base = url ?? server.url;
  try {
    await waitFor(`${base}/rondleiding`, 120_000);
    const browser = await launch();
    const runs = [];
    if (!locale || locale === 'nl') for (const size of sizes) runs.push([size, 'nl']);
    const others = (timeline.locales ?? []).filter((l) => l !== 'nl' && (!locale || l === locale));
    for (const code of others) runs.push([sizes[0], code]);
    const [k, n] = shard;
    let failed = 0;
    for (const [i, [size, code]] of runs.entries()) {
      if (i % n === k - 1) failed += await verify(browser, base, timeline, size, code);
    }
    await browser.close();
    return failed ? 1 : 0;
  } finally {
    server?.stop();
  }
}

main().then(
  (code) => process.exit(code),
  (e) => {
    console.error(e.message);
    process.exit(1);
  },
);
