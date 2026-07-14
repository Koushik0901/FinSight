//! Robust statistics (median / MAD) — the same principle anomaly.rs uses,
//! reused so "your normal" resists a few hot months poisoning it.

/// Median of a slice. Returns 0.0 for an empty slice.
pub fn median(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    }
}

/// Median absolute deviation about `med`. Returns 0.0 for an empty slice.
pub fn mad(xs: &[f64], med: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let devs: Vec<f64> = xs.iter().map(|x| (x - med).abs()).collect();
    median(&devs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_ignores_outliers_that_mean_would_chase() {
        let mut months = vec![2000.0; 11];
        months.push(9000.0);
        assert_eq!(median(&months), 2000.0, "median stays at the true normal");
        let mean = months.iter().sum::<f64>() / months.len() as f64;
        assert!(mean > 2500.0, "the mean is dragged up by the spike");
    }

    #[test]
    fn mad_measures_spread_about_the_median() {
        assert_eq!(median(&[]), 0.0);
        assert_eq!(mad(&[5.0], 5.0), 0.0);
        assert_eq!(mad(&[1.0, 2.0, 3.0, 4.0, 5.0], 3.0), 1.0);
    }
}
