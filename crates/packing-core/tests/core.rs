use packing_core::{
    format_solution, parse_instance, parse_solution, validate, Instance, PackingError, Point,
    Solution, SolverConfig,
};

#[test]
fn parses_and_formats_fixture_in_original_order() {
    let input = include_str!("../../../tests/fixtures/two_unit_circles.in");
    let output = include_str!("../../../tests/fixtures/two_unit_circles.out");
    let instance = parse_instance(input).unwrap();
    let parsed = parse_solution(&instance, output).unwrap();
    assert_eq!(instance.radii(), &[1.0, 1.0]);
    assert!(validate(&instance, &parsed).unwrap().is_feasible);
    assert_eq!(format_solution(&instance, &parsed).unwrap(), output);
}

#[test]
fn rejects_empty_nonpositive_and_nonfinite_instance_radii() {
    assert!(matches!(
        Instance::new(vec![]),
        Err(PackingError::EmptyInstance)
    ));
    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            Instance::new(vec![radius]),
            Err(PackingError::InvalidRadius { .. })
        ));
    }
    assert!(parse_instance("2 1").is_err());
}

#[test]
fn validates_tangency_and_reports_overlap() {
    let instance = Instance::new(vec![1.0, 2.0]).unwrap();
    let tangent = Solution {
        radius: 3.0,
        circles: vec![Point::new(-2.0, 0.0), Point::new(1.0, 0.0)],
    };
    let report = validate(&instance, &tangent).unwrap();
    assert!(report.is_feasible);
    assert_eq!(report.max_overlap, 0.0);
    let overlapping = Solution {
        radius: 3.0,
        circles: vec![Point::new(-1.9, 0.0), Point::new(0.9, 0.0)],
    };
    let report = validate(&instance, &overlapping).unwrap();
    assert!(!report.is_feasible);
    assert!((report.max_overlap - 0.2).abs() < 1e-12);
}

#[test]
fn detects_containment_violation_and_count_mismatch() {
    let instance = Instance::new(vec![1.0]).unwrap();
    let outside = Solution {
        radius: 1.5,
        circles: vec![Point::new(1.0, 0.0)],
    };
    let report = validate(&instance, &outside).unwrap();
    assert!(!report.is_feasible);
    assert!((report.max_containment_violation - 0.5).abs() < 1e-12);
    let wrong_count = Solution {
        radius: 1.0,
        circles: vec![],
    };
    assert!(matches!(
        validate(&instance, &wrong_count),
        Err(PackingError::CircleCount { .. })
    ));
}

#[test]
fn recomputes_radius_about_origin_after_translation() {
    let instance = Instance::new(vec![1.0, 2.0]).unwrap();
    let solution = Solution {
        radius: 3.0,
        circles: vec![Point::new(-2.0, 0.0), Point::new(1.0, 0.0)],
    };
    assert_eq!(solution.radius_about_origin(&instance).unwrap(), 3.0);
    let translated = Solution {
        radius: 5.0,
        circles: vec![Point::new(0.0, 0.0), Point::new(3.0, 0.0)],
    };
    assert_eq!(translated.radius_about_origin(&instance).unwrap(), 5.0);
}

#[test]
fn scales_one_and_two_circle_optima_linearly() {
    for scale in [0.25, 3.0, 100.0] {
        let one = Instance::new(vec![scale]).unwrap();
        let single = Solution {
            radius: scale,
            circles: vec![Point::new(0.0, 0.0)],
        };
        assert!(validate(&one, &single).unwrap().is_feasible);
        assert_eq!(single.radius_about_origin(&one).unwrap(), scale);
        let two = Instance::new(vec![scale, 2.0 * scale]).unwrap();
        let pair = Solution {
            radius: 3.0 * scale,
            circles: vec![Point::new(-2.0 * scale, 0.0), Point::new(scale, 0.0)],
        };
        assert!(validate(&two, &pair).unwrap().is_feasible);
        assert_eq!(pair.radius_about_origin(&two).unwrap(), 3.0 * scale);
    }
}

#[test]
fn parses_rejects_nonfinite_or_invalid_solution_values() {
    let instance = Instance::new(vec![1.0]).unwrap();
    assert!(matches!(
        parse_solution(&instance, "1 1 NaN 0"),
        Err(PackingError::NonFiniteCoordinate { .. })
    ));
    assert!(matches!(
        parse_solution(&instance, "inf 1 0 0"),
        Err(PackingError::InvalidSolutionRadius(_))
    ));
    let negative = Solution {
        radius: -1.0,
        circles: vec![Point::new(0.0, 0.0)],
    };
    assert!(matches!(
        validate(&instance, &negative),
        Err(PackingError::InvalidSolutionRadius(_))
    ));
    let zero = Solution {
        radius: 0.0,
        circles: vec![Point::new(0.0, 0.0)],
    };
    assert!(matches!(
        validate(&instance, &zero),
        Err(PackingError::InvalidSolutionRadius(_))
    ));
    let nonfinite = Solution {
        radius: 1.0,
        circles: vec![Point::new(f64::INFINITY, 0.0)],
    };
    assert!(matches!(
        validate(&instance, &nonfinite),
        Err(PackingError::NonFiniteCoordinate { .. })
    ));
}

#[test]
fn solution_parser_rejects_missing_extra_and_reordered_radii() {
    let instance = Instance::new(vec![1.0, 2.0]).unwrap();
    assert!(parse_solution(&instance, "3 1 -2 0").is_err());
    assert!(parse_solution(&instance, "3 1 -2 0 2 1 0 9").is_err());
    assert!(parse_solution(&instance, "3 2 -2 0 1 1 0").is_err());
    assert!(parse_solution(&instance, "3 NaN -2 0 2 1 0").is_err());
}

#[test]
fn parser_rejects_unrepresentable_count_without_overflowing_length_check() {
    assert!(parse_instance("18446744073709551615").is_err());
}

#[test]
fn rejects_arithmetic_overflow_from_finite_geometry() {
    let huge_pair = Instance::new(vec![f64::MAX, f64::MAX]).unwrap();
    let separated = Solution {
        radius: f64::MAX,
        circles: vec![Point::new(0.0, 0.0), Point::new(0.0, 0.0)],
    };
    assert!(matches!(
        validate(&huge_pair, &separated),
        Err(PackingError::GeometryOverflow)
    ));

    let one = Instance::new(vec![1.0]).unwrap();
    let overflowing_center = Solution {
        radius: f64::MAX,
        circles: vec![Point::new(f64::MAX, f64::MAX)],
    };
    assert!(matches!(
        validate(&one, &overflowing_center),
        Err(PackingError::GeometryOverflow)
    ));
}

#[test]
fn rejects_output_that_becomes_infeasible_after_rounding() {
    let instance = Instance::new(vec![1.0, 1.0]).unwrap();
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    let barely_separated = Solution {
        radius: 2.0,
        circles: vec![
            Point::new(-diagonal, -diagonal),
            Point::new(diagonal, diagonal),
        ],
    };
    assert!(matches!(
        format_solution(&instance, &barely_separated),
        Err(PackingError::RoundedOutputInfeasible(_))
    ));
}

#[test]
fn computes_area_bound_and_validates_configuration() {
    let instance = Instance::new(vec![3.0, 4.0]).unwrap();
    assert!((instance.area_lower_bound() - 5.0).abs() < 1e-12);
    assert!(SolverConfig::new(17, 0.25, 1_000).is_ok());
    assert!(SolverConfig::new(17, f64::NAN, 1_000).is_err());
    assert!(SolverConfig::new(17, -1.0, 1_000).is_err());
}
