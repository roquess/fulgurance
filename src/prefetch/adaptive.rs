use crate::PrefetchStrategy;
use super::{BenchmarkablePrefetch, PrefetchType};
use std::collections::{HashMap, VecDeque};

/// Adaptive prefetch strategy
///
/// This strategy dynamically combines multiple prefetch approaches
/// and adapts its behavior at runtime based on observed performance.
/// Key characteristics:
/// - Uses an ensemble of strategies (Sequential, Stride, History-based, Hybrid)
/// - Weights each strategy dynamically according to prediction success
/// - Learns which strategy is most effective for the current access pattern
/// - Classifies access patterns heuristically (sequential, stride, cyclic, random…)
#[derive(Debug, Clone)]
pub struct AdaptivePrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    /// Recent access history for detecting regularities
    access_history: VecDeque<K>,
    /// Tracks performance metrics per strategy
    strategy_performance: HashMap<StrategyType, PerformanceMetrics>,
    /// Current weights assigned to each strategy (sum to 1.0)
    strategy_weights: HashMap<StrategyType, f64>,
    /// Maximum amount of access history to retain
    max_history: usize,
    /// How far ahead to prefetch keys
    prefetch_distance: usize,
    /// Maximum number of predictions per access
    max_predictions: usize,
    /// Learning rate applied when updating weights
    learning_rate: f64,
    /// Minimum confidence required to accept predictions
    min_confidence: f64,
    /// Current classification of detected access pattern
    current_pattern_type: PatternType,
    /// Internal state machine for sequential detection
    sequential_state: SequentialState<K>,
    /// Internal state machine for stride detection
    stride_state: StrideState,
    /// Internal state machine for history-based patterns
    history_state: HistoryState<K>,
    /// Number of processed accesses so far
    total_accesses: usize,
}

/// Set of distinct strategies available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrategyType {
    Sequential,
    Stride,
    HistoryBased,
    Hybrid,
}

/// Heuristic classification of detected access patterns
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternType {
    Sequential,
    Stride,
    Random,
    Cyclic,
    Mixed,
    Unknown,
}

/// Statistics for a single strategy
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total number of predictions issued
    total_predictions: usize,
    /// Number of predictions that matched actual accesses
    successful_predictions: usize,
    /// Sliding window of recent outcomes for smoothing
    recent_successes: VecDeque<bool>,
    /// Confidence score in this strategy
    confidence: f64,
}

/// Tracks internal state of the sequential predictor
#[derive(Debug, Clone)]
struct SequentialState<K> {
    last_key: Option<K>,
    stride: Option<i64>,
    consecutive_hits: usize,
    confidence: f64,
}

/// Tracks internal state of the stride predictor
#[derive(Debug, Clone)]
struct StrideState {
    detected_strides: HashMap<i64, usize>,
    dominant_stride: Option<i64>,
    stride_confidence: f64,
}

/// Tracks internal state of the history-based predictor
#[derive(Debug, Clone)]
struct HistoryState<K> {
    pattern_frequencies: HashMap<Vec<K>, usize>,
    pattern_length: usize,
}

