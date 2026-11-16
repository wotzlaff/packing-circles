// Each pump yields after ~8 ms; individual native transitions are bounded at N<=50.
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', {fatal: true});
let wasm, runId, timer, active = false, paused = false;
let started = 0, pausedAt = 0, pausedDuration = 0, lastProgress = 0;
const loaded = (async () => {
  const response = await fetch(new URL('./packing_wasm.wasm', import.meta.url));
  if (!response.ok) throw new Error(`WASM load failed: ${response.status}`);
  // ArrayBuffer works with static servers that do not declare application/wasm.
  wasm = (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance.exports;
  postMessage({type: 'ready'});
})();
loaded.catch((error) => postMessage({type: 'error', error: error.message}));

function snapshot() {
  return JSON.parse(decoder.decode(new Uint8Array(
    wasm.memory.buffer, wasm.snapshot_ptr(), wasm.snapshot_len())));
}

function check(status) {
  if (status < 0) throw new Error(snapshot().error || 'Solver failed');
  return status;
}

function emit(type) { postMessage({type, runId, snapshot: snapshot()}); }
function elapsed() { return (performance.now() - started - pausedDuration) / 1000; }

function fail(error) {
  clearTimeout(timer);
  active = false;
  paused = false;
  postMessage({type: 'error', runId, error: error.message});
}

function pump() {
  if (!active || paused) return;
  try {
    const until = performance.now() + 8;
    do {
      if (check(wasm.session_step(64, elapsed())) === 1) {
        active = false;
        emit('done');
        return;
      }
    } while (performance.now() < until);
    if (performance.now() - lastProgress >= 100) {
      emit('progress');
      lastProgress = performance.now();
    }
    timer = setTimeout(pump, 0);
  } catch (error) { fail(error); }
}

function uint32(value, name) {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`${name} must be an unsigned 32-bit integer`);
  }
  return value;
}

function start(options) {
  active = false;
  paused = false;
  clearTimeout(timer);
  if (options?.validationError) throw new Error(options.validationError);
  if (!options || typeof options.input !== 'string') throw new Error('Input must be text');
  // Check before encoding, then enforce bytes as well for multibyte text.
  if (options.input.length > wasm.input_capacity()) throw new Error('Input exceeds 16 KiB');
  const bytes = encoder.encode(options.input);
  if (bytes.length > wasm.input_capacity()) throw new Error('Input exceeds 16 KiB');
  const seedText = String(options.seed ?? '5');
  if (!/^\d{1,20}$/.test(seedText)) throw new Error('Seed must be a decimal u64');
  const seed = BigInt(seedText);
  if (seed > 0xffffffffffffffffn) throw new Error('Seed exceeds u64');
  const time = options.timeLimitSeconds ?? -1;
  if (time !== -1 && (!Number.isFinite(time) || time < 0)) throw new Error('Invalid time limit');
  const algorithm = uint32(options.algorithm ?? 0, 'algorithm');
  const evaluations = uint32(options.evaluationLimit ?? 0xffffffff, 'evaluationLimit');
  const iterations = uint32(options.iterationLimit ?? 1_000_000, 'iterationLimit');
  const restarts = uint32(options.restartLimit ?? 10_000, 'restartLimit');
  const pointer = wasm.input_ptr(); // This call can allocate and grow linear memory.
  new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
  started = performance.now();
  pausedDuration = 0;
  check(wasm.session_start(bytes.length, algorithm,
    Number(seed & 0xffffffffn), Number(seed >> 32n), evaluations, iterations, restarts, time));
  active = !snapshot().finished;
  lastProgress = performance.now();
  emit('progress');
  if (active) timer = setTimeout(pump, 0);
  else emit('done');
}

// Preserve command order while the binary is loading. Pumping stays synchronous.
let commands = Promise.resolve();
onmessage = ({data}) => {
  commands = commands.then(async () => {
    await loaded;
    if (data.type === 'start') {
      runId = data.runId;
      start(data.options);
      return;
    }
    if (data.runId !== runId) return;
    if (data.type === 'pause' && active && !paused) {
      clearTimeout(timer);
      if (check(wasm.session_step(0, elapsed())) === 1) {
        active = false;
        emit('done');
        return;
      }
      pausedAt = performance.now();
      paused = true;
      emit('paused');
    } else if (data.type === 'resume' && active && paused) {
      pausedDuration += performance.now() - pausedAt;
      paused = false;
      if (check(wasm.session_step(0, elapsed())) === 1) {
        active = false;
        emit('done');
        return;
      }
      emit('resumed');
      timer = setTimeout(pump, 0);
    } else if (data.type === 'cancel' && active) {
      clearTimeout(timer);
      if (!paused) check(wasm.session_step(0, elapsed()));
      check(wasm.session_cancel());
      active = false;
      paused = false;
      emit('done');
    }
  }).catch(fail);
};
