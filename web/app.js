import {PackingWorker} from './solver.js';
import {drawPacking, drawHistory} from './render.js';
import {presets} from './presets.js';
import {timeFraction, fractionTime, replayDuration} from './timeline.js';
import {parseInput, validateSnapshot, validateConfig, appendSnapshot, traceStats, parseRecording,
  MAX_INPUT_BYTES, MAX_IMPORT_BYTES, names} from './data.js';

const $ = id => document.getElementById(id);
const state = {phase: 'idle', runs: [], activeIndex: -1, selectedIndex: 0, selectedTime: 0, selectedFrameIndex: null};
let worker, ready = false, activeId = null, framePending = false, preview = null;
let instance, buildInfo = {version: '0.1.0', math_profile: 'portable-libm-0.2.16'};
let importedSolver = null, pendingControl = null, replayStart = null, generation = 0;
let replayCheckpoints = [], replayFraction = 0;
let inputError = false;
const performanceStats = {frames: [], controls: []};
const active = () => ['running', 'paused'].includes(state.phase);
const maximumTime = () => Math.max(0, ...state.runs.flatMap(r => r.snapshots.map(s => s.elapsed_seconds)));
const sample = (array, value) => { array.push(value); if (array.length > 2048) array.shift(); };

/** Read-only developer diagnostics used by the reproducible browser checks. */
export function getAppState() {
  return structuredClone({...state, runs: state.runs.map(({name, algorithm, config, status, snapshots, dropped}) =>
    ({name, algorithm, config, status, snapshots, dropped}))});
}
export function getPerformanceStats() {
  return {...structuredClone(performanceStats), ...traceStats(state.runs)};
}

function connectWorker() {
  const current = ++generation;
  ready = false;
  worker = new PackingWorker();
  worker.addEventListener('message', ({data}) => {
    if (current !== generation) return;
    if (data.type === 'ready') { ready = true; updateControls(); return; }
    if (data.type === 'error') {
      if (data.runId == null || data.runId === activeId) showError(new Error(data.error));
      return;
    }
    if (data.runId !== activeId || state.activeIndex < 0) return;
    try {
      const run = state.runs[state.activeIndex];
      for (const frame of data.history ?? []) recordFrame(state.activeIndex, frame);
      if (data.snapshot) validateSnapshot(data.snapshot, instance.radii);
      if (['progress', 'done'].includes(data.type)) recordFrame(state.activeIndex, data.snapshot);
      if (data.type === 'paused') {
        state.phase = 'paused'; run.status = 'paused';
        // The acknowledgement includes a current clock and possibly a final repair.
        recordFrame(state.activeIndex, data.snapshot);
      } else if (data.type === 'resumed') {
        state.phase = 'running'; run.status = 'running';
      }
      if (pendingControl && (data.type === pendingControl.type || data.type === 'done')) {
        sample(performanceStats.controls, performance.now() - pendingControl.at);
        pendingControl = null;
      }
      state.selectedTime = data.snapshot.elapsed_seconds;
      state.selectedFrameIndex = null;
      preview = null;
      if (data.type === 'done') {
        const cancelled = data.snapshot.termination === 'cancelled';
        run.status = cancelled ? 'cancelled' : 'complete';
        const next = state.activeIndex + 1;
        activeId = null;
        if (!cancelled && state.runs[next]?.status === 'queued') {
          startRun(next);
        } else {
          state.phase = 'complete'; state.activeIndex = -1;
          state.selectedTime = maximumTime();
          $('status').textContent = cancelled ? 'Cancelled. The best checked packing is retained.' : 'Experiment complete. Replay expands the early steps so you can watch the search.';
        }
      }
      updateControls(); scheduleRender();
    } catch (error) { showError(error); }
  });
}

function recordFrame(index, snapshot) {
  validateSnapshot(snapshot, instance.radii);
  const last = state.runs[index].snapshots.at(-1);
  if (last && last.elapsed_seconds === snapshot.elapsed_seconds && last.evaluations === snapshot.evaluations &&
      last.iterations === snapshot.iterations && last.radius === snapshot.radius && last.finished === snapshot.finished) return;
  appendSnapshot(state.runs, index, snapshot);
}

