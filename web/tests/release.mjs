import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {spawn} from 'node:child_process';
import {mkdtemp, readFile, readdir, rm, writeFile} from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {chromium} from 'playwright';

const args = process.argv.slice(2);
function option(name, fallback) { const i = args.indexOf(name); if (i < 0) return fallback; assert.ok(args[i + 1], `missing ${name}`); return args[i + 1]; }
const base = new URL(option('--url', 'https://wotzlaff.github.io/packing-circles/'));
assert.ok(['http:', 'https:'].includes(base.protocol));
if (!base.pathname.endsWith('/')) base.pathname += '/';
const reportPath = option('--report', '/tmp/circles-release.json');
const chrome = option('--chrome', '/tmp/circles-release-browsers/chrome-linux64/chrome');
const firefox = option('--firefox', '/tmp/circles-release-browsers/firefox/firefox');
const geckodriver = option('--geckodriver', '/tmp/circles-release-browsers/geckodriver');
const expected = option('--expected-commit', null);
const report = {url: base.href, measured_at: new Date().toISOString(), passed: false, cases: []};
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const hash = b => createHash('sha256').update(b).digest('hex');
async function get(url) { const r = await fetch(url, {signal: AbortSignal.timeout(30000)}); assert.ok(r.ok, `${url}: HTTP ${r.status}`); return r; }
async function test(name, fn) { try { const details = await fn(); report.cases.push({name, passed: true, ...(details ? {details} : {})}); console.log(`PASS ${name}`); } catch (e) { report.cases.push({name, passed: false, error: e.stack}); throw e; } }
function checkSolution(output, input) {
  const values = input.trim().split(/\s+/).map(Number), radii = values.slice(1);
  assert.equal(values[0], radii.length);
  const t = output.trim().split(/\s+/); assert.equal(t.length, 1 + radii.length * 3);
  assert.match(t[0], /^\d+\.\d{4}$/); const radius = Number(t[0]); assert.ok(Number.isFinite(radius) && radius > 0);
  const points = radii.map((r, i) => { assert.equal(Number(t[1 + i * 3]), r); const p = t.slice(2 + i * 3, 4 + i * 3); p.forEach(x => assert.match(x, /^-?\d+\.\d{4}$/)); return p.map(Number); });
  const tolerance = 1e-9 * Math.max(1, radius, ...radii);
  points.forEach(([x, y], i) => { assert.ok(Math.hypot(x, y) + radii[i] <= radius + tolerance, `containment ${i}`); for (let j = 0; j < i; j++) assert.ok(Math.hypot(x - points[j][0], y - points[j][1]) + tolerance >= radii[i] + radii[j], `overlap ${i}, ${j}`); });
}
async function waitFor(fn, description, timeout = 30000) { const end = Date.now() + timeout; while (Date.now() < end) { const value = await fn(); if (value) return value; await sleep(100); } throw new Error(`Timed out waiting for ${description}`); }

