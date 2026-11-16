import {PackingWorker} from './solver.js';

const solver = new PackingWorker();
const element = (id) => document.getElementById(id);
function controls(running, paused = false) {
  element('run').disabled = false;
  element('pause').disabled = !running || paused;
  element('resume').disabled = !running || !paused;
  element('cancel').disabled = !running;
}
solver.addEventListener('message', ({data}) => {
  if (data.type === 'ready') {
    controls(false);
    element('status').textContent = 'Ready';
  } else if (data.type === 'error') {
    controls(false);
    element('status').textContent = data.error;
  } else if (data.snapshot) {
    const s = data.snapshot;
    const paused = data.type === 'paused';
    controls(!s.finished, paused);
    element('status').textContent = `${paused ? 'Paused' : s.termination} · radius ${s.radius.toFixed(4)} · ${s.evaluations} evaluations · ${s.elapsed_seconds.toFixed(2)} s`;
    element('solution').textContent = s.output;
  }
});
element('run').onclick = () => solver.start({
  input: element('input').value,
  algorithm: Number(element('algorithm').value), seed: '5', timeLimitSeconds: 5,
});
element('pause').onclick = () => solver.pause();
element('resume').onclick = () => solver.resume();
element('cancel').onclick = () => solver.cancel();
window.addEventListener('pagehide', () => solver.dispose());
