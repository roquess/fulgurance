use crate::PrefetchStrategy;
use super::{BenchmarkablePrefetch, PrefetchType};
use std::collections::{HashMap, VecDeque};

/// History-based prefetch strategy.
///
/// Learns correlations between recent access sequences and their next likely keys.
/// Useful for irregular or complex access patterns with temporal locality.
#[derive(Debug, Clone)]
pub struct HistoryBasedPrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    /// Sliding window of recent accesses
    access_history: VecDeque<K>,
    /// Correlation table mapping access patterns to next key statistics
    correlation_table: HashMap<Vec<K>, HashMap<K, CorrelationEntry>>,
    /// Maximum size of access history kept
    history_window_size: usize,
    /// Length of sequences (n-gram size) used as access patterns
    pattern_length: usize,
    /// Maximum number of predictions to output
    max_predictions: usize,
    /// Minimum frequency threshold for considering correlations
    min_frequency: usize,
    /// Learning rate for confidence adjustments
    learning_rate: f64,
    /// Total number of accesses processed (for aging/cleanup)
    total_observations: usize,
}

/// Statistics for a correlated next key
#[derive(Debug, Clone)]
struct CorrelationEntry {
    frequency: usize,
    confidence: f64,
    last_seen: usize,
    success_rate: f64,
}

impl<K> HistoryBasedPrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    /// Create new prefetcher with default parameters
    pub fn new() -> Self {
        Self::with_config(10, 3, 4, 2, 0.1)
    }

    /// Create prefetcher with custom parameters
    pub fn with_config(
        history_window_size: usize,
        pattern_length: usize,
        max_predictions: usize,
        min_frequency: usize,
        learning_rate: f64,
    ) -> Self {
        Self {
            access_history: VecDeque::with_capacity(history_window_size),
            correlation_table: HashMap::new(),
            history_window_size,
            pattern_length: pattern_length.max(1),
            max_predictions,
            min_frequency,
            learning_rate,
            total_observations: 0,
        }
    }

    /// Extracts the current pattern from access history or returns None if empty
    fn current_pattern(&self) -> Option<Vec<K>> {
        let len = self.access_history.len();
        if len >= self.pattern_length {
            Some(self.access_history.range(len - self.pattern_length..).cloned().collect())
        } else if len > 0 {
            Some(self.access_history.iter().cloned().collect())
        } else {
            None
        }
    }

    /// Cleans up low-value or expired correlations to bound memory
    fn cleanup_correlations(&mut self) {
        let now = self.total_observations;
        self.correlation_table.retain(|_, correlations| {
            correlations.retain(|_, entry| {
                entry.frequency >= self.min_frequency
                    || entry.confidence > 0.3
                    || (now - entry.last_seen) < self.history_window_size * 2
                    || entry.success_rate > 0.5
            });
            !correlations.is_empty()
        });
    }

    /// Returns diagnostic statistics for the correlation table
    pub fn stats(&self) -> HistoryStats {
        let total_patterns = self.correlation_table.len();
        let total_correlations: usize = self.correlation_table.values().map(|m| m.len()).sum();
        let avg_confidence: f64 = if total_correlations > 0 {
            self.correlation_table
                .values()
                .flat_map(|m| m.values())
                .map(|e| e.confidence)
                .sum::<f64>()
                / total_correlations as f64
        } else {
            0.0
        };
        HistoryStats {
            total_patterns,
            total_correlations,
            avg_confidence,
            total_observations: self.total_observations,
        }
    }
}

/// Diagnostic stats returned by `stats()`
#[derive(Debug, Clone)]
pub struct HistoryStats {
    pub total_patterns: usize,
    pub total_correlations: usize,
    pub avg_confidence: f64,
    pub total_observations: usize,
}

