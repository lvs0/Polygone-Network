//! Benchmarking infrastructure for Petals inference engine

use crate::types::{BackendType, DeviceType, InferenceRequest};
use crate::backends::InferenceBackend;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Configuration for a benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Model to benchmark
    pub model: String,
    /// Number of warmup runs
    pub warmup_runs: usize,
    /// Number of benchmark runs
    pub benchmark_runs: usize,
    /// Prompt to use (or generate random)
    pub prompt: Option<String>,
    /// Generation config
    pub generation_config: crate::types::GenerationConfig,
    /// Concurrent requests
    pub concurrency: usize,
    /// Measure memory usage
    pub measure_memory: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            model: "".to_string(),
            warmup_runs: 3,
            benchmark_runs: 10,
            prompt: None,
            generation_config: crate::types::GenerationConfig::default(),
            concurrency: 1,
            measure_memory: false,
        }
    }
}

/// Result of a benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Model name
    pub model: String,
    /// Backend used
    pub backend: BackendType,
    /// Device used
    pub device: DeviceType,
    /// Configuration used
    pub config: BenchmarkConfig,
    /// Individual run results
    pub runs: Vec<RunResult>,
    /// Aggregated statistics
    pub stats: BenchmarkStats,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Result of a single run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    /// Run index
    pub run: usize,
    /// Time to first token (ms)
    pub ttft_ms: Option<u64>,
    /// Total generation time (ms)
    pub total_time_ms: u64,
    /// Tokens generated
    pub tokens_generated: u32,
    /// Tokens per second
    pub tokens_per_second: f32,
    /// Memory usage (MB) if measured
    pub memory_mb: Option<f64>,
    /// Success
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Aggregated benchmark statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStats {
    /// Mean tokens/second
    pub mean_tokens_per_sec: f32,
    /// Median tokens/second
    pub median_tokens_per_sec: f32,
    /// Min tokens/second
    pub min_tokens_per_sec: f32,
    /// Max tokens/second
    pub max_tokens_per_sec: f32,
    /// Standard deviation
    pub std_tokens_per_sec: f32,
    /// Mean TTFT (ms)
    pub mean_ttft_ms: Option<f64>,
    /// Mean total time (ms)
    pub mean_total_time_ms: f64,
    /// Success rate
    pub success_rate: f32,
    /// Total tokens generated
    pub total_tokens: u64,
    /// Total time (ms)
    pub total_time_ms: u64,
}

/// Benchmark runner
pub struct InferenceBench {
    backend: Arc<InferenceBackend>,
    config: BenchmarkConfig,
}

impl InferenceBench {
    /// Create a new benchmark runner
    pub fn new(backend: Arc<InferenceBackend>, config: BenchmarkConfig) -> Self {
        Self { backend, config }
    }

