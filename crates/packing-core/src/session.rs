//! Resumable, host-clock-driven solver. Each work unit is one oracle call or
//! one bounded transition; accepted BFGS and line-search state survives yields.
use crate::numerics::{
    evaluate, initialize, initialize_portable, min_scale, min_scale_portable, repair,
    repair_portable, value, Evaluation, PenaltySchedule, PenaltyWeights, SplitMix64,
};
use crate::solver::{
    consider, export_candidate, line_fallback, Algorithm, SolveOptions, SolveResult, StopReason,
};
use crate::{Instance, PackingError, Point, Solution};

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |s, (x, y)| s + x * y)
}
fn identity(n: usize) -> Vec<f64> {
    let mut h = vec![0.; n * n];
    for i in 0..n {
        h[i * n + i] = 1.;
    }
    h
}
#[derive(Clone, Copy)]
enum Phase {
    Restart,
    Initial,
    Callback,
    Refresh(bool),
    Direction,
    SearchStart,
    BracketValue,
    BracketGradient,
    ZoomValue,
    ZoomGradient,
    Accepted,
    FinishLocal,
}
struct Local {
    x: Vec<f64>,
    current: Option<Evaluation>,
    current_weights: Option<PenaltyWeights>,
    norm: f64,
    h: Vec<f64>,
    previous: Option<f64>,
    schedule: PenaltySchedule,
    iterations: usize,
    direction: Vec<f64>,
    f0: f64,
    df0: f64,
    ta: f64,
    tb: f64,
    fta: f64,
    ft: f64,
    a: f64,
    b: f64,
    fa: f64,
    t: f64,
    k: usize,
    step: f64,
}
impl Local {
    fn new(x: Vec<f64>, algorithm: Algorithm) -> Self {
        let mut schedule = PenaltySchedule::default();
        match algorithm {
            Algorithm::EarlyPenalty => schedule.gradient_tolerance_squared = 1e-3,
            Algorithm::TightPenalty => schedule.violation_tolerance = 1e-6,
            _ => (),
        };
        let n = x.len();
        Self {
            x,
            current: None,
            current_weights: None,
            norm: 0.,
            h: identity(n),
            previous: None,
            schedule,
            iterations: 0,
            direction: vec![],
            f0: 0.,
            df0: 0.,
            ta: 0.,
            tb: 0.,
            fta: 0.,
            ft: 0.,
            a: 0.,
            b: 0.,
            fa: 0.,
            t: 0.,
            k: 0,
            step: 0.,
        }
    }
    fn at(&self, t: f64) -> Vec<f64> {
        self.x
            .iter()
            .zip(&self.direction)
            .map(|(x, d)| x + t * d)
            .collect()
    }
}
/// Read-only description of the accepted local optimization state.
///
/// The trial radius and points need not form a feasible packing. The objective
/// and squared gradient correspond to the given penalty weights.
#[derive(Debug, Clone)]
pub struct SessionDiagnostics {
    pub objective: f64,
    pub trial_radius: f64,
    pub points: Vec<Point>,
    pub containment_weight: f64,
    pub pairwise_weight: f64,
    pub gradient_squared: f64,
}

