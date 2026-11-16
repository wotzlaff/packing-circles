/** Worker-only solver client. No WASM or numerical code runs on the UI thread. */
export class PackingWorker extends EventTarget {
  #worker;
  #runId = 0;
  #trace = [];
  #traceBytes = 0;
  #dropped = 0;

  constructor() {
    super();
    this.#worker = new Worker(new URL('./solver-worker.js', import.meta.url), {type: 'module'});
    this.#worker.onmessage = ({data}) => {
      if (data.runId !== undefined && data.runId !== this.#runId) return;
      const frames = [...(data.history ?? [])];
      if (data.snapshot && ['progress', 'done', 'paused', 'resumed'].includes(data.type)) frames.push(data.snapshot);
      for (const frame of frames) {
        const last = this.#trace.at(-1)?.snapshot;
        if (last && last.elapsed_seconds === frame.elapsed_seconds &&
            last.evaluations === frame.evaluations && last.radius === frame.radius &&
            last.iterations === frame.iterations && last.finished === frame.finished) continue;
        const snapshot = structuredClone(frame);
        const bytes = new TextEncoder().encode(JSON.stringify(snapshot)).length;
        this.#trace.push({snapshot, bytes});
        this.#traceBytes += bytes;
        while (this.#trace.length > 256 || this.#traceBytes > 1024 * 1024) {
          this.#traceBytes -= this.#trace.shift().bytes;
          this.#dropped++;
        }
      }
      this.dispatchEvent(new MessageEvent('message', {data}));
    };
    this.#worker.onerror = (event) => {
      this.dispatchEvent(new MessageEvent('message', {
        data: {type: 'error', runId: this.#runId, error: event.message},
      }));
    };
  }

  /** Start replaces any previous run. Seed is a decimal u64 string. */
  start(options) {
    this.#trace = [];
    this.#traceBytes = 0;
    this.#dropped = 0;
    const {input, algorithm, seed, evaluationLimit, iterationLimit, restartLimit, timeLimitSeconds} = options ?? {};
    // Reject huge input before copying it across threads. Send only scalar config.
    const safeOptions = typeof input !== 'string' || input.length > 16 * 1024
      ? {validationError: 'Input must be text of at most 16 KiB'}
      : {input, algorithm, seed, evaluationLimit, iterationLimit, restartLimit, timeLimitSeconds};
    this.#worker.postMessage({type: 'start', runId: ++this.#runId, options: safeOptions});
    return this.#runId;
  }

  pause() { this.#send('pause'); }
  resume() { this.#send('resume'); }
  cancel() { this.#send('cancel'); }
  #send(type) { this.#worker.postMessage({type, runId: this.#runId}); }

  /** Bounded recent history; old snapshots are evicted, and drops are explicit. */
  exportTrace() {
    return {dropped: this.#dropped, bytes: this.#traceBytes,
      snapshots: structuredClone(this.#trace.map(({snapshot}) => snapshot))};
  }

  dispose() { this.#worker.terminate(); }
}