impl<K> AdaptivePrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    /// Creates a new adaptive prefetcher with default tuning parameters
    pub fn new() -> Self {
        Self::with_config(20, 4, 5, 0.1, 0.3)
    }

    /// Creates an adaptive prefetcher with user-defined parameters
    pub fn with_config(
        max_history: usize,
        prefetch_distance: usize,
        max_predictions: usize,
        learning_rate: f64,
        min_confidence: f64,
    ) -> Self {
        let mut strategy_weights = HashMap::new();
        strategy_weights.insert(StrategyType::Sequential, 0.25);
        strategy_weights.insert(StrategyType::Stride, 0.25);
        strategy_weights.insert(StrategyType::HistoryBased, 0.25);
        strategy_weights.insert(StrategyType::Hybrid, 0.25);

        let mut strategy_performance = HashMap::new();
        for &strategy_type in &[
            StrategyType::Sequential,
            StrategyType::Stride,
            StrategyType::HistoryBased,
            StrategyType::Hybrid,
        ] {
            strategy_performance.insert(
                strategy_type,
                PerformanceMetrics {
                    total_predictions: 0,
                    successful_predictions: 0,
                    recent_successes: VecDeque::with_capacity(50),
                    confidence: 0.5,
                },
            );
        }

        Self {
            access_history: VecDeque::with_capacity(max_history),
            strategy_performance,
            strategy_weights,
            max_history,
            prefetch_distance,
            max_predictions,
            learning_rate,
            min_confidence,
            current_pattern_type: PatternType::Unknown,
            sequential_state: SequentialState {
                last_key: None,
                stride: None,
                consecutive_hits: 0,
                confidence: 0.5,
            },
            stride_state: StrideState {
                detected_strides: HashMap::new(),
                dominant_stride: None,
                stride_confidence: 0.0,
            },
            history_state: HistoryState {
                pattern_frequencies: HashMap::new(),
                pattern_length: 3,
            },
            total_accesses: 0,
        }
    }

    /// Returns the current weight distribution across strategies
    pub fn strategy_weights(&self) -> &HashMap<StrategyType, f64> {
        &self.strategy_weights
    }

    /// Gets the most recent classification of the access pattern
    pub fn current_pattern_type(&self) -> PatternType {
        self.current_pattern_type
    }

    /// Returns immutable view of per-strategy performance metrics
    pub fn performance_metrics(&self) -> &HashMap<StrategyType, PerformanceMetrics> {
        &self.strategy_performance
    }

    /// Updates internal classification of the access pattern
    fn classify_pattern(&mut self) -> PatternType {
        if self.access_history.len() < 4 {
            return PatternType::Unknown;
        }
        if self.is_sequential_pattern() {
            return PatternType::Sequential;
        }
        if self.is_stride_pattern() {
            return PatternType::Stride;
        }
        if self.is_cyclic_pattern() {
            return PatternType::Cyclic;
        }
        if self.is_random_pattern() {
            return PatternType::Random;
        }
        PatternType::Mixed
    }

    /// Updates ensemble weights based on the most recent performance statistics
    fn update_strategy_weights(&mut self) {
        let total_weight: f64 = self
            .strategy_performance
            .values()
            .map(|metrics| {
                let success_rate = if metrics.total_predictions > 0 {
                    metrics.successful_predictions as f64 / metrics.total_predictions as f64
                } else {
                    0.5
                };
                let recent_success_rate = if !metrics.recent_successes.is_empty() {
                    metrics
                        .recent_successes
                        .iter()
                        .map(|&s| if s { 1.0 } else { 0.0 })
                        .sum::<f64>()
                        / metrics.recent_successes.len() as f64
                } else {
                    0.5
                };
                success_rate * 0.4 + recent_success_rate * 0.4 + metrics.confidence * 0.2
            })
            .sum();

        if total_weight > 0.0 {
            for (strategy_type, metrics) in &self.strategy_performance {
                let success_rate = if metrics.total_predictions > 0 {
                    metrics.successful_predictions as f64 / metrics.total_predictions as f64
                } else {
                    0.5
                };
                let recent_success_rate = if !metrics.recent_successes.is_empty() {
                    metrics
                        .recent_successes
                        .iter()
                        .map(|&s| if s { 1.0 } else { 0.0 })
                        .sum::<f64>()
                        / metrics.recent_successes.len() as f64
                } else {
                    0.5
                };
                let performance_score =
                    success_rate * 0.4 + recent_success_rate * 0.4 + metrics.confidence * 0.2;
                let new_weight = performance_score / total_weight;
                // Apply smoothing factor
                if let Some(current_weight) = self.strategy_weights.get_mut(strategy_type) {
                    *current_weight =
                        *current_weight * (1.0 - self.learning_rate) + new_weight * self.learning_rate;
                }
            }
        }

        // Normalize to sum exactly 1.0
        let weight_sum: f64 = self.strategy_weights.values().sum();
        if weight_sum > 0.0 {
            for weight in self.strategy_weights.values_mut() {
                *weight /= weight_sum;
            }
        }
    }

    /// Internal pattern recognition helper functions
    fn is_sequential_pattern(&self) -> bool {
        self.access_history.len() >= 3 && self.sequential_state.confidence > 0.6
    }
    fn is_stride_pattern(&self) -> bool {
        self.stride_state.detected_strides.len() >= 2 && self.stride_state.stride_confidence > 0.5
    }
    fn is_cyclic_pattern(&self) -> bool {
        if self.access_history.len() < 6 {
            return false;
        }
        let len = self.access_history.len();
        for cycle_len in 2..=(len / 2) {
            let mut is_cycle = true;
            for i in 0..(len - cycle_len) {
                if self.access_history[i] != self.access_history[i + cycle_len] {
                    is_cycle = false;
                    break;
                }
            }
            if is_cycle {
                return true;
            }
        }
        false
    }
    fn is_random_pattern(&self) -> bool {
        self.access_history.len() >= 10
            && !self.is_sequential_pattern()
            && !self.is_stride_pattern()
            && !self.is_cyclic_pattern()
    }
}

