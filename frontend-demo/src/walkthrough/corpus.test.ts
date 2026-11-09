import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { validTimeline } from './timeline';
import { mediaFiles } from '../../scripts/fetch-walkthrough-media.mjs';

// The committed walkthrough, if there is one. A timeline without a release,
// or with a video that has no checksum, would only fail in the Docker build,
// after review; here it fails on the commit that introduces it.
const dir = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'corpus', 'demo', 'walkthrough');
const path = resolve(dir, 'timeline.json');

describe.skipIf(!existsSync(path))('committed walkthrough', () => {
  const timeline = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : null;

  it('is a playable timeline', () => {
    expect(validTimeline(timeline)).toBe(true);
  });

  it('names the release its videos come from', () => {
    expect(timeline.release, 'timeline.json zonder `release`: de Docker-build kan de video\'s nergens ophalen').toMatch(/^[A-Za-z0-9._-]+$/);
  });

  it('pins every video by checksum', () => {
    for (const f of mediaFiles(timeline)) expect(f.sha256, f.src).toMatch(/^[0-9a-f]{64}$/);
  });

  it('ships the captions it names', () => {
    for (const t of [timeline.main, ...(timeline.faq ?? [])]) {
      const vtt = t.captions?.nl;
      if (vtt) expect(existsSync(resolve(dir, vtt)), vtt).toBe(true);
      // The captions of a dubbed voice say what that voice says.
      for (const d of Object.values<{ captions: string }>(t.dub ?? {})) expect(existsSync(resolve(dir, d.captions)), d.captions).toBe(true);
    }
  });
});
