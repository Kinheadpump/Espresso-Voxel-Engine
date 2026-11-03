use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summary {
    pub count: usize,
    pub min: f64,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
    pub mean: f64,
    pub stddev: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metric {
    pub unit: String,
    pub per_repeat_p50: Vec<f64>,
    pub pooled: Summary,
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let rank = fraction * (sorted.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    sorted[lower] + (sorted[upper] - sorted[lower]) * (rank - lower as f64)
}

impl Summary {
    pub fn of(samples: &[f64]) -> Self {
        assert!(!samples.is_empty(), "summary needs at least one sample");
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
        let variance = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / sorted.len() as f64;
        Self {
            count: sorted.len(),
            min: sorted[0],
            p50: percentile(&sorted, 0.5),
            p90: percentile(&sorted, 0.9),
            p99: percentile(&sorted, 0.99),
            max: sorted[sorted.len() - 1],
            mean,
            stddev: variance.sqrt(),
        }
    }
}

impl Metric {
    pub fn from_repeats(unit: &str, repeats: &[Vec<f64>]) -> Self {
        let pooled: Vec<f64> = repeats.iter().flatten().copied().collect();
        Self {
            unit: unit.to_owned(),
            per_repeat_p50: repeats
                .iter()
                .map(|samples| Summary::of(samples).p50)
                .collect(),
            pooled: Summary::of(&pooled),
        }
    }

    pub fn from_single_samples(unit: &str, samples: &[f64]) -> Self {
        let repeats: Vec<Vec<f64>> = samples.iter().map(|&sample| vec![sample]).collect();
        Self::from_repeats(unit, &repeats)
    }

    pub fn center(&self) -> f64 {
        Summary::of(&self.per_repeat_p50).p50
    }

    pub fn relative_spread(&self) -> f64 {
        let summary = Summary::of(&self.per_repeat_p50);
        if summary.p50 == 0.0 {
            0.0
        } else {
            (summary.max - summary.min) / summary.p50
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_interpolate_between_samples() {
        let summary = Summary::of(&[4.0, 1.0, 3.0, 2.0, 5.0]);
        assert_eq!((summary.min, summary.p50, summary.max), (1.0, 3.0, 5.0));
        assert!((summary.p90 - 4.6).abs() < 1e-9);
        assert!((summary.mean - 3.0).abs() < 1e-9);
    }

    #[test]
    fn metric_center_is_median_of_repeat_medians() {
        let metric = Metric::from_repeats("ms", &[vec![1.0, 2.0, 3.0], vec![10.0], vec![4.0, 4.0]]);
        assert_eq!(metric.per_repeat_p50, vec![2.0, 10.0, 4.0]);
        assert!((metric.center() - 4.0).abs() < 1e-9);
        assert!((metric.relative_spread() - 2.0).abs() < 1e-9);
    }
}
