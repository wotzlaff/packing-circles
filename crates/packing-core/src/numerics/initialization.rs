//! Legacy rolling initialization with prefix-safe centering and deterministic RNG.

use super::NumericalError;
use crate::{Instance, Point};

/// Minimal random source required by initialization.
pub trait RandomSource {
    fn next_u64(&mut self) -> u64;
}

/// Small reproducible generator suitable for repeatable solver runs.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl RandomSource for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Initialization {
    pub points: Vec<Point>,
    pub fallback_count: usize,
}

/// Produces the legacy rolling placement, shuffling radii for each trial.
/// As in Version A, the final trial is returned (the historical best-value
/// update was missing). Every centering call receives only its placed prefix.
pub fn initialize<R: RandomSource>(
    instance: &Instance,
    trials: usize,
    rng: &mut R,
) -> Result<Initialization, NumericalError> {
    initialize_impl(instance, trials, rng, false)
}

/// Rolling placement with pinned software transcendental functions across targets.
pub fn initialize_portable<R: RandomSource>(
    instance: &Instance,
    trials: usize,
    rng: &mut R,
) -> Result<Initialization, NumericalError> {
    initialize_impl(instance, trials, rng, true)
}

fn initialize_impl<R: RandomSource>(
    instance: &Instance,
    trials: usize,
    rng: &mut R,
    portable: bool,
) -> Result<Initialization, NumericalError> {
    if trials == 0 {
        return Err(NumericalError::InvalidInput(
            "initialization needs at least one trial",
        ));
    }
    let n = instance.len();
    let mut result = Vec::new();
    let mut fallback_count = 0;
    for _ in 0..trials {
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = uniform_below(rng, i + 1)?;
            order.swap(i, j);
        }
        let radii: Vec<f64> = order.iter().map(|&i| instance.radii()[i]).collect();
        let mut placed = Vec::with_capacity(n);
        placed.push(Point::new(0.0, 0.0));
        if n > 1 {
            let separation = radii[0] + radii[1];
            if !separation.is_finite() {
                return Err(NumericalError::NonFinite);
            }
            placed.push(Point::new(separation, 0.0));
        }
        for j in 2..n {
            let mut candidates = rolling_candidates(&radii[..j], &placed, radii[j], portable);
            candidates.retain(|p| {
                p.x.is_finite()
                    && p.y.is_finite()
                    && prefix_feasible(&placed, &radii[..j], *p, radii[j], portable)
            });
            if candidates.is_empty() {
                fallback_count += 1;
                let right = placed
                    .iter()
                    .zip(&radii[..j])
                    .map(|(p, r)| p.x + r)
                    .fold(f64::NEG_INFINITY, f64::max);
                let p = Point::new(right + radii[j], 0.0);
                if !p.x.is_finite() {
                    return Err(NumericalError::NonFinite);
                }
                placed.push(p);
            } else {
                let chosen = candidates
                    .into_iter()
                    .min_by(|a, b| {
                        prefix_center(&radii[..=j], &placed_with(&placed, *a), portable)
                            .0
                            .total_cmp(
                                &prefix_center(&radii[..=j], &placed_with(&placed, *b), portable).0,
                            )
                    })
                    .ok_or(NumericalError::NonFinite)?;
                placed.push(chosen);
            }
        }
        let (_, center) = prefix_center(&radii, &placed, portable);
        if !center.x.is_finite() || !center.y.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        let mut original_order = vec![Point::default(); n];
        for (k, &original) in order.iter().enumerate() {
            original_order[original] = Point::new(placed[k].x - center.x, placed[k].y - center.y);
        }
        result = original_order;
    }
    Ok(Initialization {
        points: result,
        fallback_count,
    })
}

fn uniform_below<R: RandomSource>(rng: &mut R, bound: usize) -> Result<usize, NumericalError> {
    let bound = bound as u64;
    let threshold = bound.wrapping_neg() % bound;
    for _ in 0..64 {
        let x = rng.next_u64();
        if x >= threshold {
            return Ok((x % bound) as usize);
        }
    }
    Err(NumericalError::InvalidInput(
        "random source failed rejection sampling",
    ))
}

fn placed_with(points: &[Point], p: Point) -> Vec<Point> {
    let mut v = points.to_vec();
    v.push(p);
    v
}

