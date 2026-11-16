import { test } from "node:test";
import assert from "node:assert/strict";
import { radiusTickFormatter } from "../render.js";
import { timeFraction, fractionTime, replayDuration, windowStart, visibleSnapshots } from "../timeline.js";

test("timeline transforms invert elapsed times across recordings", () => {
  for (const max of [0.001, 0.1, 1, 60, 3600]) {
    for (const scale of ["early", "linear"]) {
      for (const time of [0, Math.min(0.001, max), max / 2, max]) {
        assert.ok(Math.abs(fractionTime(timeFraction(time, max, scale), max, scale) - time) < Math.max(1e-12, max * 1e-12));
      }
    }
  }
});

test("timeline handles bounds and empty recordings", () => {
  assert.equal(timeFraction(-1, 2), 0);
  assert.equal(timeFraction(3, 2), 1);
  assert.equal(fractionTime(-1, 2), 1);
  assert.ok(Math.abs(fractionTime(2, 2) - 2) < 1e-12);
  assert.equal(timeFraction(1, 0), 0);
  assert.equal(fractionTime(1, 0), 0);
  assert.equal(timeFraction(NaN, Infinity), 0);
  assert.equal(timeFraction(1, 2, "linear"), 0.5);
});

test("replay duration applies minimum and pace", () => {
  assert.equal(replayDuration(0), 8);
  assert.equal(replayDuration(2, 0.25), 32);
  assert.equal(replayDuration(12, 4), 3);
  assert.equal(replayDuration(NaN, NaN), 8);
  assert.equal(replayDuration(2, 1, 160), 16);
  assert.equal(replayDuration(2, 1, NaN), 8);
});


test("late window maps and inverts the last half of elapsed time", () => {
  assert.equal(windowStart(10), 5);
  assert.equal(windowStart(10, "early"), 0);
  assert.equal(timeFraction(0, 10), 0);
  assert.equal(timeFraction(5, 10), 0);
  assert.equal(timeFraction(7.5, 10), 0.5);
  assert.equal(timeFraction(10, 10), 1);
  assert.equal(fractionTime(0, 10), 5);
  assert.equal(fractionTime(0.5, 10), 7.5);
  assert.equal(fractionTime(1, 10), 10);
});

test("visible snapshots carry state without mutating recorded frames", () => {
  const frames = [{elapsed_seconds: 1, radius: 220}, {elapsed_seconds: 4, radius: 200}, {elapsed_seconds: 8, radius: 199.999999}];
  const visible = visibleSnapshots(frames, 5);
  assert.deepEqual(visible.map((s) => [s.elapsed_seconds, s.radius]), [[5, 200], [8, 199.999999]]);
  assert.equal(frames[1].elapsed_seconds, 4);
  assert.deepEqual(visibleSnapshots(frames.slice(0, 2), 5), []);
  assert.deepEqual(visibleSnapshots([{elapsed_seconds: 5, radius: 200}], 5).map((s) => s.radius), [200]);
});

test("radius tick precision distinguishes small refinements", () => {
  for (const span of [1e-4, 1e-6, 4e-10]) {
    const ticks = Array.from({length: 4}, (_, i) => 200 + span * i / 3);
    const format = radiusTickFormatter(span, ticks);
    assert.equal(new Set(ticks.map(format)).size, 4);
  }
});
