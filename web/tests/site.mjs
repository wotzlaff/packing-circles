import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
function option(name, fallback) {
    const i = args.indexOf(name);
    if (i < 0) return fallback;
    assert.ok(args[i + 1], `missing value for ${name}`);
    return args[i + 1];
}
const browserName = option('--browser', 'chromium');
assert.ok(['chromium', 'firefox'].includes(browserName));
const reportPath = option('--report', '/tmp/circles-site-report.json');
const screenshotPath = option('--screenshots', '/tmp/circles-site-screenshots');
const stressSeconds = Number(option('--stress-seconds', '30'));
assert.ok(Number.isFinite(stressSeconds) && stressSeconds >= 3);
const playwright = await import(process.env.PLAYWRIGHT_MODULE ? pathToFileURL(process.env.PLAYWRIGHT_MODULE).href : 'playwright');
const report = { browser: browserName, measured_at: new Date().toISOString(), node: process.version,
    platform: os.platform(), kernel: os.release(), cpu: os.cpus()[0]?.model,
    logical_cpus: os.availableParallelism(), stress_seconds: stressSeconds,
    performance_qualified: stressSeconds >= 30, cases: [] };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
report.wasm_sha256 = sha256(await readFile(path.join(root, 'web/packing_wasm.wasm')));
report.source_sha256 = Object.fromEntries(await Promise.all(['app.js', 'render.js', 'data.js', 'styles.css'].map(async file => [file, sha256(await readFile(path.join(root, 'web', file)))])));
try { report.build_info = JSON.parse(await readFile(path.join(root, 'web/build-info.json'), 'utf8')); }
catch (error) { if (error.code !== 'ENOENT') throw error; report.build_info = null; }
const serverCode = `import http.server, socketserver, os\nos.chdir(${JSON.stringify(root)})\ns=socketserver.TCPServer(('127.0.0.1',0),http.server.SimpleHTTPRequestHandler)\nprint(s.server_address[1],flush=True)\ns.serve_forever()`;
const server = spawn('python3', ['-u', '-c', serverCode], { stdio: ['ignore', 'pipe', 'pipe'] });
let browser;
try {
    const port = await new Promise((resolve, reject) => {
        let data = '', errors = '';
        server.stdout.on('data', chunk => { data += chunk; if (data.includes('\n')) resolve(Number(data.split('\n')[0])); });
        server.stderr.on('data', chunk => { errors = (errors + chunk).slice(-2000); });
        server.once('error', reject);
        server.once('exit', code => reject(new Error(`HTTP server exited ${code}: ${errors}`)));
    });
    browser = await playwright[browserName].launch({ headless: true });
    report.browser_version = browser.version();
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    const failures = [];
    page.on('pageerror', error => failures.push(error.stack));
    await page.goto(`http://127.0.0.1:${port}/web/`);
    await page.locator('#run').waitFor({ state: 'visible' });
    await page.waitForFunction(() => !document.querySelector('#run').disabled);
    await page.evaluate(async () => { window.__packingState = (await import('./app.js')).getAppState; });
    const state = () => page.evaluate(() => window.__packingState());
    const stats = () => page.evaluate(async () => (await import('./app.js')).getPerformanceStats());
    const wait = (predicate, timeout = 30000) => page.waitForFunction(predicate, null, { timeout });
    async function test(name, fn) {
        try { await fn(); report.cases.push({ name, passed: true }); console.log(`PASS ${name}`); }
        catch (error) { report.cases.push({ name, passed: false, error: error.stack }); throw error; }
    }
    async function reset() { await page.locator('#reset').click(); await wait(() => window.__unused === undefined && document.querySelector('#run') && !document.querySelector('#run').disabled); }
    async function upload(input, name = 'input.txt') {
        await page.locator('#upload').setInputFiles({ name, mimeType: 'text/plain', buffer: Buffer.from(input) });
    }
    async function download(selector) {
        const [file] = await Promise.all([page.waitForEvent('download'), page.locator(selector).click()]);
        return readFile(await file.path(), 'utf8');
    }
    function checkSolution(output, input) {
        const values = input.trim().split(/\s+/).map(Number), radii = values.slice(1);
        assert.equal(values[0], radii.length);
        const tokens = output.trim().split(/\s+/);
        assert.equal(tokens.length, 1 + radii.length * 3);
        assert.match(tokens[0], /^\d+\.\d{4}$/);
        const radius = Number(tokens[0]);
        assert.ok(Number.isFinite(radius) && radius > 0);
        const points = radii.map((r, i) => {
            assert.equal(Number(tokens[1 + 3 * i]), r);
            for (const token of tokens.slice(2 + 3 * i, 4 + 3 * i)) assert.match(token, /^-?\d+\.\d{4}$/);
            return [Number(tokens[2 + 3 * i]), Number(tokens[3 + 3 * i])];
        });
        const tol = 1e-9 * Math.max(1, radius, ...radii);
        points.forEach(([x, y], i) => {
            assert.ok(Math.hypot(x, y) + radii[i] <= radius + tol, `containment ${i}`);
            for (let j = 0; j < i; j++) assert.ok(Math.hypot(x - points[j][0], y - points[j][1]) + tol >= radii[i] + radii[j], `overlap ${i}, ${j}`);
        });
    }
    const input10 = await readFile(path.join(root, 'bench/data/development/10_01.txt'), 'utf8');
    const input50 = await readFile(path.join(root, 'bench/data/development/50_01.txt'), 'utf8');
    await test('valid and invalid uploads', async () => {
        for (const input of [input10, input50]) {
            await upload(input);
            await page.waitForFunction(value => document.querySelector('#input').value.trim() === value.trim(), input);
            assert.ok(await page.locator('#run').isEnabled());
        }
        for (const invalid of ['0', '2\n1 NaN', `51\n${'1 '.repeat(51)}`, '1\n' + '1'.repeat(16385)]) {
            await reset();
            await upload(invalid);
            await page.locator('[role=alert]').filter({ hasText: /\S/ }).waitFor();
            assert.equal(await page.locator('#run').isEnabled(), false, 'A rejected upload must block execution of old input');
            assert.notEqual((await state()).phase, 'running');
        }
        await reset();
    });
    await test('comparison is sequential with equal settings and replay', async () => {
        await upload(input10);
        await page.locator('#compare').selectOption('compare');
        await page.locator('#variant').selectOption('2');
        await page.locator('#seed').fill('5');
        await page.locator('#budget').fill('1.95');
        await page.locator('#run').click();
        await wait(() => document.querySelector('#run-tabs')?.textContent?.match(/A/i));
        let observedQueued = false;
        const deadline = Date.now() + 30000;
        while (Date.now() < deadline) {
            const s = await state();
            assert.ok(s.runs.filter(r => r.status === 'running').length <= 1, 'runs must not overlap');
            if (s.runs[1]?.status === 'queued') observedQueued = true;
            if (s.runs[1]?.status === 'running') assert.equal(s.runs[0].status, 'complete');
            if (s.phase === 'complete') break;
            await page.waitForTimeout(20);
        }
        const s = await state();
        assert.equal(s.phase, 'complete');
        assert.equal(s.runs.length, 2);
        assert.ok(observedQueued, 'B should be queued while A runs');
        assert.match(s.runs[0].name, /A/i);
        assert.match(s.runs[1].name, /B/i);
        assert.notEqual(s.runs[0].algorithm, s.runs[1].algorithm);
        assert.ok(s.runs.every(r => r.status === 'complete' && r.snapshots.length));
        assert.equal(String(s.runs[0].config.seed), '5');
        assert.equal(String(s.runs[1].config.seed), '5');
        assert.equal(s.runs[0].config.timeLimitSeconds, s.runs[1].config.timeLimitSeconds);
        await page.locator('#timeline').fill('500');
        const replay = await state();
        assert.ok(replay.selectedTime >= 0);
        for (let i = 0; i < replay.runs.length; i++) {
            await page.locator('#run-tabs button').nth(i).click();
            const selected = await state();
            const eligible = selected.runs[i].snapshots.filter(snapshot => snapshot.elapsed_seconds <= selected.selectedTime);
            if (eligible.length) {
                const displayed = Number((await page.locator('#radius').innerText()).replace(/,/g, ''));
                const expected = eligible.at(-1).radius;
                assert.ok(Math.abs(displayed - expected) <= 0.001 * Math.max(1, expected), `replay radius ${displayed} vs ${expected}`);
            }
        }
        assert.ok((await page.locator('#history-table tr').count()) > 0);
        assert.match(await page.locator('#packing-summary').innerText(), /\S/);
        assert.match(await page.locator('#history-summary').innerText(), /\S/);
        await page.locator('#replay').click();
        await page.waitForFunction(() => window.__packingState().phase === 'replaying');
        const replayStart = await page.locator('#frame-status').textContent();
        await page.waitForFunction(text => document.querySelector('#frame-status').textContent !== text, replayStart);
        await page.locator('#replay').click();
        assert.equal((await state()).phase, 'complete');
    });
    await test('solution and run export round trip', async () => {
        const solution = await download('#export-solution');
        checkSolution(solution, input10);
        const before = await state();
        const json = await download('#export-run');
        JSON.parse(json);
        await reset();
        await page.locator('#import-run').setInputFiles({ name: 'run.json', mimeType: 'application/json', buffer: Buffer.from(json) });
        await wait(() => document.querySelector('#export-solution') && !document.querySelector('#export-solution').disabled);
        const after = await state();
        assert.equal(after.runs.length, before.runs.length);
        assert.deepEqual(after.runs.map(r => [r.algorithm, r.config, r.snapshots.at(-1)]), before.runs.map(r => [r.algorithm, r.config, r.snapshots.at(-1)]));
        assert.equal(await download('#export-solution'), solution);
        const malicious = JSON.parse(json);
        malicious.runs[0].snapshots[0].radius = -1;
        for (const bad of ['{invalid', JSON.stringify(malicious)]) {
            await reset();
            await page.locator('#import-run').setInputFiles({ name: 'run.json', mimeType: 'application/json', buffer: Buffer.from(json) });
            await page.waitForFunction(() => window.__packingState().phase === 'complete');
            await page.locator('#import-run').setInputFiles({ name: 'bad.json', mimeType: 'application/json', buffer: Buffer.from(bad) });
            await page.locator('[role=alert]').filter({ hasText: /\S/ }).waitFor();
            assert.notEqual((await state()).phase, 'running');
            assert.equal((await state()).runs.length, 0, 'invalid import must clear stale runs');
        }
    });
    await test('pause, resume, cancel queued comparison and reset', async () => {
        await reset(); await upload(input50);
        await page.locator('#compare').selectOption('compare');
        await page.locator('#budget').fill('10');
        await page.locator('#run').click();
        await wait(() => !document.querySelector('#pause').disabled);
        await page.locator('#pause').click();
        await wait(() => document.querySelector('#resume') && !document.querySelector('#resume').disabled);
        const paused = await state();
        assert.equal(paused.phase, 'paused');
        const elapsed = await page.locator('#elapsed').innerText();
        await page.waitForTimeout(600);
        assert.equal(await page.locator('#elapsed').innerText(), elapsed);
        assert.deepEqual((await state()).runs.map(r => r.snapshots.length), paused.runs.map(r => r.snapshots.length));
        await page.locator('#resume').click();
        await wait(() => !document.querySelector('#pause').disabled);
        await page.locator('#cancel').click();
        await wait(() => document.querySelector('#run') && !document.querySelector('#run').disabled);
        const cancelled = await state();
        assert.notEqual(cancelled.runs[1]?.status, 'running');
        assert.notEqual(cancelled.runs[1]?.status, 'complete');
        const frames = cancelled.runs[0].snapshots;
        const maxTime = Math.max(...cancelled.runs.flatMap(r => r.snapshots.map(snapshot => snapshot.elapsed_seconds)));
        const candidate = frames.find((snapshot, i) => {
            if (!snapshot.diagnostics) return false;
            const next = frames[i + 1]?.elapsed_seconds ?? maxTime;
            const tick = Math.round((snapshot.elapsed_seconds + next) / 2 / maxTime * 1000);
            const t = tick / 1000 * maxTime;
            return t >= snapshot.elapsed_seconds && (i + 1 === frames.length || t < next);
        });
        assert.ok(candidate, 'expected a retained diagnostic snapshot');
        const index = frames.indexOf(candidate);
        const next = frames[index + 1]?.elapsed_seconds ?? maxTime;
        await page.locator('#time-scale').selectOption('linear');
        await page.locator('#timeline').fill(String(Math.round((candidate.elapsed_seconds + next) / 2 / maxTime * 1000)));
        await page.locator('#contacts').check();
        const bestRadius = await page.locator('#radius').innerText();
        await page.locator('#trial').check();
        await page.waitForFunction(() => document.querySelector('#canvas-mode').textContent.includes('RAW TRIAL'));
        assert.match(await page.locator('#feasible').innerText(), /raw trial/i);
        assert.equal(await page.locator('#radius').innerText(), bestRadius);
        await page.locator('#objective').check();
        await page.waitForFunction(() => /Objective: dashed, signed log/.test(document.querySelector('#history').textContent));
        await page.locator('#trial').uncheck(); await page.locator('#contacts').uncheck(); await page.locator('#objective').uncheck();
        await reset();
        assert.equal((await state()).runs.length, 0);
        await page.waitForTimeout(300);
        assert.equal((await state()).runs.length, 0);
        await upload(input50); await page.locator('#compare').selectOption('compare');
        await page.locator('#budget').fill('10'); await page.locator('#run').click();
        await wait(() => !document.querySelector('#pause').disabled);
        await reset();
        await page.waitForTimeout(600);
        const resetState = await state();
        assert.equal(resetState.phase, 'idle');
        assert.equal(resetState.runs.length, 0, 'late events must not repopulate reset state');
    });
    await test('pure B variant', async () => {
        await upload(input10); await page.locator('#compare').selectOption('b');
        await page.locator('#variant').selectOption('3'); await page.locator('#budget').fill('0.1');
        await page.locator('#run').click();
        await page.waitForFunction(() => window.__packingState().phase === 'complete', null, { timeout: 30000 });
        const s = await state(); assert.equal(s.runs.length, 1); assert.equal(Number(s.runs[0].algorithm), 3);
    });
    await test('early search detail and slow checkpoint replay', async () => {
        const recorded = await state();
        const early = recorded.runs[0].snapshots.filter(s => s.elapsed_seconds < 0.1 && s.diagnostics);
        assert.ok(early.length >= 2, `early accepted search positions must be retained: ${JSON.stringify({phase: recorded.phase, activeIndex: recorded.activeIndex, config: recorded.runs[0].config, status: recorded.runs[0].status, frames: recorded.runs[0].snapshots.map(s => ({t: s.elapsed_seconds, iterations: s.iterations, diagnostics: !!s.diagnostics, finished: s.finished, termination: s.termination}))})}`);
        assert.ok(new Set(early.map(s => s.diagnostics.objective)).size >= 2);
        await page.locator('#time-scale').selectOption('early');
        await page.locator('#replay').click();
        await page.waitForTimeout(500);
        assert.equal((await state()).phase, 'replaying', 'a short solve needs a watchable replay');
        await page.locator('#replay').click();
        const original = await download('#export-run');
        const duplicateTimes = JSON.parse(original);
        duplicateTimes.runs[0].snapshots.forEach(s => { s.elapsed_seconds = 0; });
        const referenceRun = structuredClone(duplicateTimes.runs[0]); referenceRun.algorithm = 0; referenceRun.config.algorithm = 0;
        duplicateTimes.runs.unshift(referenceRun);
        duplicateTimes.view = {selectedIndex: 1, selectedTime: 0, selectedFrameIndex: 0};
        await page.locator('#import-run').setInputFiles({name: 'same-millisecond.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(duplicateTimes))});
        await page.waitForFunction(() => window.__packingState().selectedIndex === 1 && window.__packingState().selectedFrameIndex === 0);
        await page.locator('#replay').click();
        await page.waitForTimeout(300);
        assert.equal((await state()).selectedFrameIndex, 0, 'replay must not jump ahead to future equal-time checkpoints in the other run');
        await page.locator('#replay').click();
        await page.waitForFunction(expected => Math.abs(Number(document.querySelector('#radius').textContent) - expected) < 0.0001, duplicateTimes.runs[1].snapshots[0].radius);
        const first = await page.locator('#radius').textContent();
        await page.locator('#next-frame').click();
        assert.equal((await state()).selectedFrameIndex, 1);
        await page.waitForFunction(expected => Math.abs(Number(document.querySelector('#radius').textContent) - expected) < 0.0001, duplicateTimes.runs[1].snapshots[1].radius);
        assert.notEqual(await page.locator('#radius').textContent(), first, 'same-time checkpoints must remain separately inspectable');
        await page.locator('#previous-frame').click();
        assert.equal((await state()).selectedFrameIndex, 0);
        const restored = JSON.parse(original); restored.view = {selectedIndex: 0, selectedTime: recorded.runs[0].snapshots.at(-1).elapsed_seconds};
        await page.locator('#import-run').setInputFiles({name: 'original.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(restored))});
        await page.waitForFunction(() => window.__packingState().selectedFrameIndex === null);
    });
    await test('accessible responsive controls', async () => {
        await mkdir(screenshotPath, { recursive: true });
        for (const width of [360, 1440]) {
            await page.setViewportSize({ width, height: 900 });
            assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `horizontal overflow at ${width}`);
            for (const id of ['run', 'pause', 'resume', 'cancel', 'reset', 'replay', 'export-solution', 'export-run']) {
                const button = page.locator(`#${id}`);
                assert.equal(await button.evaluate(el => el.tagName), 'BUTTON');
                assert.match((await button.innerText()).trim(), /\S/);
            }
            for (const id of ['preset', 'input', 'upload', 'compare', 'variant', 'seed', 'budget', 'timeline']) {
                assert.ok(await page.locator(`#${id}`).evaluate(el => (el.labels?.length || el.getAttribute('aria-label') || el.getAttribute('aria-labelledby'))), `${id} needs label`);
            }
            await page.keyboard.press('Tab');
            assert.ok(await page.evaluate(() => document.activeElement !== document.body));
            await page.screenshot({ path: path.join(screenshotPath, `${browserName}-${width}.png`), fullPage: true });
        }
    });
    await test('performance and bounded trace', async () => {
        await page.setViewportSize({ width: 1440, height: 900 });
        await reset(); await upload(input50); await page.locator('#compare').selectOption('a');
        await page.locator('#budget').fill('60'); await page.locator('#run').click();
        await wait(() => !document.querySelector('#pause').disabled);
        const start = Date.now(); let operations = 0;
        while (Date.now() - start < stressSeconds * 1000) {
            await page.locator('#pause').click();
            await wait(() => !document.querySelector('#resume').disabled);
            await page.locator('#resume').click();
            await wait(() => !document.querySelector('#pause').disabled);
            operations += 2;
            await page.waitForTimeout(500);
        }
        const perf = await stats();
        report.performance = { ...perf, operations, viewport: { width: 1440, height: 900 }, memory: await page.evaluate(() => performance.memory ? { usedJSHeapSize: performance.memory.usedJSHeapSize, totalJSHeapSize: performance.memory.totalJSHeapSize } : null) };
        assert.ok(perf.traceBytes <= 4 * 1024 * 1024);
        assert.ok(perf.snapshotCount <= 512);
        const p95 = values => { assert.ok(values.length); return [...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1]; };
        if (stressSeconds >= 30) {
            assert.ok(operations >= 20);
            assert.ok(p95(perf.controls) < 100, `control p95 ${p95(perf.controls)} ms`);
            assert.ok(p95(perf.frames) < 33, `frame p95 ${p95(perf.frames)} ms`);
        }
        await page.locator('#cancel').click();
    });
    assert.deepEqual(failures, [], 'uncaught browser errors');
    report.passed = true;
} catch (error) {
    report.passed = false; report.error = error.stack; throw error;
} finally {
    if (browser) await browser.close();
    server.kill();
    await writeFile(reportPath, JSON.stringify(report, null, 2) + '\n');
    console.log(`Report: ${reportPath}`);
}