/// Returns legacy's 20-step center and its enclosing-radius score.
fn prefix_center(radii: &[f64], points: &[Point], portable: bool) -> (f64, Point) {
    let mut best = f64::INFINITY;
    let mut best_center = Point::default();
    let mut center = Point::default();
    let (dx0, dy0) = (0.0, 0.0); // legacy previous-state remains zero throughout
    let mut step = 20.0;
    for _ in 0..20 {
        let mut value: f64 = 0.0;
        let (mut dx, mut dy) = (0.0, 0.0);
        for (&r, p) in radii.iter().zip(points) {
            let x = p.x - center.x;
            let y = p.y - center.y;
            let v = super::hypot(x, y, portable) + r;
            if v > value {
                value = v;
                dx = x;
                dy = y;
            }
        }
        if value < best {
            best = value;
            best_center = center;
        }
        dx = 0.9 * dx / value + 0.1 * dx0;
        dy = 0.9 * dy / value + 0.1 * dy0;
        center.x += dx * step;
        center.y += dy * step;
        step *= 0.9;
    }
    (best, best_center)
}

fn prefix_feasible(
    placed: &[Point],
    radii: &[f64],
    candidate: Point,
    radius: f64,
    portable: bool,
) -> bool {
    placed.iter().zip(radii).all(|(p, r)| {
        super::hypot(p.x - candidate.x, p.y - candidate.y, portable) + 1e-10 >= r + radius
    })
}

fn rolling_candidates(radii: &[f64], points: &[Point], radius: f64, portable: bool) -> Vec<Point> {
    let j = points.len();
    let mut i0 = 0;
    let mut rightmost = points[0].x + radii[0];
    for i in 1..j {
        let edge = points[i].x + radii[i];
        if edge > rightmost {
            i0 = i;
            rightmost = edge;
        }
    }
    let first = i0;
    let (mut x0, mut y0, mut r0) = (points[i0].x, points[i0].y, radii[i0]);
    let (mut phi_ref, mut steps) = (0.0_f64, 0_usize);
    let mut output = Vec::new();
    loop {
        let mut best_phi = f64::INFINITY;
        let mut next = None;
        for i in 0..j {
            if i == i0 {
                continue;
            }
            let dx = x0 - points[i].x;
            let dy = y0 - points[i].y;
            let d = super::hypot(dx, dy, portable);
            let denom = d * (r0 + radius);
            if d == 0.0 || !denom.is_finite() || denom == 0.0 {
                continue;
            }
            let rhs = 0.5 * ((radii[i] + radius).powi(2) - (r0 + radius).powi(2) - d * d) / denom;
            if rhs < -1.0 - 1e-10 || rhs > 1.0 + 1e-10 || !rhs.is_finite() {
                continue;
            }
            let phi = (2.0 * std::f64::consts::PI
                + if portable {
                    libm::atan2(dy, dx)
                } else {
                    dy.atan2(dx)
                }
                + if portable {
                    libm::acos(rhs.clamp(-1.0, 1.0))
                } else {
                    rhs.clamp(-1.0, 1.0).acos()
                }
                - phi_ref)
                .rem_euclid(2.0 * std::f64::consts::PI);
            if phi < best_phi {
                best_phi = phi;
                next = Some(i);
            }
        }
        if !best_phi.is_finite() {
            break;
        }
        let angle = phi_ref + best_phi;
        let candidate = Point::new(
            x0 + (r0 + radius)
                * if portable {
                    libm::cos(angle)
                } else {
                    angle.cos()
                },
            y0 + (r0 + radius)
                * if portable {
                    libm::sin(angle)
                } else {
                    angle.sin()
                },
        );
        output.push(candidate);
        let Some(i) = next else {
            break;
        };
        i0 = i;
        r0 = radii[i];
        x0 = points[i].x;
        y0 = points[i].y;
        phi_ref = if portable {
            libm::atan2(candidate.y - y0, candidate.x - x0)
        } else {
            (candidate.y - y0).atan2(candidate.x - x0)
        } - 1e-10;
        steps += 1;
        if i0 == first || steps >= j.saturating_mul(2).max(1) {
            break;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::prefix_center;
    use crate::Point;

    #[test]
    fn centering_matches_twenty_steps_with_zero_previous_momentum() {
        let radii = [1.0, 1.0, 1.0];
        let points = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(1.0, 1.5),
        ];
        let (radius, center) = prefix_center(&radii, &points, false);

        // Reference: twenty legacy updates, with dx0=dy0=0 left unchanged.
        assert!((center.x - 1.845_928_626_256_722_3).abs() < 1e-12);
        assert_eq!(center.y, 0.0);
        assert!((radius - 2.845_928_626_256_722_3).abs() < 1e-12);
        // Accumulating the previous update would instead return x ~= 1.74608.
        assert!((center.x - 1.746_081_993_302_753_2).abs() > 1e-3);
    }
}
