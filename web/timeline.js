const EARLY_UNIT = 0.001;
const clamp = (value, min, max) => Math.max(min, Math.min(max, value));

/** Map elapsed seconds onto the shared timeline. */
export function timeFraction(t, maxTime, scale = "early") {
  if (!Number.isFinite(maxTime) || maxTime <= 0) return 0;
  const time = Number.isFinite(t) ? clamp(t, 0, maxTime) : 0;
  return scale === "linear"
    ? time / maxTime
    : Math.log1p(time / EARLY_UNIT) / Math.log1p(maxTime / EARLY_UNIT);
}

/** Recover elapsed seconds from a timeline fraction. */
export function fractionTime(f, maxTime, scale = "early") {
  if (!Number.isFinite(maxTime) || maxTime <= 0) return 0;
  const fraction = Number.isFinite(f) ? clamp(f, 0, 1) : 0;
  return scale === "linear"
    ? fraction * maxTime
    : EARLY_UNIT * Math.expm1(fraction * Math.log1p(maxTime / EARLY_UNIT));
}

/** Playback duration in seconds, with a minimum eight-second recording. */
export function replayDuration(maxTime, pace = 1, checkpoints = 0) {
  const time = Number.isFinite(maxTime) ? Math.max(0, maxTime) : 0;
  const speed = Number.isFinite(pace) && pace > 0 ? pace : 1;
  const count = Number.isFinite(checkpoints) ? Math.max(0, checkpoints) : 0;
  return Math.max(8, time, count * 0.1) / speed;
}