    /// Run the benchmark
    pub async fn run(&self) -> Result<BenchmarkResult> {
        let mut runs = Vec::new();
        let prompt = self.config.prompt.clone().unwrap_or_else(|| {
            "Explain the ML-KEM-1024 key encapsulation mechanism in detail.".to_string()
        });

        // Warmup runs
        for i in 0..self.config.warmup_runs {
            let request = InferenceRequest::new(&prompt)
                .with_model(&self.config.model)
                .with_config(self.config.generation_config.clone());
            let _ = self.backend.generate(request).await;
            log::info!("Warmup run {} completed", i + 1);
        }

        // Benchmark runs
        for i in 0..self.config.benchmark_runs {
            let request = InferenceRequest::new(&prompt)
                .with_model(&self.config.model)
                .with_config(self.config.generation_config.clone());

            let start = Instant::now();
            let result = self.backend.generate(request).await;
            let elapsed = start.elapsed();

            let run_result = match result {
                Ok(response) => RunResult {
                    run: i,
                    ttft_ms: response.ttft_ms,
                    total_time_ms: response.total_time_ms,
                    tokens_generated: response.tokens_generated,
                    tokens_per_second: response.tokens_per_second,
                    memory_mb: None, // TODO: implement memory measurement
                    success: true,
                    error: None,
                },
                Err(e) => RunResult {
                    run: i,
                    ttft_ms: None,
                    total_time_ms: elapsed.as_millis() as u64,
                    tokens_generated: 0,
                    tokens_per_second: 0.0,
                    memory_mb: None,
                    success: false,
                    error: Some(e.to_string()),
                },
            };

            runs.push(run_result);
            log::info!("Benchmark run {} completed: {:.2} tok/s", i + 1, runs.last().unwrap().tokens_per_second);
        }

        let stats = self.compute_stats(&runs);
        let backend_type = self.backend.backend_type();

        Ok(BenchmarkResult {
            model: self.config.model.clone(),
            backend: backend_type,
            device: DeviceType::Cpu, // TODO: detect actual device
            config: self.config.clone(),
            runs,
            stats,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Compute aggregated statistics
    fn compute_stats(&self, runs: &[RunResult]) -> BenchmarkStats {
        let successful: Vec<&RunResult> = runs.iter().filter(|r| r.success).collect();
        let success_rate = successful.len() as f32 / runs.len().max(1) as f32;

        if successful.is_empty() {
            return BenchmarkStats {
                mean_tokens_per_sec: 0.0,
                median_tokens_per_sec: 0.0,
                min_tokens_per_sec: 0.0,
                max_tokens_per_sec: 0.0,
                std_tokens_per_sec: 0.0,
                mean_ttft_ms: None,
                mean_total_time_ms: 0.0,
                success_rate,
                total_tokens: 0,
                total_time_ms: 0,
            };
        }

        let tokens_per_sec: Vec<f32> = successful.iter().map(|r| r.tokens_per_second).collect();
        let total_times: Vec<u64> = successful.iter().map(|r| r.total_time_ms).collect();
        let ttfts: Vec<u64> = successful.iter().filter_map(|r| r.ttft_ms).collect();

        let mean_tokens = tokens_per_sec.iter().sum::<f32>() / tokens_per_sec.len() as f32;
        let mut sorted = tokens_per_sec.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[sorted.len() / 2];
        let min = *sorted.first().unwrap_or(&0.0);
        let max = *sorted.last().unwrap_or(&0.0);
        let variance = tokens_per_sec.iter().map(|x| (x - mean_tokens).powi(2)).sum::<f32>() / tokens_per_sec.len() as f32;
        let std = variance.sqrt();

        let mean_ttft = if !ttfts.is_empty() {
            Some(ttfts.iter().sum::<u64>() as f64 / ttfts.len() as f64)
        } else {
            None
        };

        let mean_total = total_times.iter().sum::<u64>() as f64 / total_times.len() as f64;
        let total_tokens: u64 = successful.iter().map(|r| r.tokens_generated as u64).sum();
        let total_time: u64 = total_times.iter().sum();

        BenchmarkStats {
            mean_tokens_per_sec: mean_tokens,
            median_tokens_per_sec: median,
            min_tokens_per_sec: min,
            max_tokens_per_sec: max,
            std_tokens_per_sec: std,
            mean_ttft_ms: mean_ttft,
            mean_total_time_ms: mean_total,
            success_rate,
            total_tokens,
            total_time_ms: total_time,
        }
    }
}

/// Quick benchmark for CLI use
pub async fn quick_benchmark(
    backend: Arc<InferenceBackend>,
    model: &str,
    prompt: &str,
    runs: usize,
) -> Result<BenchmarkResult> {
    let config = BenchmarkConfig {
        model: model.to_string(),
        warmup_runs: 1,
        benchmark_runs: runs,
        prompt: Some(prompt.to_string()),
        generation_config: crate::types::GenerationConfig::default(),
        concurrency: 1,
        measure_memory: false,
    };

    let bench = InferenceBench::new(backend, config);
    bench.run().await
}