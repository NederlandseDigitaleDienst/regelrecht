// @vitest-environment node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { createWhyServer, parseCliLine, passwordChecker } from './why.mjs';
import { buildRequest, MAX_TRACE_CHARS, systemPrompt } from './prompt.mjs';

// A stand-in for the Claude CLI: it echoes what it was asked in the stream-json
// shape the real one writes, so the tests see the whole route without a model.
// One script per mode, for the failures the server has to handle. Not an
// environment variable: the server hands the child a whitelist, so it would
// never arrive (and that the whitelist holds is worth knowing too).
const FAKE_CLI = `#!/usr/bin/env node
if (__MODE__ === 'deaf') process.exit(1);
let input = '';
process.stdin.on('data', (c) => (input += c));
process.stdin.on('end', () => {
  const mode = __MODE__;
  if (mode === 'crash') { process.stderr.write('boom'); process.exit(3); }
  if (mode === 'cutoff') {
    console.log(JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { type: 'text_delta', text: 'Je hebt ' } } }));
    setTimeout(() => process.exit(1), 50);
    return;
  }
  if (mode === 'late') {
    // An error result followed, in the same chunk, by more text: the text
    // arrives after the server already gave up on this answer.
    const d = (t) => JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { type: 'text_delta', text: t } } });
    process.stdout.write(d('Je ') + '\\n' + JSON.stringify({ type: 'result', is_error: true, result: 'overloaded' }) + '\\n' + d('hebt') + '\\n');
    setTimeout(() => process.exit(1), 50);
    return;
  }
  if (mode === 'error') {
    console.log(JSON.stringify({ type: 'result', is_error: true, result: 'rate limited' }));
    return;
  }
  const words = ['Je ', 'hebt ', 'recht. '];
  for (const w of words) {
    console.log(JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { type: 'text_delta', text: w } } }));
  }
  console.log(JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { type: 'text_delta', text: 'args:' + process.argv.includes('--tools') + ':' + input.includes('Wet: Zorgtoeslag') } } }));
  console.log(JSON.stringify({ type: 'result', is_error: false, result: 'ignored because deltas came' }));
});
`;

let dir;
const fakeCli = {};
beforeAll(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'why-test-'));
  for (const mode of ['ok', 'crash', 'error', 'cutoff', 'deaf', 'late']) {
    fakeCli[mode] = path.join(dir, `claude-${mode}`);
    fs.writeFileSync(fakeCli[mode], FAKE_CLI.replaceAll('__MODE__', JSON.stringify(mode)), { mode: 0o755 });
  }
});
afterAll(() => fs.rmSync(dir, { recursive: true, force: true }));

async function withServer(options, fn) {
  const server = createWhyServer({ password: 'geheim', claudeBin: fakeCli.ok, wrongPasswordDelayMs: 0, ...options });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    return await fn(base);
  } finally {
    await new Promise((r) => server.close(r));
  }
}