// Implementation for i32 keys
impl PrefetchStrategy<i32> for HistoryBasedPrefetch<i32> {
    fn predict_next(&mut self, _accessed_key: &i32) -> Vec<i32> {
        if let Some(pattern) = self.current_pattern() {
            if let Some(correlations) = self.correlation_table.get(&pattern) {
                let mut candidates: Vec<_> = correlations
                    .iter()
                    .filter(|(_, e)| e.frequency >= self.min_frequency && e.confidence > 0.2)
                    .collect();
                candidates.sort_by(|a, b| {
                    let score_a = a.1.confidence * (1.0 + a.1.frequency as f64 / 10.0) * (1.0 + a.1.success_rate);
                    let score_b = b.1.confidence * (1.0 + b.1.frequency as f64 / 10.0) * (1.0 + b.1.success_rate);
                    score_b.partial_cmp(&score_a).unwrap()
                });
                candidates.into_iter().take(self.max_predictions).map(|(k, _)| *k).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    fn update_access_pattern(&mut self, key: &i32) {
        if self.access_history.len() >= self.history_window_size {
            self.access_history.pop_front();
        }
        self.access_history.push_back(*key);
        self.total_observations += 1;

        if self.access_history.len() >= self.pattern_length + 1 {
            let pattern: Vec<i32> = self.access_history
                .range(..self.access_history.len() - 1)
                .cloned()
                .rev()
                .take(self.pattern_length)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let next_key = *key;

            let correlations = self.correlation_table.entry(pattern).or_default();
            let entry = correlations.entry(next_key).or_insert(CorrelationEntry {
                frequency: 0,
                confidence: 0.3,
                last_seen: self.total_observations,
                success_rate: 0.0,
            });

            entry.frequency += 1;
            entry.confidence = (entry.confidence + self.learning_rate * 0.5).min(1.0);
            entry.last_seen = self.total_observations;
        }

        if self.total_observations % (self.history_window_size * 5) == 0 {
            self.cleanup_correlations();
        }
    }

    fn reset(&mut self) {
        self.access_history.clear();
        self.correlation_table.clear();
        self.total_observations = 0;
    }
}

// Implementation for i64 keys
impl PrefetchStrategy<i64> for HistoryBasedPrefetch<i64> {
    fn predict_next(&mut self, _accessed_key: &i64) -> Vec<i64> {
        if let Some(pattern) = self.current_pattern() {
            if let Some(correlations) = self.correlation_table.get(&pattern) {
                let mut candidates: Vec<_> = correlations
                    .iter()
                    .filter(|(_, e)| e.frequency >= self.min_frequency && e.confidence > 0.2)
                    .collect();
                candidates.sort_by(|a, b| {
                    let score_a = a.1.confidence * (1.0 + a.1.frequency as f64 / 10.0) * (1.0 + a.1.success_rate);
                    let score_b = b.1.confidence * (1.0 + b.1.frequency as f64 / 10.0) * (1.0 + b.1.success_rate);
                    score_b.partial_cmp(&score_a).unwrap()
                });
                candidates.into_iter().take(self.max_predictions).map(|(k, _)| *k).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    fn update_access_pattern(&mut self, key: &i64) {
        if self.access_history.len() >= self.history_window_size {
            self.access_history.pop_front();
        }
        self.access_history.push_back(*key);
        self.total_observations += 1;

        if self.access_history.len() >= self.pattern_length + 1 {
            let pattern: Vec<i64> = self.access_history
                .range(..self.access_history.len() - 1)
                .cloned()
                .rev()
                .take(self.pattern_length)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let next_key = *key;

            let correlations = self.correlation_table.entry(pattern).or_default();
            let entry = correlations.entry(next_key).or_insert(CorrelationEntry {
                frequency: 0,
                confidence: 0.3,
                last_seen: self.total_observations,
                success_rate: 0.0,
            });

            entry.frequency += 1;
            entry.confidence = (entry.confidence + self.learning_rate * 0.5).min(1.0);
            entry.last_seen = self.total_observations;
        }

        if self.total_observations % (self.history_window_size * 5) == 0 {
            self.cleanup_correlations();
        }
    }

    fn reset(&mut self) {
        self.access_history.clear();
        self.correlation_table.clear();
        self.total_observations = 0;
    }
}

// Implementation for usize keys
impl PrefetchStrategy<usize> for HistoryBasedPrefetch<usize> {
    fn predict_next(&mut self, _accessed_key: &usize) -> Vec<usize> {
        if let Some(pattern) = self.current_pattern() {
            if let Some(correlations) = self.correlation_table.get(&pattern) {
                let mut candidates: Vec<_> = correlations
                    .iter()
                    .filter(|(_, e)| e.frequency >= self.min_frequency && e.confidence > 0.2)
                    .collect();
                candidates.sort_by(|a, b| {
                    let score_a = a.1.confidence * (1.0 + a.1.frequency as f64 / 10.0) * (1.0 + a.1.success_rate);
                    let score_b = b.1.confidence * (1.0 + b.1.frequency as f64 / 10.0) * (1.0 + b.1.success_rate);
                    score_b.partial_cmp(&score_a).unwrap()
                });
                candidates.into_iter().take(self.max_predictions).map(|(k, _)| *k).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    fn update_access_pattern(&mut self, key: &usize) {
        if self.access_history.len() >= self.history_window_size {
            self.access_history.pop_front();
        }
        self.access_history.push_back(*key);
        self.total_observations += 1;

        if self.access_history.len() >= self.pattern_length + 1 {
            let pattern: Vec<usize> = self.access_history
                .range(..self.access_history.len() - 1)
                .cloned()
                .rev()
                .take(self.pattern_length)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let next_key = *key;

            let correlations = self.correlation_table.entry(pattern).or_default();
            let entry = correlations.entry(next_key).or_insert(CorrelationEntry {
                frequency: 0,
                confidence: 0.3,
                last_seen: self.total_observations,
                success_rate: 0.0,
            });

            entry.frequency += 1;
            entry.confidence = (entry.confidence + self.learning_rate * 0.5).min(1.0);
            entry.last_seen = self.total_observations;
        }

        if self.total_observations % (self.history_window_size * 5) == 0 {
            self.cleanup_correlations();
        }
    }

    fn reset(&mut self) {
        self.access_history.clear();
        self.correlation_table.clear();
        self.total_observations = 0;
    }
}

impl BenchmarkablePrefetch<i32> for HistoryBasedPrefetch<i32> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::HistoryBased
    }
}
impl BenchmarkablePrefetch<i64> for HistoryBasedPrefetch<i64> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::HistoryBased
    }
}
impl BenchmarkablePrefetch<usize> for HistoryBasedPrefetch<usize> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::HistoryBased
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_pattern_learning() {
        let mut strategy = HistoryBasedPrefetch::<i32>::with_config(10, 2, 3, 1, 0.1);

        // Pattern 1->2->3 repeating several times
        let pattern = [1, 2, 3];
        for _ in 0..5 {
            for &key in &pattern {
                strategy.update_access_pattern(&key);
            }
        }

        // After pattern [1, 2], prediction should include 3
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        let predictions = strategy.predict_next(&2);
        assert!(predictions.contains(&3));
    }

    #[test]
    fn test_multiple_patterns() {
        let mut strategy = HistoryBasedPrefetch::<i32>::with_config(10, 2, 3, 1, 0.1);

        // Patterns: 1->2->4 and 1->2->5 with different frequencies
        for _ in 0..3 {
            strategy.update_access_pattern(&1);
            strategy.update_access_pattern(&2);
            strategy.update_access_pattern(&4);
        }
        for _ in 0..2 {
            strategy.update_access_pattern(&1);
            strategy.update_access_pattern(&2);
            strategy.update_access_pattern(&5);
        }

        // Prediction after [1, 2] should include 4 or 5
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        let predictions = strategy.predict_next(&2);
        assert!(predictions.contains(&4) || predictions.contains(&5));
    }

    #[test]
    fn test_clear_stats() {
        let mut strategy = HistoryBasedPrefetch::<i32>::with_config(10, 2, 3, 1, 0.2);
        for _ in 0..10 {
            strategy.update_access_pattern(&10);
            strategy.update_access_pattern(&20);
            strategy.update_access_pattern(&30);
        }
        let stats = strategy.stats();
        assert!(stats.avg_confidence > 0.4);
        assert!(stats.total_patterns > 0);
    }

    #[test]
    fn test_cleanup() {
        let mut strategy = HistoryBasedPrefetch::<i32>::with_config(5, 2, 3, 1, 0.1);
        for i in 0..100 {
            strategy.update_access_pattern(&i);
            strategy.update_access_pattern(&(i + 1));
            strategy.update_access_pattern(&(i + 2));
        }
        let stats_before = strategy.stats();
        strategy.cleanup_correlations();
        let stats_after = strategy.stats();
        assert!(stats_after.total_patterns <= stats_before.total_patterns);
    }

    #[test]
    fn test_history_window_limit() {
        let mut strategy = HistoryBasedPrefetch::<i32>::with_config(3, 2, 3, 1, 0.1);
        for i in 0..10 {
            strategy.update_access_pattern(&i);
        }
        assert!(strategy.access_history.len() <= 3);
        assert_eq!(strategy.access_history.back(), Some(&9));
    }

    #[test]
    fn test_reset_clears_state() {
        let mut strategy = HistoryBasedPrefetch::<i32>::new();
        for i in 0..5 {
            strategy.update_access_pattern(&i);
        }
        let stats_before = strategy.stats();
        assert!(stats_before.total_observations > 0);
        strategy.reset();
        let stats_after = strategy.stats();
        assert_eq!(stats_after.total_observations, 0);
        assert_eq!(stats_after.total_patterns, 0);
        assert!(strategy.access_history.is_empty());
    }
}

