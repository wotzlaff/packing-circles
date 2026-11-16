use packing_core::numerics::{min_scale, repair, NumericalError};
use packing_core::{validate, Instance, Point};

#[test]
fn scales_pairwise_overlap_to_feasibility() {
    let instance = Instance::new(vec![1.0, 2.0, 1.0]).unwrap();
    let points = [
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(0.0, 1.0),
    ];
    assert!(min_scale(&instance, &points).unwrap() < 1.0);
    let solution = repair(&instance, &points, 1.001).unwrap();
    assert!(validate(&instance, &solution).unwrap().is_feasible);
}

#[test]
fn one_circle_repairs_to_origin_at_its_radius() {
    let instance = Instance::new(vec![4.0]).unwrap();
    let solution = repair(&instance, &[Point::new(100.0, -20.0)], 1.0).unwrap();
    assert_eq!(solution.radius, 4.0);
    assert_eq!(solution.circles, vec![Point::new(0.0, 0.0)]);
}

#[test]
fn coincident_centers_have_zero_scale_and_cannot_be_repaired() {
    let instance = Instance::new(vec![1.0, 2.0]).unwrap();
    let points = [Point::default(), Point::default()];
    assert_eq!(min_scale(&instance, &points).unwrap(), 0.0);
    assert!(matches!(
        repair(&instance, &points, 1.0),
        Err(NumericalError::InvalidInput(_))
    ));
}

#[test]
fn rejects_invalid_counts_factors_and_nonfinite_points() {
    let instance = Instance::new(vec![1.0, 1.0]).unwrap();
    assert!(min_scale(&instance, &[Point::default()]).is_err());
    assert!(repair(&instance, &[Point::default(), Point::new(3.0, 0.0)], 0.9).is_err());
    assert!(repair(
        &instance,
        &[Point::default(), Point::new(f64::NAN, 0.0)],
        1.0
    )
    .is_err());
}
