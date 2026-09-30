use otelo_storage::{Increase, NumberPoint, StepIncreases, Temporality};

const SECOND: i64 = 1_000_000_000;

fn increases(
    temporality: Temporality,
    step_seconds: i64,
    points: &[(i64, f64)],
) -> Vec<(i64, Increase)> {
    let mut increases = StepIncreases::new(temporality);
    for &(second, value) in points {
        let recorded_at = second * SECOND;
        increases.add_point(
            recorded_at.div_euclid(step_seconds * SECOND) * step_seconds,
            NumberPoint { recorded_at, value },
        );
    }
    increases.into_increases_by_step().into_iter().collect()
}

const fn increase(amount: f64, elapsed_seconds: f64) -> Increase {
    Increase {
        amount,
        elapsed_seconds,
    }
}

#[test]
fn a_cumulative_counter_counts_from_the_point_before() {
    let steps = increases(
        Temporality::Cumulative,
        60,
        &[(0, 100.0), (15, 130.0), (30, 130.0), (60, 190.0)],
    );
    // The first point only sets the start.
    assert_eq!(
        steps,
        [(0, increase(30.0, 30.0)), (60, increase(60.0, 30.0))]
    );
    assert_eq!(steps[0].1.rate_per_second(), Some(1.0));
    assert_eq!(steps[1].1.rate_per_second(), Some(2.0));
}

#[test]
fn a_cumulative_counter_that_falls_started_again_from_zero() {
    let steps = increases(
        Temporality::Cumulative,
        60,
        &[(0, 100.0), (15, 120.0), (30, 5.0), (45, 9.0)],
    );
    assert_eq!(steps, [(0, increase(29.0, 45.0))]);
}

#[test]
fn a_delta_counter_adds_its_points() {
    let steps = increases(Temporality::Delta, 60, &[(0, 10.0), (30, 20.0), (60, 30.0)]);
    // Nothing says how long the first point counted for.
    assert_eq!(
        steps,
        [(0, increase(30.0, 30.0)), (60, increase(30.0, 30.0))]
    );
    assert_eq!(steps[0].1.rate_per_second(), Some(1.0));
}

#[test]
fn the_rate_divides_by_the_time_between_the_points_not_by_the_step() {
    // Points a minute apart in steps of 30 seconds: every other step has a point.
    let steps = increases(
        Temporality::Cumulative,
        30,
        &[(0, 0.0), (60, 120.0), (120, 180.0)],
    );
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].1.rate_per_second(), Some(2.0));
    assert_eq!(steps[1].1.rate_per_second(), Some(1.0));
}

#[test]
fn a_step_with_only_the_first_point_has_no_rate() {
    assert!(increases(Temporality::Cumulative, 60, &[(0, 100.0)]).is_empty());
    let delta = increases(Temporality::Delta, 60, &[(0, 100.0)]);
    assert_eq!(delta[0].1.rate_per_second(), None);
}