function showError(error) {
  activeId = null;
  if (worker && active()) worker.cancel();
  state.phase = 'error'; state.activeIndex = -1;
  state.runs.forEach(run => { if (['running', 'paused', 'queued'].includes(run.status)) run.status = 'error'; });
  $('error').textContent = error.message;
  $('status').textContent = 'The experiment could not be completed.';
  pendingControl = null; replayStart = null;
  updateControls(); scheduleRender();
}

function updateControls() {
  const busy = active();
  $('run').disabled = !ready || inputError || busy || state.phase === 'replaying';
  $('pause').disabled = state.phase !== 'running' || pendingControl != null;
  $('resume').disabled = state.phase !== 'paused' || pendingControl != null;
  $('cancel').disabled = !busy || pendingControl != null;
  for (const id of ['preset', 'input', 'upload', 'compare', 'variant', 'seed', 'budget', 'import-run']) $(id).disabled = busy || state.phase === 'replaying';
  $('variant').disabled ||= $('compare').value === 'a';
  $('timeline').disabled = busy || state.runs.every(r => !r.snapshots.length);
  $('replay').disabled = busy || state.runs.every(r => !r.snapshots.length);
  const frames = state.runs[state.selectedIndex]?.snapshots ?? [];
  const frameIndex = selectedFrameIndex();
  $('previous-frame').disabled = busy || frameIndex <= 0;
  $('next-frame').disabled = busy || frameIndex < 0 || frameIndex >= frames.length - 1;
  const selected = selectedSnapshot();
  $('export-solution').disabled = !selected || busy || preview != null;
  $('export-run').disabled = busy || !state.runs.some(r => ['complete', 'cancelled'].includes(r.status) && r.snapshots.at(-1)?.finished);
  $('replay').textContent = state.phase === 'replaying' ? 'Ⅱ Stop replay' : '▶ Replay';
  renderTabs();
}

function renderTabs() {
  const tabs = $('run-tabs');
  while (tabs.children.length > state.runs.length) tabs.lastElementChild.remove();
  state.runs.forEach((run, index) => {
    let button = tabs.children[index];
    if (!button) { button = document.createElement('button'); tabs.append(button); }
    button.type = 'button'; button.textContent = `${run.name} · ${run.status}`;
    button.setAttribute('aria-pressed', String(index === state.selectedIndex));
    button.disabled = !run.snapshots.length;
    button.onclick = () => { state.selectedIndex = index; state.selectedFrameIndex = state.phase === 'replaying' ? visitedReplayFrame(index) : null; updateControls(); scheduleRender(); };
  });
}

function selectedSnapshot() {
  const frames = state.runs[state.selectedIndex]?.snapshots;
  if (!frames?.length) return preview;
  return frames[selectedFrameIndex()];
}

function selectedFrameIndex() {
  const frames = state.runs[state.selectedIndex]?.snapshots ?? [];
  if (!frames.length) return -1;
  if (Number.isInteger(state.selectedFrameIndex)) return Math.max(0, Math.min(frames.length - 1, state.selectedFrameIndex));
  return Math.max(0, frames.findLastIndex(s => s.elapsed_seconds <= state.selectedTime));
}

function visitedReplayFrame(run) {
  const current = Math.floor(replayFraction * Math.max(0, replayCheckpoints.length - 1));
  for (let i = current; i >= 0; i--) if (replayCheckpoints[i].run === run) return replayCheckpoints[i].frame;
  return 0;
}

function scheduleRender() {
  if (framePending) return;
  framePending = true;
  requestAnimationFrame(render);
}

function number(value, digits = 4) {
  if (!Number.isFinite(value)) return '—';
  return Math.abs(value) >= 1e8 || Math.abs(value) > 0 && Math.abs(value) < 1e-4 ? value.toExponential(3) : value.toFixed(digits);
}

