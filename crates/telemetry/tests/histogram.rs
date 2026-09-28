use siner_telemetry::{Histogram, Merger};

fn point(counts: &[u64], sum: f64, cumulative: bool) -> Histogram {
    Histogram {
        bounds: vec![10.0, 100.0],
        counts: counts.to_vec(),
        count: counts.iter().sum(),
        sum: Some(sum),
        min: None,
        max: None,
        cumulative,
    }
}

#[test]
fn checks_the_counts_against_the_bounds() {
    assert!(point(&[1, 2, 3], 1.0, false).check().is_ok());
    assert!(point(&[1, 2], 1.0, false).check().is_err());
    let mut descending = point(&[1, 2, 3], 1.0, false);
    descending.bounds = vec![100.0, 10.0];
    assert!(descending.check().is_err());
}

#[test]
fn adds_the_deltas_of_a_step() {
    let mut merger = Merger::default();
    merger.push(0, point(&[1, 0, 0], 5.0, false));
    merger.push(0, point(&[1, 2, 0], 70.0, false));
    merger.push(60, point(&[0, 0, 1], 500.0, false));
    let steps = merger.finish();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].1.counts, [2, 2, 0]);
    assert_eq!(steps[0].1.count, 4);
    assert_eq!(steps[0].1.sum, Some(75.0));
    assert_eq!(steps[1].1.counts, [0, 0, 1]);
}

#[test]
fn counts_the_increase_of_cumulative_points_and_a_restart() {
    let mut merger = Merger::default();
    // The first point sets the start.
    merger.push(0, point(&[5, 5, 0], 100.0, true));
    merger.push(0, point(&[6, 7, 0], 130.0, true));
    merger.push(60, point(&[6, 9, 1], 400.0, true));
    // The app restarted: the counts fell.
    merger.push(60, point(&[1, 0, 0], 2.0, true));
    let steps = merger.finish();
    assert_eq!(steps[0].1.counts, [1, 2, 0]);
    assert_eq!(steps[0].1.sum, Some(30.0));
    assert_eq!(steps[1].1.counts, [1, 2, 1]);
    assert_eq!(steps[1].1.count, 4);
    assert_eq!(steps[1].1.sum, Some(272.0));
}

#[test]
fn a_step_keeps_the_newest_bounds() {
    let mut merger = Merger::default();
    merger.push(0, point(&[1, 1, 1], 1.0, false));
    let mut other = point(&[4, 0], 1.0, false);
    other.bounds = vec![50.0];
    merger.push(0, other);
    let steps = merger.finish();
    assert_eq!(steps[0].1.bounds, [50.0]);
    assert_eq!(steps[0].1.counts, [4, 0]);
}

#[test]
fn estimates_percentiles_inside_the_buckets() {
    let mut merger = Merger::default();
    merger.push(0, point(&[50, 40, 10], 0.0, false));
    let steps = merger.finish();
    let d = &steps[0].1;
    // Half the values are in (0, 10].
    assert_eq!(d.p50, Some(10.0));
    // 90 of 100 fall at or below 100.
    assert_eq!(d.p90, Some(100.0));
    // The last bucket has no upper bound, so its lower bound stands in.
    assert_eq!(d.p99, Some(100.0));
    let mut empty = Merger::default();
    empty.push(0, point(&[0, 0, 0], 0.0, false));
    assert_eq!(empty.finish()[0].1.p50, None);
}
