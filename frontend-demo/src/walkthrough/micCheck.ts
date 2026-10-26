/**
 * The microphone test in the recorder panel: which microphone, and how loud.
 *
 * A take that is too quiet can be turned up afterwards, but the background
 * noise comes up with it; the first real test take came in at -43 LUFS
 * because the input level was low. So before a take, the panel shows the
 * level live with a plain verdict, and lets the presenter pick the input.
 */
import { reactive } from 'vue';

/** An input device as the panel lists it. */
export interface MediaInput {
  id: string;
  label: string;
}

export const mic = reactive({
  devices: [] as MediaInput[],
  deviceId: '',
  testing: false,
  /**
   * The speech level: the loudest 50 ms RMS over the last half second, in
   * dBFS. Not the peak: the first real take peaked at -15 dB (a click, a
   * bump) while the voice itself sat around -40, far too quiet.
   */
  levelDb: -100,
  /** The peak over the same half second, only to catch distortion. */
  peakDb: -100,
});

let stream: MediaStream | null = null;
let ctx: AudioContext | null = null;
let timer: ReturnType<typeof setInterval> | 0 = 0;

/** A verdict on the speech level (and the peak) while speaking: the key of its text. */
export function levelVerdict(levelDb: number, peakDb = -100) {
  if (peakDb > -1.5) return 'recorder.mic.loud';
  if (levelDb < -55) return 'recorder.mic.silent';
  if (levelDb < -32) return 'recorder.mic.quiet';
  if (levelDb > -10) return 'recorder.mic.loud';
  return 'recorder.mic.good';
}

/** The meter's fill, 0 to 100, from -60 to 0 dBFS. */
export function levelPercent(db: number) {
  return Math.max(0, Math.min(100, Math.round(((db + 60) / 60) * 100)));
}

export async function listInputs() {
  const all = (await navigator.mediaDevices?.enumerateDevices?.()) ?? [];
  mic.devices = all.filter((d) => d.kind === 'audioinput').map((d) => ({ id: d.deviceId, label: d.label || d.deviceId.slice(0, 8) }));
  if (!mic.deviceId && mic.devices.length) mic.deviceId = mic.devices.find((d) => d.id === 'default')?.id ?? mic.devices[0]!.id; // length checked
}

/** The constraints a take records with: the chosen input, untouched. */
export function micConstraints(): MediaTrackConstraints {
  return {
    ...(mic.deviceId ? { deviceId: { exact: mic.deviceId } } : {}),
    echoCancellation: false,
    noiseSuppression: false,
    autoGainControl: false,
    channelCount: 1,
    sampleRate: 48000,
  };
}

export async function startTest() {
  stopTest();
  stream = await navigator.mediaDevices.getUserMedia({ audio: micConstraints() });
  // Labels are only filled in once a page has microphone permission.
  await listInputs();
  ctx = new AudioContext();
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 2048;
  ctx.createMediaStreamSource(stream).connect(analyser);
  const buf = new Float32Array(analyser.fftSize);
  let windows: { rms: number; peak: number }[] = [];
  const db = (v: number) => (v > 0 ? Math.round(20 * Math.log10(v)) : -100);
  mic.testing = true;
  timer = setInterval(() => {
    analyser.getFloatTimeDomainData(buf);
    let peak = 0;
    let sum = 0;
    for (const v of buf) {
      peak = Math.max(peak, Math.abs(v));
      sum += v * v;
    }
    windows = [...windows.slice(-9), { rms: Math.sqrt(sum / buf.length), peak }];
    mic.levelDb = db(Math.max(...windows.map((w) => w.rms)));
    mic.peakDb = db(Math.max(...windows.map((w) => w.peak)));
  }, 50);
}

export function stopTest() {
  clearInterval(timer);
  stream?.getTracks().forEach((t) => t.stop());
  ctx?.close();
  stream = null;
  ctx = null;
  mic.testing = false;
  mic.levelDb = -100;
  mic.peakDb = -100;
}
