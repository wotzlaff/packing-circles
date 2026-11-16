import { timeFraction, windowStart, visibleSnapshots } from "./timeline.js";

const colors = { a: "#df806e", b: "#59b5ac", ink: "#20384e", red: "#c34e41" };
const finite = (x) => Number.isFinite(x);

/** Draw a recorded feasible packing, or the current raw trial when requested. */
export function drawPacking(
  canvas,
  snapshot,
  { contacts = false, trial = false } = {},
) {
  const start = performance.now();
  const box = canvas.getBoundingClientRect();
  const width = box.width || canvas.clientWidth || 300,
    height = box.height || canvas.clientHeight || 300;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.max(1, Math.round(width * dpr));
  canvas.height = Math.max(1, Math.round(height * dpr));
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);
  const result = {
    contacts: 0,
    overlaps: 0,
    containmentViolations: 0,
    renderMs: 0,
  };
  if (!snapshot) {
    ctx.fillStyle = "#97a6a0";
    ctx.textAlign = "center";
    ctx.font = "13px sans-serif";
    ctx.fillText("Your packing will appear here", width / 2, height / 2);
    result.renderMs = performance.now() - start;
    return result;
  }
  const diagnostic = trial && snapshot.diagnostics;
  const points = diagnostic?.points || snapshot.points || [];
  const radii = snapshot.radii || [];
  const radius =
    diagnostic && finite(diagnostic.trial_radius)
      ? diagnostic.trial_radius
      : snapshot.radius;
  let extent = finite(radius) ? Math.abs(radius) : 0;
  for (let i = 0; i < Math.min(points.length, radii.length); i++) {
    const [x, y] = points[i] || [];
    const r = radii[i];
    if (finite(x) && finite(y) && finite(r))
      extent = Math.max(
        extent,
        Math.abs(x) + Math.max(0, r),
        Math.abs(y) + Math.max(0, r),
      );
  }
  extent = Math.max(extent, 1e-12);
  const scale = Math.max(1, Math.min(width, height) - 70) / (2 * extent);
  const px = (x) => width / 2 + x * scale,
    py = (y) => height / 2 - y * scale;
  const contactTolerance = Math.max(1, Math.abs(radius || 0)) * 0.003;
  const feasibilityTolerance =
    1e-9 *
    Math.max(1, Math.abs(radius || 0), ...radii.filter(finite).map(Math.abs));
  const valid = (p, r) =>
    Array.isArray(p) && finite(p[0]) && finite(p[1]) && finite(r) && r >= 0;
  const bad = new Set();
  for (let i = 0; i < points.length; i++) {
    if (!valid(points[i], radii[i])) continue;
    if (
      !finite(radius) ||
      radius < 0 ||
      Math.hypot(...points[i]) + radii[i] > radius + feasibilityTolerance
    ) {
      result.containmentViolations++;
      bad.add(i);
    }
    for (let j = 0; j < i; j++) {
      if (!valid(points[j], radii[j])) continue;
      const gap =
        Math.hypot(points[i][0] - points[j][0], points[i][1] - points[j][1]) -
        radii[i] -
        radii[j];
      if (gap < -feasibilityTolerance) {
        result.overlaps++;
        bad.add(i);
        bad.add(j);
      }
      if (Math.abs(gap) <= contactTolerance) {
        result.contacts++;
        if (contacts) {
          ctx.beginPath();
          ctx.moveTo(px(points[i][0]), py(points[i][1]));
          ctx.lineTo(px(points[j][0]), py(points[j][1]));
          ctx.strokeStyle = "#9bb4ab";
          ctx.lineWidth = 1;
          ctx.stroke();
        }
      }
    }
  }
  if (finite(radius) && radius >= 0) {
    ctx.beginPath();
    ctx.arc(width / 2, height / 2, radius * scale, 0, Math.PI * 2);
    ctx.strokeStyle = colors.ink;
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }
  const color =
    snapshot.algorithm && snapshot.algorithm !== 0 ? colors.b : colors.a;
  points.forEach((p, i) => {
    if (!valid(p, radii[i])) return;
    ctx.beginPath();
    ctx.arc(px(p[0]), py(p[1]), radii[i] * scale, 0, Math.PI * 2);
    ctx.fillStyle = color + "80";
    ctx.fill();
    ctx.strokeStyle = trial && bad.has(i) ? colors.red : color;
    ctx.lineWidth = trial && bad.has(i) ? 2 : 1;
    ctx.stroke();
  });
  result.renderMs = performance.now() - start;
  return result;
}