async function smoke(driver, label) {
  await test(`${label}: first load and reload`, async () => { await driver.navigate(base.href); await waitFor(() => driver.evaluate(() => !!document.querySelector('#run') && !document.querySelector('#run').disabled), 'worker ready'); await driver.reload(); await waitFor(() => driver.evaluate(() => !!document.querySelector('#run') && !document.querySelector('#run').disabled), 'worker ready after reload'); });
  await test(`${label}: sequential A and explicit B`, async () => {
    const input = await driver.evaluate(() => document.querySelector('#input').value);
    const result = await driver.evaluate(async () => {
      for (const [id, value] of [['compare','compare'], ['variant','2'], ['budget','0.5']]) { const el = document.getElementById(id); el.value = value; el.dispatchEvent(new Event('change', {bubbles:true})); }
      document.querySelector('#run').click();
      const end = Date.now() + 25000;
      const seen = [];
      while (Date.now() < end) { const status = document.querySelector('#status').textContent; if (/is running/.test(status) && !seen.includes(status)) seen.push(status); if (/Experiment complete/.test(status)) return {seen, tabs: [...document.querySelectorAll('#run-tabs button')].map(x => x.textContent)}; await new Promise(resolve => setTimeout(resolve, 40)); }
      throw new Error('comparison completion timed out');
    });
    assert.equal(result.tabs.length, 2); assert.match(result.tabs[0], /A/); assert.match(result.tabs[1], /B/);
    assert.equal(result.seen.length, 2, 'both algorithms must run sequentially'); assert.match(result.seen[0], /A/); assert.match(result.seen[1], /B/);
    for (let i = 0; i < 2; i++) { await driver.evaluate(i => document.querySelectorAll('#run-tabs button')[i].click(), i); const output = await driver.download(); checkSolution(output, input); }
    return result;
  });
}
let browser, gecko, geckoEndpoint, session, downloadDir;
try {
  await test('published build metadata and WASM hash', async () => {
    const html = await (await get(base)).text();
    const moduleSrc = html.match(/<script\b[^>]*type=["']module["'][^>]*src=["']([^"']+)["']/i)?.[1];
    assert.ok(moduleSrc, 'module script missing');
    const info = await (await get(new URL('build-info.json', new URL(moduleSrc, base)))).json();
    const wasm = Buffer.from(await (await get(new URL('packing_wasm.wasm', new URL(moduleSrc, base)))).arrayBuffer());
    assert.match(info.source_commit, /^[0-9a-f]{40}$/); assert.equal(hash(wasm), info.wasm_sha256);
    if (expected) assert.equal(info.source_commit, expected);
    report.build_info = info; return {source_commit: info.source_commit, wasm_sha256: info.wasm_sha256};
  });
  browser = await chromium.launch({executablePath: chrome, headless: true}); report.chrome_version = browser.version();
  const page = await browser.newPage({acceptDownloads: true});
  const chromeDriver = {
    navigate: url => page.goto(url, {waitUntil: 'load'}), reload: () => page.reload({waitUntil: 'load'}),
    evaluate: (fn, arg) => page.evaluate(fn, arg),
    download: async () => { const [file] = await Promise.all([page.waitForEvent('download', {timeout: 30000}), page.locator('#export-solution').click()]); return readFile(await file.path(), 'utf8'); },
  };
  await smoke(chromeDriver, 'Chrome'); await browser.close(); browser = null;
  downloadDir = await mkdtemp(path.join(os.tmpdir(), 'circles-firefox-download-'));
  const port = await new Promise((resolve, reject) => { const s = net.createServer(); s.once('error', reject); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => resolve(p)); }); });
  gecko = spawn(geckodriver, ['--host', '127.0.0.1', '--port', String(port)], {stdio: 'ignore'});
  const endpoint = `http://127.0.0.1:${port}`; geckoEndpoint = endpoint;
  await waitFor(async () => { try { return (await fetch(`${endpoint}/status`, {signal: AbortSignal.timeout(500)})).ok; } catch { return false; } }, 'geckodriver', 10000);
  async function command(method, route, body) { const r = await fetch(endpoint + route, {method, headers: {'Content-Type':'application/json'}, body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(45000)}); const json = await r.json(); if (!r.ok || json.value?.error) throw new Error(`WebDriver ${route}: ${JSON.stringify(json.value)}`); return json.value; }
  const started = await command('POST', '/session', {capabilities: {alwaysMatch: {browserName:'firefox', 'moz:firefoxOptions': {binary:firefox, args:['-headless'], prefs:{'browser.download.folderList':2, 'browser.download.dir':downloadDir, 'browser.download.useDownloadDir':true, 'browser.helperApps.neverAsk.saveToDisk':'text/plain,application/octet-stream', 'browser.download.alwaysOpenPanel':false}}}}});
  session = started.sessionId; report.firefox_version = started.capabilities.browserVersion;
  const call = (method, route, body) => command(method, `/session/${session}${route}`, body);
  const ff = {
    navigate: url => call('POST', '/url', {url}), reload: () => call('POST', '/refresh', {}),
    evaluate: (fn, arg) => call('POST', '/execute/async', {script: `const done = arguments[arguments.length - 1]; Promise.resolve((${fn.toString()})(arguments[0])).then(value => done({value}), error => done({error: String(error)}));`, args:[arg ?? null]}).then(x => { if (x.error) throw new Error(x.error); return x.value; }),
    download: async () => {
      const before = new Set(await readdir(downloadDir));
      await ff.evaluate(() => document.querySelector('#export-solution').click());
      return waitFor(async () => {
        const files = await readdir(downloadDir);
        const file = files.find(n => !before.has(n) && !n.endsWith('.part'));
        if (!file || files.some(n => n.endsWith('.part'))) return false;
        // Firefox creates the final empty placeholder before finishing its .part file.
        const output = await readFile(path.join(downloadDir, file), 'utf8');
        return output.trim() ? output : false;
      }, 'completed Firefox download');
    },
  };
  await smoke(ff, 'Firefox');
  await call('DELETE', '', undefined); session = null;
  report.passed = true;
} catch (error) { report.error = error.stack; process.exitCode = 1; }
finally {
  if (session && gecko) { try { await fetch(`${geckoEndpoint}/session/${session}`, {method:'DELETE', signal:AbortSignal.timeout(2000)}); } catch {} }
  if (browser) await browser.close().catch(() => {});
  if (gecko) gecko.kill();
  if (downloadDir) await rm(downloadDir, {recursive:true, force:true});
  await writeFile(reportPath, JSON.stringify(report, null, 2) + '\n'); console.log(`Report: ${reportPath}`);
}
