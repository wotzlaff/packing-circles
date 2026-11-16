/** Browser-side validation and bounded recordings, independent of the optimizer. */
export const MAX_INPUT_BYTES = 16 * 1024;
export const MAX_IMPORT_BYTES = 8 * 1024 * 1024;
export const MAX_TRACE_BYTES = 4 * 1024 * 1024;
export const MAX_FRAMES = 512;
export const names = ['A · legacy', 'B · early penalty', 'B · tight penalty', 'B · short restarts'];
const encoder = new TextEncoder();
const sizes = new WeakMap();
function frameSize(snapshot) {
  if (!sizes.has(snapshot)) sizes.set(snapshot, bytes(snapshot));
  return sizes.get(snapshot);
}
export const bytes = value => encoder.encode(typeof value === 'string' ? value : JSON.stringify(value)).length;
const require = (condition, message) => { if (!condition) throw new Error(message); };

export function parseInput(text) {
  require(typeof text === 'string' && bytes(text) <= MAX_INPUT_BYTES, 'Input must contain at most 16 KiB.');
  const tokens = text.trim().split(/\s+/);
  require(/^\d+$/.test(tokens[0]), 'Begin with an integer circle count.');
  const count = Number(tokens[0]);
  require(count >= 1 && count <= 50, 'Choose between 1 and 50 circles.');
  require(tokens.length === count + 1, `Expected ${count} radii; found ${tokens.length - 1}.`);
  require(tokens.slice(1).every(token => /^[+]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/.test(token)), 'Radii must be positive decimal numbers.');
  const radii = tokens.slice(1).map(Number);
  require(radii.every(r => Number.isFinite(r) && r > 0), 'Every radius must be finite and positive.');
  return {radii, input: `${count}\n${radii.join(' ')}\n`, lowerBound: Math.hypot(...radii)};
}

export function checkGeometry(radius, points, radii) {
  require(Number.isFinite(radius) && radius > 0, 'Invalid enclosing radius.');
  require(Array.isArray(points) && points.length === radii.length, 'Circle count does not match the input.');
  const tolerance = 1e-9 * Math.max(1, radius, ...radii);
  for (let i = 0; i < radii.length; i++) {
    const point = points[i];
    require(Array.isArray(point) && point.length === 2 && point.every(Number.isFinite), 'Invalid circle coordinates.');
    const extent = Math.hypot(...point) + radii[i];
    require(Number.isFinite(extent) && extent <= radius + tolerance, 'A circle crosses the container.');
    for (let j = 0; j < i; j++) {
      const distance = Math.hypot(point[0] - points[j][0], point[1] - points[j][1]);
      const required = radii[i] + radii[j];
      require(Number.isFinite(distance) && Number.isFinite(required) && distance + tolerance >= required, 'Circles overlap.');
    }
  }
}

export function validateSnapshot(s, radii) {
  require(s && s.schema_version === 1 && s.feasible === true, 'Unsupported or infeasible snapshot.');
  require(Array.isArray(s.radii) && s.radii.length === radii.length && s.radii.every((r, i) => r === radii[i]), 'Recorded radii do not match the input.');
  checkGeometry(s.radius, s.points, radii);
  require(typeof s.output === 'string' && bytes(s.output) <= MAX_INPUT_BYTES, 'Invalid solution text.');
  const tokens = s.output.trim().split(/\s+/);
  require(tokens.length === 1 + 3 * radii.length && /^\d+\.\d{4}$/.test(tokens[0]), 'Invalid four-decimal solution format.');
  const points = radii.map((r, i) => {
    require(Number(tokens[1 + 3 * i]) === r, 'Solution radii changed order.');
    const pair = tokens.slice(2 + 3 * i, 4 + 3 * i);
    require(pair.every(t => /^-?\d+\.\d{4}$/.test(t)), 'Coordinates must have four decimals.');
    return pair.map(Number);
  });
  checkGeometry(Number(tokens[0]), points, radii);
  require(Number(tokens[0]) === Math.ceil(s.radius * 1e4) / 1e4, 'Exported radius does not match the snapshot.');
  s.points.forEach((point, i) => point.forEach((v, axis) => {
    require(Math.abs(v - points[i][axis]) <= 0.00005000001, 'Exported coordinates do not match the snapshot.');
  }));
  for (const key of ['evaluations', 'iterations', 'restarts', 'initialization_fallbacks', 'numerical_failures']) {
    require(Number.isSafeInteger(s[key]) && s[key] >= 0, `Invalid ${key} counter.`);
  }
  require(Number.isFinite(s.elapsed_seconds) && s.elapsed_seconds >= 0 && typeof s.finished === 'boolean', 'Invalid snapshot time or state.');
  require(['running', 'time_limit', 'evaluation_limit', 'iteration_limit', 'restart_limit', 'cancelled', 'trivial'].includes(s.termination), 'Unknown stopping reason.');
  require(s.finished === (s.termination !== 'running'), 'Snapshot stopping reason contradicts its state.');
  if (s.diagnostics != null) {
    const d = s.diagnostics;
    require(['objective', 'trial_radius', 'containment_weight', 'pairwise_weight', 'gradient_squared'].every(k => Number.isFinite(d[k])), 'Nonfinite optimization diagnostics.');
    require(d.containment_weight > 0 && d.pairwise_weight > 0 && d.gradient_squared >= 0, 'Invalid optimization weights.');
    require(Array.isArray(d.points) && d.points.length === radii.length && d.points.every(p => Array.isArray(p) && p.length === 2 && p.every(Number.isFinite)), 'Invalid trial coordinates.');
  }
  return s;
}

