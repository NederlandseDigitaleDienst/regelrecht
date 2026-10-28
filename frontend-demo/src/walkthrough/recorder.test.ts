import { describe, expect, it } from 'vitest';
import { newTakeId } from './recorder';
import { parseTakePath } from '../../scripts/walkthrough-dev-server.mjs';
import { mediaFiles } from '../../scripts/fetch-walkthrough-media.mjs';

describe('recorder helpers', () => {
  it('names a take by its local time, safe as a directory', () => {
    expect(newTakeId(new Date(2026, 9, 2, 9, 5, 7))).toBe('2026-10-02T09-05-07');
  });
});

describe('camera test', () => {
  it('judges the light from the mean brightness', async () => {
    const { lightVerdict, meanLuma } = await import('./camCheck');
    expect(meanLuma(new Uint8ClampedArray([100, 100, 100, 255, 100, 100, 100, 255]))).toBe(100);
    expect(lightVerdict(30)).toBe('recorder.cam.dark');
    expect(lightVerdict(120)).toBe('recorder.cam.good');
    expect(lightVerdict(230)).toBe('recorder.cam.bright');
  });
});

describe('microphone test', () => {
  it('judges a speaking level the way the pipeline check does', async () => {
    const { levelPercent, levelVerdict } = await import('./micCheck');
    // The first real take peaked at -15 dB but its voice sat around -40: the
    // level, not the peak, is what "too quiet" has to catch.
    expect(levelVerdict(-40, -15)).toBe('recorder.mic.quiet');
    expect(levelVerdict(-20, -6)).toBe('recorder.mic.good');
    expect(levelVerdict(-70)).toBe('recorder.mic.silent');
    expect(levelVerdict(-20, -0.5)).toBe('recorder.mic.loud');
    expect(levelPercent(-60)).toBe(0);
    expect(levelPercent(0)).toBe(100);
  });
});

describe('take endpoint', () => {
  it('accepts the four files of a take, appending or replacing', () => {
    expect(parseTakePath('/__walkthrough/takes/2026-10-02T09-05-07/app.webm?mode=append')).toEqual({ take: '2026-10-02T09-05-07', file: 'app.webm', mode: 'append' });
    expect(parseTakePath('/__walkthrough/takes/t1/events.json')?.mode).toBe('replace');
  });

  it('refuses a path that could write outside the take directory', () => {
    expect(parseTakePath('/__walkthrough/takes/../app.webm')).toBeNull();
    expect(parseTakePath('/__walkthrough/takes/%2E%2E/app.webm')).toBeNull();
    expect(parseTakePath('/__walkthrough/takes/t1/..%2Fx.webm')).toBeNull();
    expect(parseTakePath('/__walkthrough/takes/t1/script.js')).toBeNull();
  });
});

describe('components', () => {
  // The production build leaves the recorder panel out (it is dev-only), so
  // nothing else would notice a panel that no longer compiles.
  it('compiles the recorder panel and the player', async () => {
    expect((await import('./RecorderPanel.vue')).default).toBeTruthy();
    expect((await import('./WalkthroughView.vue')).default).toBeTruthy();
  });
});

describe('media fetch', () => {
  it('lists every video and bubble the timeline names, with its checksum', () => {
    const timeline = {
      main: { video: { src: 'main-a.mp4', sha256: 'a', width: 1 }, cam: { src: 'main-cam-b.mp4', sha256: 'b' } },
      faq: [{ video: { src: 'faq-c.mp4', sha256: 'c' }, cam: null }],
    };
    expect(mediaFiles(timeline)).toEqual([
      { src: 'main-a.mp4', sha256: 'a' },
      { src: 'main-cam-b.mp4', sha256: 'b' },
      { src: 'faq-c.mp4', sha256: 'c' },
    ]);
  });

  it('fetches the short lines too, but not for the replay check', () => {
    const timeline = {
      main: { audio: { src: 'main-voice.m4a', sha256: 'v' } },
      bumpers: { wait: [{ src: 'bumper-wait-a.m4a', sha256: 'w' }], resume: [{ src: 'bumper-resume-b.m4a', sha256: 'r' }] },
    };
    expect(mediaFiles(timeline).map((f) => f.src)).toEqual(['main-voice.m4a', 'bumper-wait-a.m4a', 'bumper-resume-b.m4a']);
    expect(mediaFiles(timeline, { audioOnly: true }).map((f) => f.src)).toEqual(['main-voice.m4a']);
  });

  it('fetches the dubbed voices and their lines, but not for the replay check', () => {
    const timeline = {
      main: { audio: { src: 'main-voice.m4a', sha256: 'v' }, dub: { en: { audio: { src: 'main-voice-en.m4a', sha256: 'e' }, captions: 'main-x.en.dub.vtt' } } },
      dub: { en: { bumpers: { wait: [{ src: 'bumper-wait-en.m4a', sha256: 'w' }] } } },
    };
    expect(mediaFiles(timeline).map((f) => f.src)).toEqual(['main-voice.m4a', 'main-voice-en.m4a', 'bumper-wait-en.m4a']);
    expect(mediaFiles(timeline, { audioOnly: true }).map((f) => f.src)).toEqual(['main-voice.m4a']);
  });
});
