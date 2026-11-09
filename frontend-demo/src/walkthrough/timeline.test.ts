import { describe, expect, it } from 'vitest';
import {
  chapterAt,
  cueAt,
  dubOf,
  fitRect,
  followerCorrection,
  formatTime,
  loadTimeline,
  nextChapterStart,
  offeredFaq,
  parseVtt,
  previousChapterStart,
  profileAt,
  ripplesAt,
  validTimeline,
} from './timeline';
import type { FetchLike } from './timeline';

/** A fetch that answers with `res`; a stub without `json` is only used where the code does not read it. */
const respond =
  (res: { ok: boolean; json?: () => Promise<unknown> }): FetchLike =>
  async () =>
    res as Awaited<ReturnType<FetchLike>>;

const track = {
  duration: 100,
  audio: { src: 'main.m4a' },
  video: { src: 'main.mp4', width: 1600, height: 1000 },
  events: [],
  cam: null,
  captions: {},
  chapters: [
    { start: 0, end: 10, slideIndex: 0, slide: { kind: 'title' } },
    { start: 10, end: 40, slideIndex: 1, slide: { route: '/wetten' }, profile: 'merijn' },
    { start: 40, end: 100, slideIndex: 2, slide: { route: '/portaal' } },
  ],
  clicks: [{ t: 12, x: 0.5, y: 0.5 }],
};

describe('chapters', () => {
  it('finds the chapter that plays at a moment', () => {
    expect(chapterAt(track, 0)).toBe(0);
    expect(chapterAt(track, 9.99)).toBe(0);
    expect(chapterAt(track, 10)).toBe(1);
    expect(chapterAt(track, 250)).toBe(2);
  });

  it('goes back to the start of the chapter, or to the one before right after a boundary', () => {
    expect(previousChapterStart(track, 30)).toBe(10);
    expect(previousChapterStart(track, 11)).toBe(0);
    expect(previousChapterStart(track, 1)).toBe(0);
  });

  it('has no next chapter after the last', () => {
    expect(nextChapterStart(track, 5)).toBe(10);
    expect(nextChapterStart(track, 50)).toBeNull();
  });

  it('carries the persona of an earlier chapter forward', () => {
    expect(profileAt(track, 2)).toBe('merijn');
    expect(profileAt(track, 0)).toBeNull();
  });
});

describe('faq', () => {
  const timeline = { faq: [{ id: 'b', offer: 50 }, { id: 'a', offer: 20 }, { id: 'c' }] };

  it('offers a question once its moment has passed, in order of appearance', () => {
    expect(offeredFaq(timeline, 10).map((f) => f.id)).toEqual([]);
    expect(offeredFaq(timeline, 60).map((f) => f.id)).toEqual(['a', 'b']);
  });
});

describe('captions', () => {
  const vtt = `WEBVTT

1
00:00:01.000 --> 00:00:03.500
Wetten worden al jaren

NOTE a comment

00:03.500 --> 00:00:05.000 line:90%
met de hand vertaald.
`;

  it('reads the cues post-processing writes', () => {
    const cues = parseVtt(vtt);
    expect(cues).toEqual([
      { start: 1, end: 3.5, text: 'Wetten worden al jaren' },
      { start: 3.5, end: 5, text: 'met de hand vertaald.' },
    ]);
  });

  it('shows the cue that covers the moment and nothing between cues', () => {
    const cues = parseVtt(vtt);
    expect(cueAt(cues, 0.5)).toBeNull();
    expect(cueAt(cues, 3.5)?.text).toBe('met de hand vertaald.');
    expect(cueAt(cues, 5)).toBeNull();
  });
});

