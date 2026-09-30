use otelo_storage::{
    Buckets, Distribution, ExplicitBuckets, ExponentialBuckets, Histogram, IndexedCounts,
    Percentiles, StepHistograms, Temporality,
};

fn point(counts: &[u64], sum: f64) -> Histogram {
    point_with_bounds(&[10.0, 100.0], counts, sum)
}

fn point_with_bounds(bounds: &[f64], counts: &[u64], sum: f64) -> Histogram {
    Histogram {
        count: counts.iter().sum(),
        sum: Some(sum),
        min: None,
        max: None,
        buckets: Buckets::Explicit(ExplicitBuckets::new(bounds.to_vec(), counts.to_vec()).unwrap()),
    }
}

fn checked_exponential_buckets(
    scale: i32,
    zero_count: u64,
    positive: (i32, &[u64]),
    negative: (i32, &[u64]),
) -> anyhow::Result<ExponentialBuckets> {
    let side = |(offset, counts): (i32, &[u64])| IndexedCounts {
        offset,
        counts: counts.to_vec(),
    };
    ExponentialBuckets::new(scale, zero_count, side(positive), side(negative))
}

fn exponential(
    scale: i32,
    zero_count: u64,
    positive: (i32, &[u64]),
    negative: (i32, &[u64]),
) -> Histogram {
    Histogram {
        count: zero_count + positive.1.iter().sum::<u64>() + negative.1.iter().sum::<u64>(),
        sum: Some(0.0),
        min: None,
        max: None,
        buckets: Buckets::Exponential(
            checked_exponential_buckets(scale, zero_count, positive, negative).unwrap(),
        ),
    }
}

fn merge_points_by_step(
    temporality: Temporality,
    points: Vec<(i64, Histogram)>,
) -> Vec<(i64, Distribution)> {
    let mut histograms = StepHistograms::new(temporality);
    for (step_start_at, point) in points {
        histograms.add_point(step_start_at, point);
    }
    histograms
        .into_histograms_by_step()
        .into_iter()
        .map(|(step_start_at, merged)| (step_start_at, Distribution::from(merged)))
        .collect()
}

#[test]
fn checks_the_counts_against_the_bounds() {
    let checked_buckets =
        |bounds: &[f64], counts: &[u64]| ExplicitBuckets::new(bounds.to_vec(), counts.to_vec());
    assert!(checked_buckets(&[10.0, 100.0], &[1, 2, 3]).is_ok());
    assert!(checked_buckets(&[10.0, 100.0], &[1, 2]).is_err());
    assert!(checked_buckets(&[100.0, 10.0], &[1, 2, 3]).is_err());
}

#[test]
fn checks_the_scale_of_an_exponential_histogram() {
    assert!(checked_exponential_buckets(20, 0, (0, &[1]), (0, &[])).is_ok());
    assert!(checked_exponential_buckets(21, 0, (0, &[1]), (0, &[])).is_err());
    assert!(checked_exponential_buckets(-11, 0, (0, &[1]), (0, &[])).is_err());
}

#[test]
fn adds_the_deltas_of_a_step() {
    let steps = merge_points_by_step(
        Temporality::Delta,
        vec![
            (0, point(&[1, 0, 0], 5.0)),
            (0, point(&[1, 2, 0], 70.0)),
            (60, point(&[0, 0, 1], 500.0)),
        ],
    );
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].1.counts, [2, 2, 0]);
    assert_eq!(steps[0].1.count, 4);
    assert_eq!(steps[0].1.sum, Some(75.0));
    assert_eq!(steps[1].1.counts, [0, 0, 1]);
}

#[test]
fn counts_the_increase_of_cumulative_points_and_a_restart() {
    let steps = merge_points_by_step(
        Temporality::Cumulative,
        vec![
            // The first point sets the start.
            (0, point(&[5, 5, 0], 100.0)),
            (0, point(&[6, 7, 0], 130.0)),
            (60, point(&[6, 9, 1], 400.0)),
            // The app restarted: the counts fell.
            (60, point(&[1, 0, 0], 2.0)),
        ],
    );
    assert_eq!(steps[0].1.counts, [1, 2, 0]);
    assert_eq!(steps[0].1.sum, Some(30.0));
    assert_eq!(steps[1].1.counts, [1, 2, 1]);
    assert_eq!(steps[1].1.count, 4);
    assert_eq!(steps[1].1.sum, Some(272.0));
}

#[test]
fn a_step_keeps_the_newest_bounds() {
    let steps = merge_points_by_step(
        Temporality::Delta,
        vec![
            (0, point(&[1, 1, 1], 1.0)),
            (0, point_with_bounds(&[50.0], &[4, 0], 1.0)),
        ],
    );
    assert_eq!(steps[0].1.bounds, [50.0]);
    assert_eq!(steps[0].1.counts, [4, 0]);
}

#[test]
fn estimates_percentiles_inside_the_buckets() {
    let steps = merge_points_by_step(Temporality::Delta, vec![(0, point(&[50, 40, 10], 0.0))]);
    // The last bucket has no upper bound, so its lower bound stands in for p99.
    assert_eq!(
        steps[0].1.percentiles,
        Some(Percentiles {
            p50: 10.0,
            p90: 100.0,
            p99: 100.0
        })
    );
    let empty = Distribution::from(point(&[0, 0, 0], 0.0));
    assert_eq!(empty.percentiles, None);
}