function render() {
  framePending = false;
  const start = performance.now();
  if (state.phase === 'replaying') {
    const fraction = Math.min(1, (start - replayStart) / (1000 * replayDuration(maximumTime(), Number($('replay-pace').value), replayCheckpoints.length)));
    replayFraction = fraction;
    const position = fraction * Math.max(0, replayCheckpoints.length - 1);
    const checkpoint = replayCheckpoints[Math.floor(position)];
    const next = replayCheckpoints[Math.min(replayCheckpoints.length - 1, Math.floor(position) + 1)];
    state.selectedTime = checkpoint.time + (next.time - checkpoint.time) * (position % 1);
    state.selectedFrameIndex = visitedReplayFrame(state.selectedIndex);
    if (fraction >= 1) { state.phase = 'complete'; replayStart = null; updateControls(); }
  }
  const snapshot = selectedSnapshot();
  const run = state.runs[state.selectedIndex];
  const geometry = drawPacking($('packing'), snapshot ? {...snapshot, algorithm: run?.algorithm ?? 0} : null,
    {contacts: $('contacts').checked, trial: $('trial').checked});
  drawHistory($('history'), state.runs, state.selectedTime, {objective: $('objective').checked,
    timeScale: $('time-scale').value, zoom: $('zoom-improvements').checked});
  if (snapshot) {
    $('radius').textContent = number(snapshot.radius);
    $('area-ratio').textContent = number(snapshot.radius / instance.lowerBound, 3);
    $('elapsed').textContent = snapshot.elapsed_seconds < 0.1 ? `${number(snapshot.elapsed_seconds * 1000, 1)} ms` : `${number(snapshot.elapsed_seconds, 2)} s`;
    $('work').textContent = new Intl.NumberFormat('en').format(snapshot.evaluations);
    $('solution').textContent = snapshot.output;
    const raw = $('trial').checked && snapshot.diagnostics;
    $('canvas-mode').textContent = raw ? 'GEOMETRY / RAW TRIAL' : 'GEOMETRY / BEST KNOWN';
    $('feasible').textContent = raw ? 'Raw trial · may be infeasible' : '✓ Checked feasible';
    $('packing-summary').textContent = `${preview ? 'Recorded example' : run?.name ?? 'Packing'}: ${instance.radii.length} circles, best feasible radius ${number(snapshot.radius)}, ratio to area lower bound ${number(snapshot.radius / instance.lowerBound, 3)}. ${raw ? 'Showing raw accepted trial.' : 'Showing checked feasible geometry.'} ${geometry.contacts} near contacts, ${geometry.overlaps} overlapping pairs, ${geometry.containmentViolations} boundary violations in this view.`;
  } else {
    for (const id of ['radius', 'area-ratio', 'elapsed', 'work']) $(id).textContent = '—';
    $('solution').textContent = ''; $('feasible').textContent = 'Awaiting solution';
    $('packing-summary').textContent = 'No computed packing. Run an experiment to see checked geometry.';
  }
  $('active-run').textContent = state.activeIndex >= 0 ? `${state.runs[state.activeIndex].name} · ${state.phase}` : preview ? 'Recorded example · run your own' : state.phase === 'replaying' ? 'Recorded replay' : 'Search finished';
  const stats = traceStats(state.runs);
  $('trace-info').textContent = `${stats.snapshotCount} retained frames · ${(stats.traceBytes / 1024).toFixed(0)} KiB / 4096 KiB history · ${stats.dropped} frames evicted. Replay uses recorded feasible frames without interpolating geometry.`;
  const max = maximumTime();
  $('timeline').value = max ? Math.round(timeFraction(state.selectedTime, max, $('time-scale').value) * 1000) : 1000;
  const frames = state.runs[state.selectedIndex]?.snapshots ?? [];
  $('frame-status').textContent = frames.length ? `Checkpoint ${selectedFrameIndex() + 1} / ${frames.length}` : 'No recorded checkpoints';
  $('history-summary').textContent = state.runs.filter(r => r.snapshots.length).map(r => `${r.name}: ${r.snapshots.length} retained frames, latest feasible radius ${number(r.snapshots.at(-1).radius)} at ${number(r.snapshots.at(-1).elapsed_seconds, 2)} seconds.`).join(' ') || 'No run history yet.';
  const rows = document.createDocumentFragment();
  for (const r of state.runs) for (const s of r.snapshots.slice(-12)) {
    const row = document.createElement('tr');
    for (const value of [r.name, `${number(s.elapsed_seconds, 2)} s`, number(s.radius)]) {
      const cell = document.createElement('td'); cell.textContent = value; row.append(cell);
    }
    rows.append(row);
  }
  $('history-table').replaceChildren(rows);
  sample(performanceStats.frames, performance.now() - start);
  if (state.phase === 'replaying') scheduleRender();
}