const ns = "http://www.w3.org/2000/svg";
function element(name, attrs, text) {
  const node = document.createElementNS(ns, name);
  for (const [key, value] of Object.entries(attrs))
    node.setAttribute(key, value);
  if (text != null) node.textContent = text;
  return node;
}
/** Format radius ticks distinctly even for very small improvements. */
export function radiusTickFormatter(span, values) {
  const step = Math.abs(span) / 3;
  let digits = Math.max(0, Math.min(12, Math.ceil(-Math.log10(step || 1)) + 1));
  while (digits < 12 && new Set(values.map((v) => v.toFixed(digits))).size < values.length) digits++;
  return (v) => Math.abs(v) >= 1e9 ? v.toExponential(Math.min(12, digits + 2)) : v.toFixed(digits);
}

/** Plot recorded best feasible radii as steps against active elapsed time. */
export function drawHistory(
  svg,
  runs,
  selectedTime,
  { objective = false, timeScale = "late", zoom = true } = {},
) {
  svg.replaceChildren();
  const width = Math.max(
    300,
    Math.min(800, svg.getBoundingClientRect().width || 800),
  );
  const mobile = width < 500;
  const add = (name, attrs, text) => {
    const n = element(name, attrs, text);
    svg.append(n);
    return n;
  };
  const series = (runs || [])
    .map((run) => ({
      ...run,
      snapshots: (run.snapshots || [])
        .filter((s) => finite(s.elapsed_seconds) && finite(s.radius))
        .slice()
        .sort((a, b) => a.elapsed_seconds - b.elapsed_seconds),
    }))
    .filter((r) => r.snapshots.length);
  const top = mobile ? 20 + series.length * 16 + (objective ? 16 : 0) : 35;
  const bottom = top + 145;
  svg.setAttribute("viewBox", `0 0 ${width} ${bottom + 50}`);
  if (!series.length) {
    add(
      "text",
      {
        x: width / 2,
        y: 110,
        "text-anchor": "middle",
        fill: "#97a6a0",
        "font-size": 13,
      },
      "Your improvement history will appear here",
    );
    return;
  }
  const all = series.flatMap((r) => r.snapshots),
    maxX = Math.max(...all.map((s) => s.elapsed_seconds), 0);
  const start = windowStart(maxX, timeScale);
  const visibleSeries = series.map((run) => ({ ...run, snapshots: visibleSnapshots(run.snapshots, start) }))
    .filter((run) => run.snapshots.length);
  const visible = visibleSeries.flatMap((run) => run.snapshots);
  const detail = zoom && timeScale !== "late"
    ? series.flatMap((run) => {
        const real = run.snapshots.filter((s) => !(s.evaluations === 0 && s.restarts === 0));
        return real.length ? real : run.snapshots;
      })
    : visible;
  let minY = Math.min(...detail.map((s) => s.radius)),
    maxY = Math.max(...detail.map((s) => s.radius));
  const flat = visibleSeries.every((run) =>
    run.snapshots.every((snapshot) => snapshot.radius === run.snapshots[0].radius),
  );
  const pad = zoom
    ? Math.max((maxY - minY) * 0.15, Math.max(1, Math.abs(maxY)) * 1e-12)
    : Math.max((maxY - minY) * 0.15, Math.abs(maxY) * 0.01, 0.001);
  minY -= pad;
  maxY += pad;
  const tickValues = Array.from({ length: 4 }, (_, i) => minY + ((maxY - minY) * i) / 3);
  const formatRadius = radiusTickFormatter(maxY - minY, tickValues);
  const left = Math.min(140, Math.max(56, ...tickValues.map((v) => formatRadius(v).length * 6 + 12)));
  const right = width - (objective ? 85 : 26);
  const x = (t) => left + timeFraction(t, maxX, timeScale) * (right - left),
    y = (v) => bottom - ((v - minY) / (maxY - minY)) * 145;
  for (let i = 0; i <= 3; i++) {
    const v = minY + ((maxY - minY) * i) / 3,
      yy = y(v);
    add("line", { x1: left, y1: yy, x2: right, y2: yy, stroke: "#e6eae5" });
    add(
      "text",
      {
        x: left - 9,
        class: "radius-tick",
        y: yy + 4,
        "text-anchor": "end",
        fill: "#84928d",
        "font-size": 10,
      },
      formatRadius(v),
    );
  }
  const ticks = [start];
  if (timeScale === "linear" || timeScale === "late") {
    const intervals = timeScale === "late" ? 3 : 4;
    for (let i = 1; i <= intervals; i++) ticks.push(start + ((maxX - start) * i) / intervals);
  } else {
    for (let t = 0.001; t < maxX && Number.isFinite(t); t *= 10) ticks.push(t);
    if (maxX > 0) ticks.push(maxX);
  }
  let lastTickX = -Infinity;
  ticks.forEach((t, i) => {
    const xx = x(t);
    if (i && xx - lastTickX < 48) return;
    lastTickX = xx;
    add("line", { x1: xx, y1: bottom, x2: xx, y2: bottom + 4, stroke: "#abb7b0" });
    add("text", {
      x: xx, y: bottom + 20,
      "text-anchor": i === 0 ? "start" : i === ticks.length - 1 ? "end" : "middle",
      fill: "#84928d", "font-size": 10,
    }, `${Number(t.toPrecision(3))} s`);
  });
  const clipId = `${svg.id || 'history'}-plot-clip`;
  const defs = add("defs", {});
  const clip = element("clipPath", { id: clipId });
  clip.append(element("rect", { x: left, y: top, width: right - left, height: 145 }));
  defs.append(clip);
  const plot = add("g", { "clip-path": `url(#${clipId})` });
  if (timeScale !== "late" && all.some((s) => s.radius < minY || s.radius > maxY)) {
    add("text", { x: left, y: bottom + 36, fill: "#84928d", "font-size": 10 },
      "Starting fallback outside detail range");
  }
  if (flat && timeScale === "late") {
    add("text", { x: left, y: bottom + 36, fill: "#84928d", "font-size": 10 }, "No radius improvement in this window");
  }
  visibleSeries.forEach((run, index) => {
    const color = run.algorithm === 0 ? colors.a : colors.b;
    let best = Infinity,
      path = "";
    run.snapshots.forEach((s, i) => {
      best = Math.min(best, s.radius);
      path += i
        ? ` H ${x(s.elapsed_seconds)} V ${y(best)}`
        : `M ${x(s.elapsed_seconds)} ${y(best)}`;
    });
    plot.append(element("path", { d: path, fill: "none", stroke: color, "stroke-width": 2.5 }));
    add(
      "text",
      {
        x: mobile ? left : left + 9 + index * 150,
        y: mobile ? 15 + index * 16 : 14,
        fill: color,
        "font-size": 11,
      },
      run.name || `Run ${index + 1}`,
    );
  });
  if (finite(selectedTime))
    add("line", {
      x1: x(Math.max(0, Math.min(maxX, selectedTime))),
      y1: top,
      x2: x(Math.max(0, Math.min(maxX, selectedTime))),
      y2: bottom,
      stroke: "#abb7b0",
      "stroke-dasharray": "3 4",
    });
  if (objective) {
    const vals = visible.map((s) => s.diagnostics?.objective).filter(finite);
    if (vals.length) {
      const transform = (v) => Math.sign(v) * Math.log1p(Math.abs(v));
      const lo = Math.min(...vals.map(transform)),
        hi = Math.max(...vals.map(transform));
      visibleSeries.forEach((run) => {
        let path = "";
        run.snapshots.forEach((s) => {
          const v = s.diagnostics?.objective;
          if (!finite(v)) return;
          path += `${path ? " L" : "M"} ${x(s.elapsed_seconds)} ${bottom - ((transform(v) - lo) / (hi - lo || 1)) * 145}`;
        });
        if (path)
          plot.append(element("path", {
            d: path,
            fill: "none",
            stroke: run.algorithm === 0 ? colors.a : colors.b,
            "stroke-width": 1.5,
            "stroke-dasharray": "4 4",
            opacity: 0.65,
          }));
      });
      for (let i = 0; i <= 3; i++) {
        const t = lo + ((hi - lo) * i) / 3;
        const value = Math.sign(t) * Math.expm1(Math.abs(t));
        const yy = bottom - (i * 145) / 3;
        add(
          "text",
          { x: right + 8, y: yy + 4, fill: "#84928d", "font-size": 10 },
          Number(value.toPrecision(3)).toString(),
        );
      }
      add(
        "text",
        {
          x: mobile ? left : width - 10,
          y: mobile ? 15 + series.length * 16 : 14,
          "text-anchor": mobile ? "start" : "end",
          fill: "#84928d",
          "font-size": 10,
        },
        "Objective: dashed, signed log",
      );
    }
  }
}
