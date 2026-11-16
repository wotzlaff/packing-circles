import { test } from "node:test";
import assert from "node:assert/strict";
import { timeFraction, fractionTime, replayDuration } from "../timeline.js";

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
  assert.equal(fractionTime(-1, 2), 0);
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
