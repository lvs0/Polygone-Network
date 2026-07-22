//! Median filter for robust time offset estimation
//!
//! Uses sliding window median with configurable window size.
//! Resistant to outliers (Byzantine peers, network spikes).

use std::collections::VecDeque;

/// Configuration for median filter
#[derive(Debug, Clone, Copy)]
pub struct MedianFilterConfig {
    /// Window size (must be odd for clean median)
    pub window_size: usize,
    /// Minimum samples before producing output
    pub min_samples: usize,
}

impl Default for MedianFilterConfig {
    fn default() -> Self {
        Self {
            window_size: 7,  // Odd number for clean median
            min_samples: 3,
        }
    }
}

/// Sliding window median filter
#[derive(Debug, Clone)]
pub struct MedianFilter {
    config: MedianFilterConfig,
    samples: VecDeque<i64>,
}

impl MedianFilter {
    pub fn new(config: MedianFilterConfig) -> Self {
        let window_size = if config.window_size % 2 == 0 {
            config.window_size + 1
        } else {
            config.window_size
        };
        Self {
            config: MedianFilterConfig { window_size, ..config },
            samples: VecDeque::with_capacity(window_size),
        }
    }

    /// Add a new sample and return current median if enough samples
    pub fn add(&mut self, value: i64) -> Option<i64> {
        self.samples.push_back(value);
        if self.samples.len() > self.config.window_size {
            self.samples.pop_front();
        }
        self.median()
    }

    /// Get current median without adding sample
        pub fn median(&self) -> Option<i64> {
            if self.samples.len() < self.config.min_samples {
                return None;
            }
            let mut sorted: Vec<i64> = self.samples.iter().copied().collect();
            sorted.sort_unstable();
            let len = sorted.len();
            if len % 2 == 0 {
                // Even number of elements: return the average of the two middle values
                let mid = len / 2;
                Some((sorted[mid - 1] + sorted[mid]) / 2)
            } else {
                // Odd number of elements: return the middle value
                let mid = len / 2;
                Some(sorted[mid])
            }
        }

    /// Get current median and confidence based on sample consistency
    pub fn median_with_confidence(&self) -> Option<(i64, f64)> {
        let median = self.median()?;
        if self.samples.len() < self.config.min_samples {
            return None;
        }
        // Confidence = 1 - (MAD / median_abs) where MAD is median absolute deviation
        let deviations: Vec<i64> = self.samples
            .iter()
            .map(|&x| (x - median).abs())
            .collect();
        let mut sorted_dev = deviations;
        sorted_dev.sort_unstable();
        let mad = sorted_dev[sorted_dev.len() / 2] as f64;
        let median_abs = median.abs() as f64 + 1.0; // avoid div by zero
        let confidence = (1.0 - (mad / median_abs)).clamp(0.0, 1.0);
        Some((median, confidence))
    }

    /// Number of samples in window
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if filter has minimum samples
    pub fn is_ready(&self) -> bool {
        self.samples.len() >= self.config.min_samples
    }

    /// Clear all samples
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// Weighted median filter for peer offset aggregation
/// Weights by inverse RTT and sample count
#[derive(Debug, Clone)]
pub struct WeightedMedianFilter {
    samples: Vec<WeightedSample>,
    max_samples: usize,
}

#[derive(Debug, Clone, Copy)]
struct WeightedSample {
    value: i64,
    weight: f64,
}

impl WeightedMedianFilter {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add(&mut self, value: i64, weight: f64) {
        self.samples.push(WeightedSample { value, weight });
        if self.samples.len() > self.max_samples {
            // Remove lowest weight sample
            let min_idx = self.samples
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.weight.partial_cmp(&b.1.weight).unwrap())
                .map(|(i, _)| i);
            if let Some(idx) = min_idx {
                self.samples.swap_remove(idx);
            }
        }
    }

    pub fn weighted_median(&self) -> Option<(i64, f64)> {
        if self.samples.is_empty() {
            return None;
        }
        if self.samples.len() == 1 {
            return Some((self.samples[0].value, 1.0));
        }

        // Sort by value
        let mut sorted = self.samples.clone();
        sorted.sort_by_key(|s| s.value);

        let total_weight: f64 = sorted.iter().map(|s| s.weight).sum();
        let half_weight = total_weight / 2.0;

        let mut cum_weight = 0.0;
        for (_i, sample) in sorted.iter().enumerate() {
            cum_weight += sample.weight;
            if cum_weight >= half_weight {
                // Confidence based on weight concentration around median
                let median_val = sample.value;
                let nearby_weight: f64 = sorted.iter()
                    .filter(|s| (s.value - median_val).abs() <= 50) // within 50ms
                    .map(|s| s.weight)
                    .sum();
                let confidence = (nearby_weight / total_weight).clamp(0.0, 1.0);
                return Some((median_val, confidence));
            }
        }

        // Fallback
        let mid = sorted.len() / 2;
        Some((sorted[mid].value, 0.5))
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_median_filter_basic() {
        let mut filter = MedianFilter::new(MedianFilterConfig::default());
        assert_eq!(filter.add(10), None); // not enough samples
        assert_eq!(filter.add(20), None);
        assert_eq!(filter.add(30), Some(20)); // median of [10,20,30] = 20
        assert_eq!(filter.add(1000), Some(25)); // median of [10,20,30,1000] = (20+30)/2 = 25
        assert_eq!(filter.add(40), Some(30)); // median of [10,20,30,1000,40] sorted=[10,20,30,40,1000] -> 30
    }

    #[test]
    fn test_median_filter_outlier_rejection() {
        let mut filter = MedianFilter::new(MedianFilterConfig { window_size: 7, min_samples: 3 });
        // Normal samples around 100
        for v in [98, 102, 101, 99, 100] {
            filter.add(v);
        }
        let (median, conf) = filter.median_with_confidence().unwrap();
        assert!((median - 100).abs() <= 2);
        assert!(conf > 0.8);

        // Add massive outlier
        filter.add(10000);
        let (median, conf) = filter.median_with_confidence().unwrap();
        assert!((median - 100).abs() <= 2); // median should still be ~100
        assert!(conf > 0.8); // confidence remains high (MAD small)
    }

    #[test]
    fn test_weighted_median() {
        let mut filter = WeightedMedianFilter::new(10);
        // Low RTT peers = high weight
        filter.add(100, 10.0); // RTT 10ms
        filter.add(105, 8.0);  // RTT 12ms
        filter.add(98, 9.0);   // RTT 11ms
        // High RTT peer = low weight
        filter.add(500, 1.0);  // RTT 100ms

        let (median, conf) = filter.weighted_median().unwrap();
        assert!((median - 100).abs() <= 5); // Should be near 100, not 500
        assert!(conf > 0.7);
    }
}