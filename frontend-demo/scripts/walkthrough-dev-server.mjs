/**
 * Dev-server endpoint that receives a walkthrough take from the recorder
 * (src/walkthrough/recorder.ts) and writes it to `.walkthrough/takes/<take>/`
 * at the repository root.
 *
 * Only in `vite serve`: the production image has no such endpoint and the
 * recorder is not in the bundle. Names are checked against a narrow pattern so
 * a request cannot write outside the take directory.
 */
import { appendFile, mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
export const TAKES_DIR = resolve(here, '..', '..', '.walkthrough', 'takes');
const NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,80}$/;
const FILES = new Set(['app.webm', 'cam.webm', 'events.json', 'meta.json']);

/** `/__walkthrough/takes/<take>/<file>` to its parts, or null. */
export function parseTakePath(url) {
  const m = /^\/__walkthrough\/takes\/([^/?]+)\/([^/?]+)(?:\?(.*))?$/.exec(url ?? '');
  if (!m) return null;
  const take = decodeURIComponent(m[1]);
  const file = decodeURIComponent(m[2]);
  if (!NAME.test(take) || take.includes('..') || !FILES.has(file)) return null;
  const mode = new URLSearchParams(m[3] ?? '').get('mode') === 'append' ? 'append' : 'replace';
  return { take, file, mode };
}

/** A chunk is four seconds of video; this is far above that and far below a full disk. */
export const MAX_BODY = 64 * 1024 * 1024;

function readBody(req, limit = MAX_BODY) {
  return new Promise((ok, fail) => {
    const parts = [];
    let size = 0;
    req.on('data', (c) => {
      size += c.length;
      if (size > limit) {
        req.destroy();
        fail(new Error('too large'));
        return;
      }
      parts.push(c);
    });
    req.on('end', () => ok(Buffer.concat(parts)));
    req.on('error', fail);
  });
}

/** Only this machine. `just dev-demo` listens on every interface, for a talk. */
export function isLoopback(address) {
  return ['127.0.0.1', '::1', '::ffff:127.0.0.1'].includes(address);
}

/**
 * The plugin, or nothing. It is on only when `just walkthrough record` sets
 * WALKTHROUGH_RECORD: the dev server that runs the demo during a talk, on
 * conference wifi, has no endpoint that writes to disk.
 */
export function walkthroughDevServer({ dir = TAKES_DIR, enabled = !!process.env.WALKTHROUGH_RECORD } = {}) {
  if (!enabled) return null;
  return {
    name: 'regelrecht-walkthrough-takes',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        if (!req.url?.startsWith('/__walkthrough/')) return next();
        const target = req.method === 'POST' ? parseTakePath(req.url) : null;
        // The custom header cannot be sent cross-site without a preflight,
        // and nothing here answers one: a page elsewhere cannot write takes.
        if (!target || !isLoopback(req.socket?.remoteAddress) || req.headers['x-walkthrough'] !== '1') {
          res.statusCode = 400;
          res.end('bad request');
          return;
        }
        try {
          const body = await readBody(req);
          const takeDir = join(dir, target.take);
          await mkdir(takeDir, { recursive: true });
          const path = join(takeDir, target.file);
          if (target.mode === 'append') await appendFile(path, body);
          else await writeFile(path, body);
          res.statusCode = 204;
          res.end();
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e?.message ?? e));
        }
      });
    },
  };
}
