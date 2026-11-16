import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
function option(name, fallback) {
    const i = args.indexOf(name);
    return i < 0 ? fallback : args[i + 1];
}
const browserName = option('--browser', 'chromium');
assert.ok(['chromium', 'firefox'].includes(browserName), 'browser must be chromium or firefox');
const reportPath = option('--report', '/tmp/circles-browser-report.json');
const playwright = await import(process.env.PLAYWRIGHT_MODULE ? pathToFileURL(process.env.PLAYWRIGHT_MODULE).href : 'playwright');
const fixtures = [10, 20, 30, 40, 50].flatMap(n => [1, 2].map(i => `${n}_${String(i).padStart(2, '0')}.txt`));
const records = [];
const wasm = await readFile(path.join(root, 'web/packing_wasm.wasm'));
const report = { schema_version: 1, measured_at: new Date().toISOString(), math_profile: 'portable-libm-0.2.16', seed: '5', evaluation_limit: 5000, rustc: spawnSync('rustc', ['--version'], {encoding: 'utf8'}).stdout.trim(), node_version: process.version, browser: browserName, platform: os.platform(), kernel: os.release(), arch: os.arch(), cpu: os.cpus()[0]?.model, logical_cpus: os.availableParallelism(), wasm_sha256: createHash('sha256').update(wasm).digest('hex'), records };

function checkGeometry(radius, points, radii) {
    assert.ok(Number.isFinite(radius) && radius > 0);
    assert.equal(points.length, radii.length);
    const tol = 1e-9 * Math.max(1, radius, ...radii);
    points.forEach(([x, y], i) => {
        assert.ok(Number.isFinite(x) && Number.isFinite(y));
        assert.ok(Math.hypot(x, y) + radii[i] <= radius + tol, `containment ${i}`);
        for (let j = 0; j < i; j++) {
            assert.ok(Math.hypot(x - points[j][0], y - points[j][1]) + tol >= radii[i] + radii[j], `overlap ${i}, ${j}`);
        }
    });
}
function checkSnapshot(snapshot, input) {
    assert.equal(snapshot.schema_version, 1);
    assert.equal(snapshot.feasible, true);
    const values = input.trim().split(/\s+/).map(Number);
    const radii = values.slice(1);
    assert.equal(values[0], radii.length);
    assert.deepEqual(snapshot.radii, radii);
    checkGeometry(snapshot.radius, snapshot.points, radii);
    const tokens = snapshot.output.trim().split(/\s+/);
    assert.equal(tokens.length, 1 + 3 * radii.length);
    assert.match(tokens[0], /^\d+\.\d{4}$/);
    const points = radii.map((r, i) => {
        assert.equal(Number(tokens[1 + 3 * i]), r);
        assert.match(tokens[2 + 3 * i], /^-?\d+\.\d{4}$/);
        assert.match(tokens[3 + 3 * i], /^-?\d+\.\d{4}$/);
        return [Number(tokens[2 + 3 * i]), Number(tokens[3 + 3 * i])];
    });
    checkGeometry(Number(tokens[0]), points, radii);
}
function native(fixture, algorithm) {
    const p = spawnSync('cargo', ['run', '--quiet', '--release', '--locked', '-p', 'packing-core', '--example', 'session_probe', '--', path.join(root, 'bench/data/development', fixture), String(algorithm), '5', '5000', '64'], { cwd: root, encoding: 'utf8', timeout: 120000 });
    assert.equal(p.status, 0, `native probe: ${p.stderr}`);
    return JSON.parse(p.stdout);
}

