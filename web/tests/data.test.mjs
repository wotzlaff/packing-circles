import test from 'node:test';
import assert from 'node:assert/strict';
import {appendSnapshot, bytes, MAX_FRAMES, MAX_IMPORT_BYTES, MAX_INPUT_BYTES, MAX_TRACE_BYTES, parseInput, parseRecording, traceStats, validateSnapshot} from '../data.js';
import {presets} from '../presets.js';

const instance = parseInput(presets[10].input);
const snapshot = () => structuredClone(presets[10].preview);
const recording = () => ({schema_version: 1, kind: 'circle-packing-run', input: instance.input, runs: [{
  algorithm: 0, config: {input: instance.input, algorithm: 0, seed: '42', timeLimitSeconds: 1},
  status: 'complete', dropped: 0, snapshots: [snapshot()],
}]});
const read = value => parseRecording(JSON.stringify(value));

test('input is canonicalized and rejects bad counts and radii', () => {
  assert.equal(parseInput(' 3\n1.0 2e0  3  ').input, '3\n1 2 3\n');
  for (const bad of ['', '0', '51 ' + '1 '.repeat(51), '2 1', '1 0', '1 -1', '1 NaN', '1 Infinity', '1 2 3', '1 ' + ' '.repeat(MAX_INPUT_BYTES)]) {
    assert.throws(() => parseInput(bad), undefined, bad.slice(0, 40));
  }
});

test('imports a real portable result, enforcing config and input bounds', () => {
  assert.equal(read(recording()).runs[0].snapshots[0].output, snapshot().output);
  for (const change of [
    r => { r.input += ' '; },
    r => { r.runs[0].config.seed = '18446744073709551616'; },
    r => { r.runs[0].config.timeLimitSeconds = 60.1; },
    r => { r.runs[0].algorithm = 4; },
    r => { r.runs[0].config.input = '1\n1\n'; },
    r => { r.runs[0].status = 'running'; },
    r => { r.runs[0].snapshots[0].finished = false; },
    r => { r.runs[0].dropped = -1; },
  ]) {
    const r = recording(); change(r); assert.throws(() => read(r));
  }
  assert.throws(() => parseRecording(' '.repeat(MAX_IMPORT_BYTES + 1)));
});

test('rejects forged geometry and mismatched rounded solution text', () => {
  for (const change of [
    s => { s.points[0] = [...s.points[1]]; },
    s => { s.points[0] = [s.radius * 2, 0]; },
    s => { s.points[0][0] = Infinity; },
    s => { s.radii[0]++; },
    s => { s.output = s.output.replace('221.5004', '221.5003'); },
    s => { s.output = s.output.replace('-17.9341', '-17.9340'); },
    s => { s.output = s.output.replace('23 -17.9341', '24 -17.9341'); },
  ]) {
    const s = snapshot(); change(s); assert.throws(() => validateSnapshot(s, instance.radii));
  }
});

test('rejects invalid work and time measurements and backwards progress', () => {
  for (const change of [
    s => { s.evaluations = -1; }, s => { s.iterations = 1.5; },
    s => { s.restarts = Number.MAX_SAFE_INTEGER + 1; },
    s => { s.numerical_failures = Infinity; }, s => { s.elapsed_seconds = -1; },
    s => { s.termination = 'unknown'; },
  ]) {
    const s = snapshot(); change(s); assert.throws(() => validateSnapshot(s, instance.radii));
  }
  for (const change of [s => { s.elapsed_seconds = 0; }, s => { s.evaluations--; }, s => { s.radius += 1; }]) {
    const run = {snapshots: [snapshot()], dropped: 0};
    const next = snapshot(); change(next); assert.throws(() => appendSnapshot([run], 0, next));
  }
});

test('retains first and latest frames and accounts for bounded history', () => {
  const run = {snapshots: [], dropped: 0};
  for (let i = 0; i < MAX_FRAMES + 7; i++) {
    const s = snapshot(); s.elapsed_seconds = i; s.evaluations += i;
    appendSnapshot([run], 0, s);
  }
  assert.equal(run.snapshots.length, MAX_FRAMES);
  assert.equal(run.snapshots[0].elapsed_seconds, 0);
  assert.equal(run.snapshots.at(-1).elapsed_seconds, MAX_FRAMES + 6);
  assert.equal(run.dropped, 7);
  assert.deepEqual(traceStats([run]), {traceBytes: run.snapshots.reduce((sum, s) => sum + bytes(s), 0), snapshotCount: MAX_FRAMES, dropped: 7});
  assert.ok(traceStats([run]).traceBytes <= MAX_TRACE_BYTES);
});

test('allows finite infeasible trial geometry but rejects malformed diagnostics', () => {
  const s = snapshot();
  s.diagnostics = {objective: 1.5, trial_radius: -1, containment_weight: 1e-7, pairwise_weight: 1e-5, gradient_squared: 0, points: instance.radii.map(() => [0, 0])};
  assert.equal(validateSnapshot(s, instance.radii), s);
  for (const change of [
    d => { d.objective = Infinity; }, d => { d.trial_radius = NaN; },
    d => { d.containment_weight = 0; }, d => { d.pairwise_weight = -1; },
    d => { d.gradient_squared = -1; }, d => { d.points.pop(); },
    d => { d.points[0][1] = Infinity; },
  ]) {
    const malformed = structuredClone(s); change(malformed.diagnostics);
    assert.throws(() => validateSnapshot(malformed, instance.radii));
  }
});