/// Default constructor delegates to `new`
impl<K> Default for AdaptivePrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Keys that can be mapped to/from `i64` for stride calculations
trait NumericKey: Clone + std::hash::Hash + Eq {
    fn to_i64(&self) -> i64;
    fn add_i64(&self, val: i64) -> Self;
}

// Implementations for common numeric types
impl NumericKey for i32 {
    fn to_i64(&self) -> i64 { *self as i64 }
    fn add_i64(&self, val: i64) -> Self { (*self as i64 + val) as i32 }
}
impl NumericKey for i64 {
    fn to_i64(&self) -> i64 { *self }
    fn add_i64(&self, val: i64) -> Self { *self + val }
}
impl NumericKey for usize {
    fn to_i64(&self) -> i64 { *self as i64 }
    fn add_i64(&self, val: i64) -> Self { (*self as i64 + val).max(0) as usize }
}

/// Core prefetch strategy implementation
impl<K> PrefetchStrategy<K> for AdaptivePrefetch<K>
where
    K: NumericKey + std::fmt::Debug,
{
    fn predict_next(&mut self, accessed_key: &K) -> Vec<K> {
        let mut predictions = Vec::new();
        let mut strategy_predictions: HashMap<StrategyType, Vec<K>> = HashMap::new();

        // Sequential predictions
        if let Some(stride) = self.sequential_state.stride {
            if self.sequential_state.confidence >= self.min_confidence {
                let preds: Vec<K> = (1..=self.prefetch_distance)
                    .map(|i| accessed_key.add_i64(stride * i as i64))
                    .collect();
                strategy_predictions.insert(StrategyType::Sequential, preds);
            }
        }

        // Stride predictions
        if let Some(stride) = self.stride_state.dominant_stride {
            if self.stride_state.stride_confidence >= self.min_confidence {
                let preds: Vec<K> = (1..=self.prefetch_distance)
                    .map(|i| accessed_key.add_i64(stride * i as i64))
                    .collect();
                strategy_predictions.insert(StrategyType::Stride, preds);
            }
        }

        // History-based predictions (simplified placeholder)
        if self.access_history.len() >= self.history_state.pattern_length {
            let predicted = accessed_key.add_i64(1);
            strategy_predictions.insert(StrategyType::HistoryBased, vec![predicted]);
        }

        // Combine predictions weighted by strategy confidence
        let mut weighted_predictions: HashMap<K, f64> = HashMap::new();
        for (strategy_type, preds) in strategy_predictions {
            if let Some(&weight) = self.strategy_weights.get(&strategy_type) {
                for (idx, pred) in preds.iter().enumerate() {
                    let pred_weight = weight / (idx + 1) as f64; // decay with distance
                    *weighted_predictions.entry(pred.clone()).or_insert(0.0) += pred_weight;
                }
            }
        }

        // Rank predictions by accumulated weight
        let mut sorted: Vec<_> = weighted_predictions.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (key, _) in sorted.into_iter().take(self.max_predictions) {
            predictions.push(key);
        }
        predictions
    }

    fn update_access_pattern(&mut self, key: &K) {
        // Maintain history buffer
        if self.access_history.len() >= self.max_history {
            self.access_history.pop_front();
        }
        self.access_history.push_back(key.clone());
        self.total_accesses += 1;

        // Update sequential detector
        if let Some(last_key) = self.sequential_state.last_key.clone() {
            let new_stride = key.to_i64() - last_key.to_i64();
            match self.sequential_state.stride {
                Some(current_stride) if new_stride == current_stride => {
                    self.sequential_state.consecutive_hits += 1;
                    self.sequential_state.confidence =
                        (self.sequential_state.confidence + 0.1).min(1.0);
                }
                Some(_) => {
                    self.sequential_state.consecutive_hits = 0;
                    self.sequential_state.confidence =
                        (self.sequential_state.confidence - 0.1).max(0.0);
                    self.sequential_state.stride = Some(new_stride);
                }
                None => {
                    self.sequential_state.stride = Some(new_stride);
                    self.sequential_state.confidence = 0.3;
                }
            }
        }
        self.sequential_state.last_key = Some(key.clone());

        // Update stride detector
        if self.access_history.len() >= 2 {
            let stride = key.to_i64()
                - self.access_history[self.access_history.len() - 2].to_i64();
            *self.stride_state.detected_strides.entry(stride).or_insert(0) += 1;
            self.stride_state.dominant_stride = self
                .stride_state
                .detected_strides
                .iter()
                .max_by_key(|&(_, count)| count)
                .map(|(&stride, _)| stride);
            if let Some(dominant) = self.stride_state.dominant_stride {
                let dom_count = self.stride_state.detected_strides[&dominant];
                let total: usize = self.stride_state.detected_strides.values().sum();
                self.stride_state.stride_confidence = dom_count as f64 / total as f64;
            }
        }

        // Update history model (frequency of small sequences)
        if self.access_history.len() >= self.history_state.pattern_length {
            let pattern: Vec<K> = self
                .access_history
                .iter()
                .rev()
                .take(self.history_state.pattern_length)
                .rev()
                .cloned()
                .collect();
            *self.history_state.pattern_frequencies.entry(pattern).or_insert(0) += 1;
        }

        // Refresh pattern classification
        self.current_pattern_type = self.classify_pattern();
        if self.total_accesses % 20 == 0 {
            self.update_strategy_weights();
        }
    }

    /// Resets prefetcher to initial state
    fn reset(&mut self) {
        self.access_history.clear();
        self.total_accesses = 0;
        self.current_pattern_type = PatternType::Unknown;

        self.sequential_state = SequentialState {
            last_key: None,
            stride: None,
            consecutive_hits: 0,
            confidence: 0.5,
        };
        self.stride_state = StrideState {
            detected_strides: HashMap::new(),
            dominant_stride: None,
            stride_confidence: 0.0,
        };
        self.history_state = HistoryState {
            pattern_frequencies: HashMap::new(),
            pattern_length: 3,
        };

        for metrics in self.strategy_performance.values_mut() {
            *metrics = PerformanceMetrics {
                total_predictions: 0,
                successful_predictions: 0,
                recent_successes: VecDeque::with_capacity(50),
                confidence: 0.5,
            };
        }
        for weight in self.strategy_weights.values_mut() {
            *weight = 0.25;
        }
    }
}

// Benchmark adapter for Criterion integration
impl BenchmarkablePrefetch<i32> for AdaptivePrefetch<i32> {
    fn prefetch_type(&self) -> PrefetchType { PrefetchType::Adaptive }
}
impl BenchmarkablePrefetch<i64> for AdaptivePrefetch<i64> {
    fn prefetch_type(&self) -> PrefetchType { PrefetchType::Adaptive }
}
impl BenchmarkablePrefetch<usize> for AdaptivePrefetch<usize> {
    fn prefetch_type(&self) -> PrefetchType { PrefetchType::Adaptive }
}