// Python chooses an unused loopback port and reports it before serving.
const serverCode = 'import http.server, socketserver, os\nos.chdir(' + JSON.stringify(root) + ')\ns=socketserver.TCPServer(("127.0.0.1",0),http.server.SimpleHTTPRequestHandler)\nprint(s.server_address[1],flush=True)\ns.serve_forever()';
const server = spawn('python3', ['-u', '-c', serverCode], { stdio: ['ignore', 'pipe', 'pipe'] });
let browser;
try {
    const port = await new Promise((resolve, reject) => {
        let data = '';
        server.stdout.on('data', chunk => { data += chunk; if (data.includes('\n')) resolve(Number(data.split('\n')[0])); });
        let errors = '';
        server.stderr.on('data', chunk => { errors = (errors + chunk).slice(-2000); });
        server.once('error', reject);
        server.once('exit', code => reject(new Error(`HTTP server exited ${code}: ${errors}`)));
    });
    browser = await playwright[browserName].launch({ headless: true });
    report.browser_version = browser.version();
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${port}/web/`);
    await page.evaluate(async () => {
        const { PackingWorker } = await import('./solver.js');
        window.testWorker = new PackingWorker();
        window.events = [];
        window.testWorker.addEventListener('message', event => window.events.push({ ...event.data, received: performance.now() }));
        window.waitEvent = (predicate, timeout = 30000) => new Promise((resolve, reject) => {
            const start = performance.now();
            function poll() {
                const event = window.events.find(predicate);
                if (event) resolve(event);
                else if (performance.now() - start > timeout) reject(new Error(`timed out; events: ${JSON.stringify(window.events.slice(-4))}`));
                else setTimeout(poll, 5);
            }
            poll();
        });
        await window.waitEvent(e => e.type === 'ready');
    });
    async function run(input, algorithm = 0, extra = {}) {
        return page.evaluate(async ({ input, algorithm, extra }) => {
            window.events.length = 0;
            const started = performance.now();
            window.testWorker.start({ input, algorithm, seed: '5', evaluationLimit: 5000, iterationLimit: 100000, restartLimit: 100, timeLimitSeconds: null, ...extra });
            const end = await window.waitEvent(e => e.type === 'done' || e.type === 'error', 120000);
            return { end, events: window.events, duration_ms: performance.now() - started };
        }, { input, algorithm, extra });
    }
    for (const fixture of fixtures) {
        const input = await readFile(path.join(root, 'bench/data/development', fixture), 'utf8');
        for (const algorithm of [0, 1, 2, 3]) {
            const result = await run(input, algorithm);
            assert.equal(result.end.type, 'done', JSON.stringify(result.end));
            checkSnapshot(result.end.snapshot, input);
            const reference = native(fixture, algorithm);
            records.push({ fixture, input_sha256: createHash('sha256').update(input).digest('hex'), algorithm, duration_ms: result.duration_ms, browser: result.end.snapshot, native: reference });
            assert.ok(Math.abs(result.end.snapshot.radius - reference.radius) <= 1e-6 * Math.max(1, Math.abs(reference.radius)), `${fixture} algorithm ${algorithm}: ${result.end.snapshot.radius} vs ${reference.radius}`);
            for (const key of ['evaluations', 'iterations', 'restarts', 'initialization_fallbacks', 'numerical_failures', 'termination']) {
                assert.equal(result.end.snapshot[key], reference[key], `${fixture} algorithm ${algorithm}: ${key}`);
            }
            console.log(`${fixture} algorithm ${algorithm}: ${result.duration_ms.toFixed(1)} ms`);
        }
    }
    const input = await readFile(path.join(root, 'bench/data/development/50_01.txt'), 'utf8');
    const first = await run(input);
    const repeated = await run(input);
    assert.equal(first.end.snapshot.output, repeated.end.snapshot.output, 'fixed work must be deterministic');
    for (const key of ['evaluations', 'iterations', 'restarts']) assert.equal(first.end.snapshot[key], repeated.end.snapshot[key]);
    const paused = await page.evaluate(async input => {
        window.events.length = 0;
        window.testWorker.start({ input, algorithm: 0, seed: '5', evaluationLimit: 5000, iterationLimit: 100000, restartLimit: 100, timeLimitSeconds: null });
        await window.waitEvent(e => e.type === 'progress');
        window.testWorker.pause();
        const paused = await window.waitEvent(e => e.type === 'paused');
        await new Promise(resolve => setTimeout(resolve, 250));
        if (window.events.some(e => e.type === 'done' || e.type === 'progress' && e.received > paused.received)) {
            throw new Error('Paused worker continued running');
        }
        window.testWorker.resume();
        await window.waitEvent(e => e.type === 'resumed');
        return await window.waitEvent(e => e.type === 'done' || e.type === 'error', 120000);
    }, input);
    assert.equal(paused.type, 'done');
    assert.equal(paused.snapshot.output, first.end.snapshot.output);
    for (const key of ['evaluations', 'iterations', 'restarts']) assert.equal(paused.snapshot[key], first.end.snapshot[key]);
    for (const invalid of ['not an instance', `51\n${'1 '.repeat(51)}`, '2\n1 NaN', '0', `100001\n${'1\n'.repeat(100001)}`]) {
        const result = await run(invalid);
        assert.equal(result.end.type, 'error', 'invalid input must fail');
    }
    const responsiveness = await page.evaluate(async input => {
        window.events.length = 0;
        let beats = 0;
        const timer = setInterval(() => beats++, 10);
        window.testWorker.start({ input, algorithm: 0, seed: '5', evaluationLimit: 1000000000, iterationLimit: 1000000000, restartLimit: 1000000000, timeLimitSeconds: 5 });
        const first = await window.waitEvent(e => e.type === 'progress');
        await new Promise(resolve => setTimeout(resolve, 2000));
        const progress = window.events.filter(e => e.type === 'progress');
        const beforeCancel = performance.now();
        window.testWorker.cancel();
        const end = await window.waitEvent(e => e.type === 'done' || e.type === 'error');
        clearInterval(timer);
        return { beats, progress, first, end, cancel_ms: performance.now() - beforeCancel };
    }, input);
    assert.equal(responsiveness.end.type, 'done');
    assert.equal(responsiveness.end.snapshot.termination, 'cancelled');
    checkSnapshot(responsiveness.end.snapshot, input);
    assert.ok(responsiveness.progress.length >= 10, `only ${responsiveness.progress.length} progress events`);
    const gaps = responsiveness.progress.slice(1).map((event, i) => event.received - responsiveness.progress[i].received);
    report.maximum_progress_gap_ms = Math.max(...gaps);
    assert.ok(Math.max(...gaps) <= 200, `progress gap ${Math.max(...gaps)} ms`);
    assert.ok(responsiveness.beats >= 100, `only ${responsiveness.beats} main-thread timer ticks`);
    assert.ok(responsiveness.cancel_ms < 100, `cancel took ${responsiveness.cancel_ms} ms`);
    const replacement = await page.evaluate(async input => {
        window.events.length = 0;
        const options = { input, algorithm: 0, seed: '5', evaluationLimit: 5000, iterationLimit: 100000, restartLimit: 100, timeLimitSeconds: null };
        const previous = window.testWorker.start(options);
        const current = window.testWorker.start(options);
        await window.waitEvent(e => e.type === 'done' && e.runId === current, 120000);
        return { previous, current, events: window.events };
    }, input);
    assert.notEqual(replacement.previous, replacement.current);
    assert.ok(replacement.events.every(e => e.runId === replacement.current), 'superseded run emitted a stale event');
    const postCancel = await run(input);
    assert.equal(postCancel.end.type, 'done');
    assert.ok(postCancel.events.every(e => e.runId === postCancel.end.runId), 'stale event from prior run');
    const trace = await page.evaluate(() => window.testWorker.exportTrace());
    assert.ok(trace.snapshots.length <= 256 && trace.bytes <= 1024 * 1024);
    trace.snapshots.forEach(s => checkSnapshot(s, input));
    report.responsiveness = responsiveness;
    report.relative_radius_error_max = Math.max(...records.map(r => Math.abs(r.browser.radius - r.native.radius) / r.native.radius));
    await page.evaluate(() => window.testWorker.dispose());
    report.passed = true;
} catch (error) {
    report.passed = false;
    report.error = error.stack;
    throw error;
} finally {
    if (browser) await browser.close();
    server.kill();
    await writeFile(reportPath, JSON.stringify(report, null, 2) + '\n');
    console.log(`Report: ${reportPath}`);
}
