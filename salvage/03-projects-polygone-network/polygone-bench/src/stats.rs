//! Small statistics helpers — deliberately dependency-free and easy to audit.

/// Arithmetic mean.
pub fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Sample standard deviation (Bessel-corrected).
pub fn stddev(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    let variance = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    variance.sqrt()
}

/// Nearest-rank percentile. `xs` need not be sorted.
pub fn percentile(xs: &[f64], p: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rank = (p / 100.0 * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

/// Pearson chi-square statistic of `counts` against a uniform distribution.
pub fn chi_square_uniform(counts: &[u64]) -> f64 {
    let total: u64 = counts.iter().sum();
    if total == 0 || counts.is_empty() {
        return 0.0;
    }
    let expected = total as f64 / counts.len() as f64;
    counts
        .iter()
        .map(|&observed| {
            let delta = observed as f64 - expected;
            delta * delta / expected
        })
        .sum()
}

/// Approximate upper-tail p-value for a chi-square statistic with `df` degrees
/// of freedom, via the Wilson–Hilferty normal approximation. Good enough for a
/// sanity check on entropy, not for publishing.
pub fn chi_square_p_value(chi: f64, df: f64) -> f64 {
    if df <= 0.0 {
        return 1.0;
    }
    let z = ((chi / df).powf(1.0 / 3.0) - (1.0 - 2.0 / (9.0 * df))) / (2.0 / (9.0 * df)).sqrt();
    // 1 - Phi(z), using the complementary error function.
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

/// Complementary error function, Numerical Recipes' `erfcc` approximation.
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let ans = t
        * (-z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x < 0.0 {
        2.0 - ans
    } else {
        ans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_and_stddev() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(mean(&xs), 2.5);
        assert!((stddev(&xs) - 1.290_994_4).abs() < 1e-4);
    }

    #[test]
    fn percentiles() {
        let xs: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        assert_eq!(percentile(&xs, 0.0), 1.0);
        assert_eq!(percentile(&xs, 50.0), 51.0);
        assert_eq!(percentile(&xs, 100.0), 100.0);
    }

    #[test]
    fn chi_square_of_exact_uniform_is_zero() {
        let counts = vec![10u64; 16];
        assert_eq!(chi_square_uniform(&counts), 0.0);
    }

    #[test]
    fn chi_square_detects_bias() {
        let mut counts = vec![100u64; 16];
        counts[0] = 1000;
        assert!(chi_square_uniform(&counts) > 100.0);
    }

    #[test]
    fn p_value_is_in_range() {
        assert!((0.0..=1.0).contains(&chi_square_p_value(255.0, 255.0)));
        assert!(chi_square_p_value(1000.0, 255.0) < 0.01);
    }
}
