import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {chromium} from 'playwright';

const server = spawn('python3', ['-u', '-c', 'import http.server,socketserver\ns=socketserver.TCPServer(("127.0.0.1",0),http.server.SimpleHTTPRequestHandler)\nprint(s.server_address[1],flush=True)\ns.serve_forever()']);
let browser;
try {
  const port = await new Promise((resolve, reject) => {
    server.stdout.once('data', data => resolve(Number(String(data).trim())));
    server.once('error', reject);
  });
  browser = await chromium.launch({headless: true});
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/web/`);
  const result = await page.evaluate(async () => {
    const {PackingWorker} = await import('./solver.js');
    const worker = new PackingWorker();
    const messages = [];
    await new Promise((resolve, reject) => {
      worker.addEventListener('message', ({data}) => {
        messages.push(data);
        if (data.type === 'ready') worker.start({input: '10\n23 67 24 56 47 60 77 82 22 92\n', seed: '5', evaluationLimit: 5000});
        if (data.type === 'done') resolve();
        if (data.type === 'error') reject(new Error(data.error));
      });
    });
    const trace = worker.exportTrace();
    worker.dispose();
    return {messages, trace};
  });
  const initial = result.messages.find(m => m.type === 'progress');
  assert.deepEqual(initial.history, []);
  const frames = result.messages.flatMap(m => m.history ?? []);
  assert.ok(frames.length >= 1 && frames.length <= 64);
  assert.ok(frames[0].elapsed_seconds < 0.1);
  assert.ok(frames[0].radius < initial.snapshot.radius);
  assert.ok(result.messages.at(-1).snapshot.radius < frames[0].radius);
  const trials = frames.filter(frame => frame.elapsed_seconds < 0.1 && frame.diagnostics);
  assert.ok(trials.length >= 2, 'early search positions should be retained');
  assert.ok(trials.every(frame => Number.isFinite(frame.diagnostics.objective) &&
    Number.isFinite(frame.diagnostics.trial_radius) &&
    frame.diagnostics.points.every(point => point.every(Number.isFinite))));
  assert.ok(trials.some(frame => frame.diagnostics.objective !== trials[0].diagnostics.objective &&
    JSON.stringify(frame.diagnostics.points) !== JSON.stringify(trials[0].diagnostics.points)));
  for (let i = 0; i < frames.length; i++) {
    assert.equal(frames[i].feasible, true);
    if (i) {
      assert.ok(frames[i].radius <= frames[i - 1].radius);
      assert.ok(frames[i].elapsed_seconds >= frames[i - 1].elapsed_seconds);
      assert.ok(frames[i].evaluations >= frames[i - 1].evaluations);
    }
  }
  for (const m of result.messages.filter(m => m.snapshot)) {
    for (const frame of m.history) assert.ok(frame.elapsed_seconds <= m.snapshot.elapsed_seconds);
  }
  assert.ok(result.trace.snapshots.length >= frames.length);
  assert.equal(result.messages.at(-1).snapshot.evaluations, 5000);
  console.log(`Captured ${frames.length} early frames`);
} finally {
  await browser?.close();
  server.kill();
}