function startRun(index) {
  state.activeIndex = index; state.selectedIndex = index;
  state.phase = 'running'; state.runs[index].status = 'running';
  $('status').textContent = `${state.runs[index].name} is running${state.runs.length === 2 ? ' · comparison runs use equal budgets, one after another' : ''}.`;
  activeId = worker.start(state.runs[index].config);
}

function reset({clear = true} = {}) {
  const start = performance.now();
  activeId = null; replayStart = null; pendingControl = null;
  if (worker) worker.cancel();
  state.phase = 'idle'; state.activeIndex = -1; state.selectedIndex = 0; state.selectedTime = 0;
  state.selectedFrameIndex = null;
  state.runs = []; importedSolver = null;
  inputError = false;
  if (clear) { performanceStats.frames = []; performanceStats.controls = []; }
  $('error').textContent = ''; $('status').textContent = 'Ready. Configure an experiment or load a recording.';
  try { instance = parseInput($('input').value); } catch { instance = {radii: [], lowerBound: 1}; }
  preview = null;
  sample(performanceStats.controls, performance.now() - start);
  updateControls(); scheduleRender();
}

function runExperiment() {
  try {
    reset();
    instance = parseInput($('input').value);
    $('input').value = instance.input;
    const choice = $('compare').value;
    const variant = Number($('variant').value);
    if (![1, 2, 3].includes(variant)) throw new Error('Choose an experimental B policy.');
    const algorithms = choice === 'compare' ? [0, variant] : [choice === 'a' ? 0 : variant];
    const seed = $('seed').value.trim();
    const timeLimitSeconds = Number($('budget').value);
    state.runs = algorithms.map(algorithm => {
      const config = validateConfig({input: instance.input, algorithm, seed, timeLimitSeconds}, instance.input, algorithm);
      return {name: names[algorithm], algorithm, config, snapshots: [], dropped: 0, status: 'queued'};
    });
    startRun(0); updateControls(); scheduleRender();
  } catch (error) { state.runs = []; showError(error); }
}

function control(type) {
  pendingControl = {type: {pause: 'paused', resume: 'resumed', cancel: 'done'}[type], at: performance.now()};
  if (type === 'cancel') state.runs.forEach(r => { if (r.status === 'queued') r.status = 'cancelled'; });
  worker[type](); updateControls();
}

async function loadFile(file, limit) {
  if (!file) return null;
  if (file.size > limit) throw new Error(`File exceeds ${(limit / 1024).toFixed(0)} KiB.`);
  return file.text();
}

function download(text, filename, type) {
  const url = URL.createObjectURL(new Blob([text], {type}));
  const anchor = document.createElement('a'); anchor.href = url; anchor.download = filename;
  anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}