#[test]
fn an_exponential_histogram_gets_the_bounds_of_its_buckets() {
    // At scale 0 bucket `i` holds the values from 2^i to 2^(i + 1).
    let positive = Distribution::from(exponential(0, 0, (0, &[10, 10]), (0, &[])));
    assert_eq!(positive.bounds, [1.0, 2.0, 4.0]);
    assert_eq!(positive.counts, [0, 10, 10, 0]);
    let percentiles = positive.percentiles.unwrap();
    assert_eq!((percentiles.p50, percentiles.p90), (2.0, 3.6));

    let both_sides = Distribution::from(exponential(0, 3, (1, &[5]), (0, &[2])));
    assert_eq!(both_sides.bounds, [-2.0, -1.0, 0.0, 2.0, 4.0]);
    assert_eq!(both_sides.counts, [0, 2, 3, 0, 5, 0]);
    assert_eq!(both_sides.count, 10);
}

#[test]
fn exponential_points_of_different_scales_add_at_the_lower_scale() {
    let steps = merge_points_by_step(
        Temporality::Delta,
        vec![
            // At scale 1 two buckets make one of scale 0.
            (0, exponential(1, 1, (0, &[1, 1, 1, 1]), (-3, &[1, 1, 1]))),
            (0, exponential(0, 0, (1, &[5, 5]), (0, &[]))),
        ],
    );
    let merged = &steps[0].1;
    // Negative: the indexes -3, -2, -1 of scale 1 are -2, -1, -1 of scale 0.
    assert_eq!(merged.bounds, [-1.0, -0.5, -0.25, 0.0, 1.0, 2.0, 4.0, 8.0]);
    assert_eq!(merged.counts, [0, 2, 1, 1, 0, 2, 7, 5, 0]);
    assert_eq!(merged.count, 18);
}

#[test]
fn a_cumulative_exponential_histogram_counts_its_increase_across_a_change_of_scale() {
    let steps = merge_points_by_step(
        Temporality::Cumulative,
        vec![
            (0, exponential(1, 0, (0, &[1, 1, 1, 1]), (0, &[]))),
            // The sender halved its scale once the values spread.
            (60, exponential(0, 0, (0, &[3, 4, 2]), (0, &[]))),
            // It restarted: a count fell.
            (120, exponential(0, 0, (0, &[1]), (0, &[]))),
        ],
    );
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].0, 60);
    assert_eq!(steps[0].1.counts, [0, 1, 2, 2, 0]);
    assert_eq!(steps[0].1.count, 5);
    assert_eq!(steps[1].1.counts, [0, 1, 0]);
}

#[test]
fn an_explicit_and_an_exponential_point_do_not_merge() {
    let steps = merge_points_by_step(
        Temporality::Delta,
        vec![
            (0, point(&[1, 1, 1], 1.0)),
            (0, exponential(0, 0, (0, &[4]), (0, &[]))),
        ],
    );
    assert_eq!(steps[0].1.bounds, [1.0, 2.0]);
    assert_eq!(steps[0].1.count, 4);
}

#[test]
fn a_distribution_shows_at_most_64_buckets_of_an_exponential_histogram() {
    let counts = vec![1; 200];
    let wide = Distribution::from(exponential(3, 0, (0, &counts), (0, &[])));
    assert_eq!(wide.count, 200);
    assert_eq!(wide.counts.iter().sum::<u64>(), 200);
    // The first bound is where the first bucket starts, and the last count is above all bounds.
    assert!(wide.bounds.len() <= 65, "{} bounds", wide.bounds.len());
    // The estimate reads the buckets before they are joined: the median sits in bucket 99
    // of scale 3, which ends at 2^(100 / 8).
    let median = wide.percentiles.unwrap().p50;
    assert!((median - 12.5_f64.exp2()).abs() < 1e-6, "{median}");
}

#[test]
fn a_stored_histogram_reads_back_in_its_shape() {
    for histogram in [
        point(&[1, 2, 3], 9.0),
        exponential(2, 1, (-4, &[1, 2]), (3, &[4])),
    ] {
        let json = serde_json::to_string(&histogram).unwrap();
        assert_eq!(serde_json::from_str::<Histogram>(&json).unwrap(), histogram);
    }
    assert_eq!(
        serde_json::to_string(&point(&[1, 2, 3], 9.0)).unwrap(),
        r#"{"count":6,"sum":9.0,"min":null,"max":null,"bounds":[10.0,100.0],"counts":[1,2,3]}"#
    );
}

#[test]
fn a_stored_histogram_is_checked_when_it_is_read() {
    let stored = serde_json::to_string(&point(&[1, 2, 3], 1.0)).unwrap();
    let with_a_count_missing = stored.replace("[1,2,3]", "[1,2]");
    assert!(serde_json::from_str::<Histogram>(&with_a_count_missing).is_err());

    let stored = serde_json::to_string(&exponential(20, 0, (0, &[1]), (0, &[]))).unwrap();
    let with_a_scale_too_high = stored.replace(r#""scale":20"#, r#""scale":21"#);
    assert!(serde_json::from_str::<Histogram>(&with_a_scale_too_high).is_err());
}
