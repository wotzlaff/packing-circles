use packing_core::numerics::{initialize, repair, NumericalError, RandomSource, SplitMix64};
use packing_core::{validate, Instance, Point};

#[test]
fn initializes_and_repairs_all_development_sizes_with_two_seeds() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/data/development");
    for size in [10, 20, 30, 40, 50] {
        for index in 1..=10 {
            let input =
                std::fs::read_to_string(directory.join(format!("{size}_{index:02}.txt"))).unwrap();
            let instance = packing_core::parse_instance(&input).unwrap();
            for seed in [5, 42] {
                let result = initialize(&instance, 1, &mut SplitMix64::new(seed)).unwrap();
                let solution = repair(&instance, &result.points, 1.00005).unwrap();
                assert!(
                    validate(&instance, &solution).unwrap().is_feasible,
                    "initialization failed at N={size}, input={index}, seed={seed}"
                );
            }
        }
    }
}

#[test]
fn initialization_is_repeatable_and_preserves_original_order() {
    let instance = Instance::new(vec![1.0, 2.0, 0.5, 1.5]).unwrap();
    let mut a = SplitMix64::new(42);
    let mut b = SplitMix64::new(42);
    assert_eq!(
        initialize(&instance, 3, &mut a).unwrap(),
        initialize(&instance, 3, &mut b).unwrap()
    );
}

#[test]
fn handles_single_and_two_circle_instances() {
    let mut rng = SplitMix64::new(1);
    let one = initialize(&Instance::new(vec![3.0]).unwrap(), 1, &mut rng).unwrap();
    assert_eq!(one.points, vec![Point::new(0.0, 0.0)]);
    let two = initialize(&Instance::new(vec![1.0, 2.0]).unwrap(), 1, &mut rng).unwrap();
    assert_eq!(two.points.len(), 2);
    assert!((two.points[0].distance(two.points[1]) - 3.0).abs() < 1e-10);
}

#[test]
fn rejects_overflow_in_initial_pair_separation() {
    let instance = Instance::new(vec![f64::MAX, f64::MAX]).unwrap();
    let mut rng = SplitMix64::new(2);
    assert!(matches!(
        initialize(&instance, 1, &mut rng),
        Err(NumericalError::NonFinite)
    ));
}

#[test]
fn zero_trials_are_rejected() {
    let mut rng = SplitMix64::new(0);
    assert!(matches!(
        initialize(&Instance::new(vec![1.0]).unwrap(), 0, &mut rng),
        Err(NumericalError::InvalidInput(_))
    ));
}

#[test]
fn initializer_handles_equal_and_degenerate_radii_with_finite_fallback_if_needed() {
    let instance = Instance::new(vec![1.0; 12]).unwrap();
    let mut rng = SplitMix64::new(90);
    let init = initialize(&instance, 1, &mut rng).unwrap();
    assert!(init
        .points
        .iter()
        .all(|p| p.x.is_finite() && p.y.is_finite()));
    let solution = repair(&instance, &init.points, 1.0001).unwrap();
    assert!(validate(&instance, &solution).unwrap().is_feasible);
}

#[test]
fn random_source_trait_is_injectable() {
    struct Fixed(u64);
    impl RandomSource for Fixed {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(1);
            self.0
        }
    }
    let mut rng = Fixed(0);
    assert_eq!(
        initialize(&Instance::new(vec![1.0, 2.0, 3.0]).unwrap(), 1, &mut rng)
            .unwrap()
            .points
            .len(),
        3
    );
}

#[test]
fn records_fallback_when_the_rolling_tangencies_do_not_exist() {
    let instance = Instance::new(vec![1e200, 1e200, 1e200]).unwrap();
    let mut rng = SplitMix64::new(0);
    let init = initialize(&instance, 1, &mut rng).unwrap();
    assert!(init.fallback_count > 0);
    assert!(init
        .points
        .iter()
        .all(|point| point.x.is_finite() && point.y.is_finite()));
}

#[test]
fn pathological_rng_cannot_hang_rejection_sampling() {
    struct Zero;
    impl RandomSource for Zero {
        fn next_u64(&mut self) -> u64 {
            0
        }
    }
    let instance = Instance::new(vec![1.0, 1.0, 1.0]).unwrap();
    assert!(matches!(
        initialize(&instance, 1, &mut Zero),
        Err(NumericalError::InvalidInput(_))
    ));
}