$('run').onclick = runExperiment;
for (const type of ['pause', 'resume', 'cancel']) $(type).onclick = () => control(type);
$('reset').onclick = () => reset();
$('compare').onchange = updateControls;
$('preset').onchange = () => {
  const preset = presets[$('preset').value];
  if (!preset) return;
  $('input').value = preset.input; reset();
  preview = preset.preview ?? null; scheduleRender();
};
$('input').oninput = () => { $('preset').value = 'custom'; reset({clear: false}); };
$('upload').onchange = async () => {
  inputError = true; $('error').textContent = ''; updateControls();
  try {
    const text = await loadFile($('upload').files[0], MAX_INPUT_BYTES);
    if (text == null) return;
    parseInput(text); $('input').value = text; $('preset').value = 'custom'; reset();
  } catch (error) { reset(); inputError = true; showError(error); }
  finally { $('upload').value = ''; }
};
$('import-run').onchange = async () => {
  $('error').textContent = '';
  try {
    const text = await loadFile($('import-run').files[0], MAX_IMPORT_BYTES);
    if (text == null) return;
    reset();
    const data = parseRecording(text);
    instance = data.instance; state.runs = data.runs; importedSolver = data.solver;
    $('input').value = instance.input; $('preset').value = 'custom';
    state.phase = 'complete'; state.selectedIndex = Math.min(data.view?.selectedIndex ?? 0, state.runs.length - 1);
    if (!Number.isInteger(state.selectedIndex) || state.selectedIndex < 0) state.selectedIndex = 0;
    state.selectedTime = Number.isFinite(data.view?.selectedTime) ? Math.max(0, Math.min(maximumTime(), data.view.selectedTime)) : maximumTime();
    const frameIndex = data.view?.selectedFrameIndex;
    state.selectedFrameIndex = Number.isInteger(frameIndex) && frameIndex >= 0 ? Math.min(frameIndex, state.runs[state.selectedIndex].snapshots.length - 1) : null;
    $('status').textContent = 'Validated recording loaded. Replay and export are available.';
    updateControls(); scheduleRender();
  } catch (error) { state.runs = []; showError(error); }
  finally { $('import-run').value = ''; }
};
$('timeline').oninput = () => {
  if (active()) return;
  replayStart = null; state.phase = 'complete';
  state.selectedFrameIndex = null;
  state.selectedTime = fractionTime(Number($('timeline').value) / 1000, maximumTime(), $('time-scale').value);
  updateControls(); scheduleRender();
};
$('replay').onclick = () => {
  if (active()) return;
  if (state.phase === 'replaying') { state.phase = 'complete'; replayStart = null; }
  else {
    replayCheckpoints = state.runs.flatMap((run, index) => run.snapshots.map((s, frame) => ({time: s.elapsed_seconds, run: index, frame})))
      .sort((a, b) => a.time - b.time || a.run - b.run || a.frame - b.frame);
    state.phase = 'replaying'; state.selectedTime = 0; state.selectedFrameIndex = null; replayFraction = 0; replayStart = performance.now();
  }
  updateControls(); scheduleRender();
};
for (const id of ['contacts', 'trial', 'objective']) $(id).onchange = scheduleRender;
for (const id of ['time-scale', 'replay-pace']) $(id).onchange = () => {
  if (state.phase === 'replaying') replayStart = performance.now() - replayFraction * replayDuration(maximumTime(), Number($('replay-pace').value), replayCheckpoints.length) * 1000;
  scheduleRender();
};
$('zoom-improvements').onchange = scheduleRender;
function stepFrame(direction) {
  if (active()) return;
  const index = selectedFrameIndex();
  const frames = state.runs[state.selectedIndex]?.snapshots;
  if (!frames?.length) return;
  state.phase = 'complete'; replayStart = null;
  state.selectedFrameIndex = Math.max(0, Math.min(frames.length - 1, index + direction));
  state.selectedTime = frames[state.selectedFrameIndex].elapsed_seconds;
  updateControls(); scheduleRender();
}
$('previous-frame').onclick = () => stepFrame(-1);
$('next-frame').onclick = () => stepFrame(1);
$('export-solution').onclick = () => download(selectedSnapshot().output, 'packing-solution.txt', 'text/plain');
$('export-run').onclick = () => {
  const runs = state.runs.filter(r => r.snapshots.at(-1)?.finished);
  const selected = runs.indexOf(state.runs[state.selectedIndex]);
  download(JSON.stringify({schema_version: 1, kind: 'circle-packing-run', created_at: new Date().toISOString(),
    input: instance.input, solver: importedSolver ?? buildInfo, runs,
    view: {selectedIndex: Math.max(0, selected), selectedTime: selected >= 0 ? state.selectedTime : runs[0].snapshots.at(-1).elapsed_seconds,
      selectedFrameIndex: selected >= 0 ? state.selectedFrameIndex : null}}), 'packing-run.json', 'application/json');
};

new ResizeObserver(scheduleRender).observe($('packing').parentElement);
window.addEventListener('pagehide', () => worker.dispose());
fetch(new URL('./build-info.json', import.meta.url)).then(r => r.ok ? r.json() : null)
  .then(info => { if (info) buildInfo = info; }).catch(() => {});
$('input').value = presets['10'].input; instance = parseInput($('input').value); preview = presets['10'].preview;
connectWorker(); updateControls(); scheduleRender();