/// An incrementally driven optimization with a checked incumbent at all times.
pub struct SolverSession {
    instance: Instance,
    options: SolveOptions,
    algorithm: Algorithm,
    rng: SplitMix64,
    best: (Solution, String),
    result: SolveResult,
    phase: Phase,
    local: Option<Local>,
    finished: bool,
    portable: bool,
}
impl SolverSession {
    /// Build a session and its independently checked fallback.
    pub fn new(
        instance: Instance,
        options: SolveOptions,
        algorithm: Algorithm,
    ) -> Result<Self, PackingError> {
        Self::new_impl(instance, options, algorithm, false)
    }
    /// Build a session using pinned software math for cross-target reproducibility.
    /// This can follow a different trajectory from the platform-math native solver.
    pub fn new_portable(
        instance: Instance,
        options: SolveOptions,
        algorithm: Algorithm,
    ) -> Result<Self, PackingError> {
        Self::new_impl(instance, options, algorithm, true)
    }
    fn new_impl(
        instance: Instance,
        options: SolveOptions,
        algorithm: Algorithm,
        portable: bool,
    ) -> Result<Self, PackingError> {
        if options
            .time_limit_seconds
            .is_some_and(|x| !x.is_finite() || x < 0.)
        {
            return Err(PackingError::InvalidConfiguration(
                "time limit must be finite and nonnegative",
            ));
        }
        if instance.len() > 50 {
            return Err(PackingError::InvalidConfiguration(
                "version A supports at most fifty circles",
            ));
        }
        let best = export_candidate(&instance, line_fallback(&instance)?)?;
        let trivial = instance.len() == 1;
        let result = SolveResult {
            solution: best.0.clone(),
            output: best.1.clone(),
            evaluations: 0,
            iterations: 0,
            restarts: 0,
            initialization_fallbacks: 0,
            numerical_failures: 0,
            elapsed_seconds: 0.,
            termination: if trivial {
                StopReason::Trivial
            } else {
                StopReason::RestartLimit
            },
        };
        Ok(Self {
            instance,
            options,
            algorithm,
            rng: SplitMix64::new(options.seed),
            best,
            result,
            phase: Phase::Restart,
            local: None,
            finished: trivial,
            portable,
        })
    }
    /// Perform at most `max_work` bounded units, using host-provided active time.
    pub fn step(&mut self, max_work: usize, elapsed_seconds: f64) -> Result<(), PackingError> {
        if !elapsed_seconds.is_finite() || elapsed_seconds < self.result.elapsed_seconds {
            return Err(PackingError::InvalidConfiguration(
                "elapsed time must be finite and monotonic",
            ));
        }
        self.result.elapsed_seconds = elapsed_seconds;
        if !self.finished
            && self
                .options
                .time_limit_seconds
                .is_some_and(|t| elapsed_seconds >= t)
        {
            self.stop(StopReason::TimeLimit);
        }
        for _ in 0..max_work {
            if self.finished {
                break;
            }
            self.unit();
        }
        Ok(())
    }
    /// Stop and retain a repaired copy of the last accepted state.
    pub fn cancel(&mut self) -> Result<(), PackingError> {
        if !self.finished {
            self.stop(StopReason::Cancelled);
        }
        Ok(())
    }
    /// Whether a limit, cancellation, or trivial instance ended this session.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    /// Return the current checked incumbent and counters.
    pub fn result(&self) -> SolveResult {
        let mut r = self.result.clone();
        r.solution = self.best.0.clone();
        r.output = self.best.1.clone();
        r
    }
    /// Clone the accepted local state when its evaluation matches current weights.
    /// Returns `None` before evaluation, during a pending refresh, or for
    /// non-finite diagnostic values. This performs no objective evaluation.
    pub fn diagnostics(&self) -> Option<SessionDiagnostics> {
        let l = self.local.as_ref()?;
        let e = l.current.as_ref()?;
        let weights = l.current_weights?;
        if weights != l.schedule.weights {
            return None;
        }
        let gradient_squared = dot(&e.gradient, &e.gradient);
        if !e.value.is_finite()
            || !gradient_squared.is_finite()
            || !weights.containment.is_finite()
            || !weights.pairwise.is_finite()
            || l.x.iter().any(|x| !x.is_finite())
        {
            return None;
        }
        Some(SessionDiagnostics {
            objective: e.value,
            trial_radius: l.x[0],
            points: l.x[1..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| Point::new(p[0], p[1]))
                .collect(),
            containment_weight: weights.containment,
            pairwise_weight: weights.pairwise,
            gradient_squared,
        })
    }
    /// Return the instance being solved.
    pub fn instance(&self) -> &Instance {
        &self.instance
    }
    fn repair_local(&mut self) {
        if let Some(l) = &self.local {
            let points: Vec<_> = l.x[1..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| Point::new(p[0], p[1]))
                .collect();
            if let Ok(s) = if self.portable {
                repair_portable(&self.instance, &points, 1.00005)
            } else {
                repair(&self.instance, &points, 1.00005)
            } {
                consider(&self.instance, s, &mut self.best);
            }
        }
    }
    fn stop(&mut self, reason: StopReason) {
        self.repair_local();
        self.result.termination = reason;
        self.finished = true;
    }
    fn limit(&mut self, iterations: bool) -> bool {
        if self.result.evaluations >= self.options.evaluation_limit {
            self.stop(StopReason::EvaluationLimit);
            true
        } else if iterations && self.result.iterations >= self.options.iteration_limit {
            self.stop(StopReason::IterationLimit);
            true
        } else {
            false
        }
    }
    fn fail(&mut self) {
        self.result.numerical_failures += 1;
        self.phase = Phase::FinishLocal;
    }
    fn unit(&mut self) {
        match self.phase {
            Phase::Restart => {
                if self.limit(true) {
                    return;
                }
                if self.result.restarts >= self.options.restart_limit {
                    self.stop(StopReason::RestartLimit);
                    return;
                }
                self.result.restarts += 1;
                let init = match if self.portable {
                    initialize_portable(&self.instance, 1, &mut self.rng)
                } else {
                    initialize(&self.instance, 1, &mut self.rng)
                } {
                    Ok(x) => x,
                    Err(_) => {
                        self.result.numerical_failures += 1;
                        return;
                    }
                };
                self.result.initialization_fallbacks += init.fallback_count;
                if let Ok(s) = if self.portable {
                    repair_portable(&self.instance, &init.points, 1.00005)
                } else {
                    repair(&self.instance, &init.points, 1.00005)
                } {
                    consider(&self.instance, s, &mut self.best);
                }
                if self.limit(true) {
                    return;
                }
                let scale = match if self.portable {
                    min_scale_portable(&self.instance, &init.points)
                } else {
                    min_scale(&self.instance, &init.points)
                } {
                    Ok(x) if x > 0. => x,
                    _ => {
                        self.result.numerical_failures += 1;
                        return;
                    }
                };
                let points: Vec<_> = init
                    .points
                    .iter()
                    .map(|p| Point::new(p.x * 0.999 / scale, p.y * 0.999 / scale))
                    .collect();
                let radius = points
                    .iter()
                    .zip(self.instance.radii())
                    .fold(0.0_f64, |r, (p, ri)| {
                        r.max(crate::numerics::hypot(p.x, p.y, self.portable) + ri)
                    });
                let mut x = vec![radius - 1e-3];
                x.extend(points.iter().flat_map(|p| [p.x, p.y]));
                self.local = Some(Local::new(x, self.algorithm));
                self.phase = Phase::Initial;
            }
            Phase::FinishLocal => {
                self.repair_local();
                self.local = None;
                self.phase = Phase::Restart;
            }
            Phase::Callback => {
                if self.limit(true) {
                    return;
                }
                let l = self.local.as_mut().unwrap();
                if l.iterations
                    >= if self.algorithm == Algorithm::ShortRestarts {
                        100
                    } else {
                        usize::MAX
                    }
                {
                    self.phase = Phase::FinishLocal;
                    return;
                }
                match if self.portable {
                    l.schedule.update_portable(&self.instance, &l.x, l.norm)
                } else {
                    l.schedule.update(&self.instance, &l.x, l.norm)
                } {
                    Ok(a) => {
                        if !a.terminate {
                            self.result.iterations += 1;
                        }
                        if a.refresh {
                            self.phase = Phase::Refresh(a.terminate);
                        } else if a.terminate {
                            self.phase = Phase::FinishLocal;
                        } else {
                            self.phase = Phase::Direction;
                        }
                    }
                    Err(_) => self.fail(),
                }
            }
            Phase::Direction => {
                let l = self.local.as_mut().unwrap();
                l.iterations += 1;
                let n = l.x.len();
                l.direction = vec![0.; n];
                for i in 0..n {
                    for j in 0..n {
                        l.direction[i] -= l.h[i * n + j] * l.current.as_ref().unwrap().gradient[j];
                    }
                }
                let slope = dot(&l.direction, &l.current.as_ref().unwrap().gradient);
                if !slope.is_finite() {
                    self.fail();
                } else if slope > -1e-6 * l.norm {
                    l.h = identity(n);
                    self.phase = Phase::Callback;
                } else {
                    self.phase = Phase::SearchStart;
                }
            }
            Phase::Initial
            | Phase::Refresh(_)
            | Phase::SearchStart
            | Phase::BracketValue
            | Phase::BracketGradient
            | Phase::ZoomValue
            | Phase::ZoomGradient
            | Phase::Accepted => {
                let l = self.local.as_ref().unwrap();
                let phase = self.phase;
                let point = match phase {
                    Phase::SearchStart => l.at(0.),
                    Phase::BracketValue | Phase::BracketGradient => l.at(l.tb),
                    Phase::ZoomValue | Phase::ZoomGradient => l.at(l.t),
                    Phase::Accepted => l.at(l.step),
                    _ => l.x.clone(),
                };
                if point.iter().any(|x| !x.is_finite()) {
                    self.fail();
                    return;
                }
                if self.limit(false) {
                    return;
                }
                self.result.evaluations += 1;
                let l = self.local.as_mut().unwrap();
                if matches!(phase, Phase::BracketValue | Phase::ZoomValue) {
                    let f = match value(&self.instance, &point, l.schedule.weights) {
                        Ok(f) if f.is_finite() => f,
                        _ => {
                            self.fail();
                            return;
                        }
                    };
                    l.ft = f;
                    match phase {
                        Phase::BracketValue => {
                            if f > l.f0 + 0.1 * l.tb * l.df0 || (l.k > 0 && f >= l.fta) {
                                l.a = l.ta;
                                l.b = l.tb;
                                l.fa = l.fta;
                                l.k = 0;
                                l.t = 0.5 * (l.a + l.b);
                                self.phase = Phase::ZoomValue;
                            } else {
                                self.phase = Phase::BracketGradient;
                            }
                        }
                        _ => {
                            if f > l.f0 + 0.1 * l.t * l.df0 || f >= l.fa {
                                l.b = l.t;
                                l.k += 1;
                                if l.k >= 50 {
                                    self.fail();
                                } else {
                                    l.t = 0.5 * (l.a + l.b);
                                }
                            } else {
                                self.phase = Phase::ZoomGradient;
                            }
                        }
                    }
                    return;
                }
                let e = match evaluate(&self.instance, &point, l.schedule.weights) {
                    Ok(e) if e.value.is_finite() && e.gradient.iter().all(|x| x.is_finite()) => e,
                    _ => {
                        self.fail();
                        return;
                    }
                };
                match phase {
                    Phase::Initial => {
                        l.norm = dot(&e.gradient, &e.gradient);
                        if !l.norm.is_finite() {
                            self.fail();
                        } else {
                            l.current = Some(e);
                            l.current_weights = Some(l.schedule.weights);
                            self.phase = if l.norm == 0. {
                                Phase::FinishLocal
                            } else {
                                Phase::Callback
                            };
                        }
                    }
                    Phase::Refresh(terminate) => {
                        l.current = Some(e);
                        l.current_weights = Some(l.schedule.weights);
                        self.phase = if terminate {
                            Phase::FinishLocal
                        } else {
                            Phase::Direction
                        };
                    }
                    Phase::SearchStart => {
                        l.f0 = e.value;
                        l.df0 = dot(&e.gradient, &l.direction);
                        if !l.df0.is_finite() || l.df0 >= 0. {
                            self.fail();
                            return;
                        }
                        l.tb = l
                            .previous
                            .map(|fp| (1.01 * 2.0 * (l.f0 - fp) / l.df0).min(1.))
                            .unwrap_or(1.);
                        if !l.tb.is_finite() || l.tb <= 0. {
                            l.tb = 1.;
                        }
                        l.ta = 0.;
                        l.fta = l.f0;
                        l.k = 0;
                        self.phase = Phase::BracketValue;
                    }
                    Phase::BracketGradient => {
                        let d = dot(&e.gradient, &l.direction);
                        if !d.is_finite() {
                            self.fail();
                        } else if d.abs() <= -0.95 * l.df0 {
                            l.step = l.tb;
                            self.phase = Phase::Accepted;
                        } else if d >= 0. {
                            l.a = l.tb;
                            l.b = l.ta;
                            l.fa = l.ft;
                            l.k = 0;
                            l.t = 0.5 * (l.a + l.b);
                            self.phase = Phase::ZoomValue;
                        } else {
                            l.ta = l.tb;
                            l.fta = l.ft;
                            l.tb *= 2.;
                            l.k += 1;
                            if !l.tb.is_finite() || l.k >= 100 {
                                self.fail();
                            } else {
                                self.phase = Phase::BracketValue;
                            }
                        }
                    }
                    Phase::ZoomGradient => {
                        let d = dot(&e.gradient, &l.direction);
                        if !d.is_finite() {
                            self.fail();
                        } else if d.abs() <= -0.95 * l.df0 {
                            l.step = l.t;
                            self.phase = Phase::Accepted;
                        } else {
                            if d * (l.b - l.a) >= 0. {
                                l.b = l.a;
                            }
                            l.a = l.t;
                            l.fa = l.ft;
                            l.k += 1;
                            if l.k >= 50 {
                                self.fail();
                            } else {
                                l.t = 0.5 * (l.a + l.b);
                                self.phase = Phase::ZoomValue;
                            }
                        }
                    }
                    Phase::Accepted => {
                        let next_norm = dot(&e.gradient, &e.gradient);
                        if !next_norm.is_finite() {
                            self.fail();
                            return;
                        }
                        let old = l.current.as_ref().unwrap();
                        let change: Vec<_> = e
                            .gradient
                            .iter()
                            .zip(&old.gradient)
                            .map(|(a, b)| a - b)
                            .collect();
                        let n = l.x.len();
                        let mut product = vec![0.; n];
                        // Keep the indexed scalar order aligned with the reference BFGS kernel.
                        #[allow(clippy::needless_range_loop)]
                        for i in 0..n {
                            for j in 0..n {
                                product[i] += l.h[i * n + j] * change[j];
                            }
                        }
                        let q = dot(&product, &change);
                        let curvature = l.step * dot(&change, &l.direction);
                        if !curvature.is_finite() || curvature <= 0. || !q.is_finite() {
                            l.h = identity(n);
                        } else {
                            let rho = 1. / curvature;
                            for i in 0..n {
                                for j in 0..n {
                                    l.h[i * n + j] += l.step
                                        * rho
                                        * ((1. + rho * q)
                                            * l.step
                                            * l.direction[i]
                                            * l.direction[j]
                                            - l.direction[i] * product[j]
                                            - l.direction[j] * product[i]);
                                }
                            }
                            if l.h.iter().any(|x| !x.is_finite()) {
                                l.h = identity(n);
                            }
                        }
                        l.previous = Some(old.value);
                        l.x = point;
                        l.current = Some(e);
                        l.current_weights = Some(l.schedule.weights);
                        l.norm = next_norm;
                        self.phase = if next_norm == 0. {
                            Phase::FinishLocal
                        } else {
                            Phase::Callback
                        };
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}