export function validateConfig(config, input, algorithm) {
  require(config && config.input === input && config.algorithm === algorithm, 'Run settings do not match the input or algorithm.');
  require(typeof config.seed === 'string' && /^\d{1,20}$/.test(config.seed) && BigInt(config.seed) <= 0xffffffffffffffffn, 'Seed must be a decimal unsigned 64-bit integer.');
  require(Number.isFinite(config.timeLimitSeconds) && config.timeLimitSeconds >= 0.1 && config.timeLimitSeconds <= 60, 'Budget must be between 0.1 and 60 seconds.');
  return config;
}

export function traceStats(runs) {
  return {traceBytes: runs.reduce((n, run) => n + run.snapshots.reduce((m, s) => m + frameSize(s), 0), 0),
    snapshotCount: runs.reduce((n, run) => n + run.snapshots.length, 0), dropped: runs.reduce((n, run) => n + run.dropped, 0)};
}

/** Keep the first and latest frame; disclose evicted intermediate history. */
export function appendSnapshot(runs, index, snapshot) {
  const run = runs[index];
  const last = run.snapshots.at(-1);
  if (last) {
    require(snapshot.elapsed_seconds >= last.elapsed_seconds && snapshot.radius <= last.radius, 'Progress must have monotonic time and feasible radius.');
    for (const key of ['evaluations', 'iterations', 'restarts', 'initialization_fallbacks', 'numerical_failures']) require(snapshot[key] >= last[key], 'Progress work counters decreased.');
  }
  run.snapshots.push(snapshot);
  while (run.snapshots.length > MAX_FRAMES) { run.snapshots.splice(1, 1); run.dropped++; }
  let stats = traceStats(runs);
  while (stats.traceBytes > MAX_TRACE_BYTES) {
    const candidate = runs.filter(r => r.snapshots.length > 2).sort((a, b) => b.snapshots.length - a.snapshots.length)[0];
    require(candidate, 'Recording exceeds the trace memory limit.');
    candidate.snapshots.splice(1, 1); candidate.dropped++;
    stats = traceStats(runs);
  }
}

export function parseRecording(text) {
  require(typeof text === 'string' && bytes(text) <= MAX_IMPORT_BYTES, 'Recording must be at most 8 MiB.');
  const data = JSON.parse(text);
  require(data.schema_version === 1 && data.kind === 'circle-packing-run', 'Unsupported recording format.');
  const instance = parseInput(data.input);
  require(data.input === instance.input, 'Recording input is not canonical.');
  require(Array.isArray(data.runs) && data.runs.length >= 1 && data.runs.length <= 2, 'Recording must contain one or two runs.');
  const runs = data.runs.map(run => {
    require(Number.isInteger(run.algorithm) && run.algorithm >= 0 && run.algorithm <= 3, 'Unknown algorithm.');
    validateConfig(run.config, data.input, run.algorithm);
    require(['complete', 'cancelled'].includes(run.status), 'Only finished recordings can be imported.');
    require(Array.isArray(run.snapshots) && run.snapshots.length >= 1 && run.snapshots.length <= MAX_FRAMES, 'Invalid history length.');
    require(Number.isSafeInteger(run.dropped) && run.dropped >= 0, 'Invalid history truncation count.');
    const checked = {name: names[run.algorithm], algorithm: run.algorithm, config: run.config, status: run.status, snapshots: [], dropped: run.dropped};
    for (const snapshot of run.snapshots) appendSnapshot([checked], 0, validateSnapshot(snapshot, instance.radii));
    require(checked.snapshots.at(-1).finished, 'A recording must end at a final snapshot.');
    require((checked.status === 'cancelled') === (checked.snapshots.at(-1).termination === 'cancelled'), 'Run status contradicts the final stopping reason.');
    return checked;
  });
  require(traceStats(runs).traceBytes <= MAX_TRACE_BYTES, 'Recording exceeds the 4 MiB history limit.');
  return {instance, runs, solver: data.solver ?? null, view: data.view};
}