describe('ripples and layout', () => {
  it('shows a ripple only shortly after the click', () => {
    expect(ripplesAt(track.clicks, 11.9)).toHaveLength(0);
    expect(ripplesAt(track.clicks, 12.3)).toHaveLength(1);
    expect(ripplesAt(track.clicks, 13)).toHaveLength(0);
  });

  it('places the picture inside a letterboxed element', () => {
    expect(fitRect(1000, 1000, 1600, 1000)).toEqual({ left: 0, top: 187.5, width: 1000, height: 625 });
    expect(fitRect(2000, 1000, 1600, 1000)).toEqual({ left: 200, top: 0, width: 1600, height: 1000 });
  });
});

describe('follower sync', () => {
  it('leaves a follower alone when it is in step', () => {
    expect(followerCorrection(10, 10.01)).toEqual({ rate: 1 });
  });
  it('nudges the rate for a small drift, in the right direction', () => {
    expect(followerCorrection(10, 10.1).rate).toBeLessThan(1);
    expect(followerCorrection(10, 9.9).rate).toBeGreaterThan(1);
    expect(followerCorrection(10, 10.1, 1.5).rate).toBeCloseTo(1.425);
  });
  it('jumps for a large drift', () => {
    expect(followerCorrection(10, 12)).toEqual({ seek: 10, rate: 1 });
  });
});

describe('loading', () => {
  it('treats a missing recording as none, not as an error', async () => {
    expect(await loadTimeline(respond({ ok: false }))).toBeNull();
    expect(await loadTimeline(async () => { throw new Error('offline'); })).toBeNull();
  });

  it('rejects a timeline without a playable main track', async () => {
    expect(validTimeline({ version: 1, main: { ...track, chapters: [] } })).toBe(false);
    expect(validTimeline({ version: 1, main: track, faq: [{ id: 'x' }] })).toBe(false);
    expect(await loadTimeline(async () => ({ ok: true, json: async () => ({ version: 1, main: track }) }))).not.toBeNull();
  });
});

it('formats a running time', () => {
  expect(formatTime(0)).toBe('0:00');
  expect(formatTime(75.9)).toBe('1:15');
  expect(formatTime(3725)).toBe('1:02:05');
});

describe('pauses', () => {
  it('stops at a pause the clock runs into, not at one it jumps past', async () => {
    const { pauseDue } = await import('./timeline');
    const pauses = [{ t: 10 }, { t: 20, hint: 'Klik op een wet' }];
    expect(pauseDue(pauses, 9.98, 10.01)).toEqual({ t: 10 });
    expect(pauseDue(pauses, 19.99, 20.0)).toEqual({ t: 20, hint: 'Klik op een wet' });
    // Resumed at the pause itself: not again.
    expect(pauseDue(pauses, 10, 10.02)).toBeNull();
    // A seek over it.
    expect(pauseDue(pauses, 5, 15)).toBeNull();
    // Backwards.
    expect(pauseDue(pauses, 10.01, 9.98)).toBeNull();
    expect(pauseDue(undefined, 9, 10)).toBeNull();
  });

  it('picks one of the lines at random, or none', async () => {
    const { pickClip } = await import('./timeline');
    const clips = [{ src: 'a' }, { src: 'b' }];
    expect(pickClip(clips, () => 0)).toEqual({ src: 'a' });
    expect(pickClip(clips, () => 0.99)).toEqual({ src: 'b' });
    expect(pickClip([], () => 0)).toBeNull();
    expect(pickClip(undefined)).toBeNull();
  });
});

describe('dubOf', () => {
  const dub = { audio: { src: 'main-voice-en.m4a', sha256: 'e' }, captions: 'main-x.en.dub.vtt' };
  it("gives the cloned voice in the viewer's language", () => {
    expect(dubOf({ dub: { en: dub } }, 'en')).toBe(dub);
  });
  it('gives none in a language without one, or when the viewer chose the original', () => {
    expect(dubOf({ dub: { en: dub } }, 'fy')).toBeNull();
    expect(dubOf({ dub: { en: dub } }, 'en', true)).toBeNull();
    expect(dubOf({}, 'en')).toBeNull();
    expect(dubOf(null, 'en')).toBeNull();
  });
});