const body = {
  locale: 'nl',
  law: { id: 'zorgtoeslagwet', name: 'Zorgtoeslag', service: 'Dienst Toeslagen' },
  outcome: [{ label: 'Hoogte zorgtoeslag', value: '€ 1.234,56' }],
  trace_text: 'zorgtoeslagwet\n  toetsingsinkomen = 2500000',
};
const post = (base, pathname, payload, password = 'geheim') =>
  fetch(`${base}${pathname}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...(password ? { 'x-demo-password': password } : {}) },
    body: JSON.stringify(payload ?? {}),
  });

describe('the password', () => {
  it('matches only itself', async () => {
    const matches = passwordChecker('geheim');
    expect(await matches('geheim')).toBe(true);
    expect(await matches('geheim2')).toBe(false);
    expect(await matches('')).toBe(false);
    expect(await matches(undefined)).toBe(false);
  });

  it('is required to build a server at all', () => {
    expect(() => createWhyServer({ password: '' })).toThrow(/password/);
  });

  it('is checked by /api/why/check', () =>
    withServer({}, async (base) => {
      expect((await post(base, '/api/why/check', {}, 'geheim')).status).toBe(204);
      expect((await post(base, '/api/why/check', {}, 'fout')).status).toBe(401);
      expect((await post(base, '/api/why/check', {}, null)).status).toBe(401);
    }));

  it('turns password checks away once too many run at once', () =>
    withServer({ maxConcurrentChecks: 0 }, async (base) => {
      expect((await post(base, '/api/why/check', {}, 'geheim')).status).toBe(429);
      expect((await post(base, '/api/why', body, 'geheim')).status).toBe(429);
    }));

  it('guards the explanation itself, not only the check', () =>
    withServer({}, async (base) => {
      expect((await post(base, '/api/why', body, 'fout')).status).toBe(401);
    }));
});

describe('the explanation', () => {
  it('reports that it is available', () =>
    withServer({}, async (base) => {
      const res = await fetch(`${base}/api/why`);
      expect(res.status).toBe(200);
      expect(await res.json()).toEqual({ available: true });
    }));

  it('streams the text the model types, with tools off and the law in the message', () =>
    withServer({}, async (base) => {
      const res = await post(base, '/api/why', body);
      expect(res.status).toBe(200);
      expect(res.headers.get('content-type')).toMatch(/text\/plain/);
      expect(await res.text()).toBe('Je hebt recht. args:true:true');
    }));

  it('refuses a body without a trace', () =>
    withServer({}, async (base) => {
      const res = await post(base, '/api/why', { ...body, trace_text: '' });
      expect(res.status).toBe(400);
    }));

  it('answers 502 when the CLI fails before writing anything', () =>
    withServer({ claudeBin: fakeCli.crash }, async (base) => {
      expect((await post(base, '/api/why', body)).status).toBe(502);
    }));

  it('answers 502 when the model reports an error', () =>
    withServer({ claudeBin: fakeCli.error }, async (base) => {
      const res = await post(base, '/api/why', body);
      expect(res.status).toBe(502);
      expect((await res.json()).error).toBe('rate limited');
    }));

  it('breaks the connection when the model stops halfway, so the reader sees an error', () =>
    withServer({ claudeBin: fakeCli.cutoff }, async (base) => {
      const res = await post(base, '/api/why', body);
      expect(res.status).toBe(200);
      await expect(res.text()).rejects.toThrow();
    }));

  it('ignores output that arrives after it gave up on the answer', () =>
    withServer({ claudeBin: fakeCli.late }, async (base) => {
      // The connection breaks, before or after the headers depending on timing.
      await expect(post(base, '/api/why', body).then((res) => res.text())).rejects.toThrow();
      expect((await fetch(`${base}/api/why`)).status).toBe(200);
    }));

  it('survives a CLI that dies before reading a large trace', () =>
    withServer({ claudeBin: fakeCli.deaf }, async (base) => {
      const big = { ...body, trace_text: 'x'.repeat(140_000) };
      for (let i = 0; i < 3; i++) expect((await post(base, '/api/why', big)).status).toBe(502);
      // Still listening: an EPIPE on the child's stdin used to exit the process.
      expect((await fetch(`${base}/api/why`)).status).toBe(200);
    }));

  it('turns a request away when the limit is already reached', () =>
    withServer({ maxConcurrent: 0 }, async (base) => {
      expect((await post(base, '/api/why', body)).status).toBe(429);
    }));
});

describe('parseCliLine', () => {
  it('reads a text delta, a result, and ignores the rest', () => {
    expect(parseCliLine(JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { type: 'text_delta', text: 'a' } } }))).toEqual({ text: 'a' });
    expect(parseCliLine(JSON.stringify({ type: 'result', is_error: false, result: 'b' }))).toEqual({ result: 'b', isError: false });
    expect(parseCliLine(JSON.stringify({ type: 'system' }))).toBeNull();
    expect(parseCliLine('not json')).toBeNull();
  });
});

describe('the prompt', () => {
  it('asks for the answer in the language of the demo', () => {
    expect(systemPrompt('nl')).not.toMatch(/Engels|Fries|Frans/);
    expect(systemPrompt('en')).toMatch(/Engels/);
    expect(systemPrompt('fy')).toMatch(/Fries/);
    expect(systemPrompt('fr')).toMatch(/Frans/);
  });

  it('falls back to Dutch for an unknown locale', () => {
    expect(buildRequest({ ...body, locale: 'de' }).locale).toBe('nl');
  });

  it('carries the law, the outcome and the trace', () => {
    const { message } = buildRequest(body);
    expect(message).toContain('Wet: Zorgtoeslag (zorgtoeslagwet)');
    expect(message).toContain('Uitvoerder: Dienst Toeslagen');
    expect(message).toContain('- Hoogte zorgtoeslag: € 1.234,56');
    expect(message).toContain('toetsingsinkomen = 2500000');
  });

  it('cuts off a runaway trace', () => {
    const { message } = buildRequest({ ...body, trace_text: 'x'.repeat(MAX_TRACE_CHARS + 10) });
    expect(message).toContain('weggelaten');
    expect(message.length).toBeLessThan(MAX_TRACE_CHARS + 1000);
  });

  it('refuses a body without a law name', () => {
    expect(buildRequest({ ...body, law: {} }).error).toMatch(/law.name/);
    expect(buildRequest(null).error).toBeTruthy();
  });
});
