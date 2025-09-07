use crate::PrefetchStrategy;
use super::{BenchmarkablePrefetch, PrefetchType};
use std::collections::HashMap;

/// Stride prefetch strategy.
///
/// Detects multiple stride patterns concurrently and dynamically
/// adjusts to the dominant stride based on confidence.
/// Handles interleaved access patterns and switches between stride patterns.
#[derive(Debug, Clone)]
pub struct StridePrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    /// History of recent accesses
    access_history: Vec<K>,
    /// Map of candidate strides to their statistics
    stride_patterns: HashMap<i64, StridePattern>,
    /// Max number of accesses to track in history
    max_history: usize,
    /// How many keys ahead to prefetch
    prefetch_distance: usize,
    /// Max predicted keys allowed per access
    max_predictions: usize,
    /// Minimum confidence threshold to use a stride prediction
    min_confidence: f64,
    /// Currently dominant stride pattern (highest confidence)
    dominant_stride: Option<i64>,
}

/// Statistics for each detected stride pattern
#[derive(Debug, Clone)]
pub struct StridePattern {
    /// Confidence score from 0.0 to 1.0
    confidence: f64,
    /// How many times this stride has been observed
    occurrences: usize,
}

impl<K> StridePrefetch<K>
where
    K: Clone + std::hash::Hash + Eq,
{
    /// Creates a default stride prefetcher with preset configuration
    pub fn new() -> Self {
        Self::with_config(8, 3, 4, 0.6)
    }

    /// Creates a stride prefetcher with custom configuration
    pub fn with_config(
        max_history: usize,
        prefetch_distance: usize,
        max_predictions: usize,
        min_confidence: f64,
    ) -> Self {
        Self {
            access_history: Vec::with_capacity(max_history),
            stride_patterns: HashMap::new(),
            max_history,
            prefetch_distance,
            max_predictions,
            min_confidence,
            dominant_stride: None,
        }
    }

    /// Update dominant stride: pick the stride with highest confidence above threshold
    fn update_dominant_stride(&mut self) {
        self.dominant_stride = self
            .stride_patterns
            .iter()
            .filter(|(_, pattern)| pattern.confidence >= self.min_confidence)
            .max_by(|a, b| a.1.confidence.partial_cmp(&b.1.confidence).unwrap())
            .map(|(stride, _)| *stride);
    }

    /// Remove low-confidence stride patterns to prevent growth
    fn cleanup_patterns(&mut self) {
        self.stride_patterns
            .retain(|_, p| p.confidence > 0.1 || p.occurrences > 2);
    }
}

impl Default for StridePrefetch<i32> {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for StridePrefetch<i64> {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for StridePrefetch<usize> {
    fn default() -> Self {
        Self::new()
    }
}

/// Implement `PrefetchStrategy<i32>` for `StridePrefetch<i32>`
impl PrefetchStrategy<i32> for StridePrefetch<i32> {
    fn predict_next(&mut self, accessed_key: &i32) -> Vec<i32> {
        let mut predictions = Vec::with_capacity(self.max_predictions);

        if let Some(dominant) = self.dominant_stride {
            if let Some(pattern) = self.stride_patterns.get(&dominant) {
                if pattern.confidence >= self.min_confidence {
                    for i in 1..=self.prefetch_distance {
                        if predictions.len() >= self.max_predictions {
                            break;
                        }
                        predictions.push(accessed_key + (dominant as i32) * i as i32);
                    }
                }
            }
        }

        // Fill with other confident strides if needed
        if predictions.len() < self.max_predictions {
            let mut other_strides: Vec<_> = self
                .stride_patterns
                .iter()
                .filter(|(stride, pattern)| {
                    pattern.confidence >= self.min_confidence && Some(**stride) != self.dominant_stride
                })
                .collect();

            other_strides.sort_by(|a, b| b.1.confidence.partial_cmp(&a.1.confidence).unwrap());

            for (stride, _) in other_strides.iter().take(2) {
                if predictions.len() >= self.max_predictions {
                    break;
                }
                let candidate = accessed_key + **stride as i32;
                if !predictions.contains(&candidate) {
                    predictions.push(candidate);
                }
            }
        }

        predictions
    }

    fn update_access_pattern(&mut self, key: &i32) {
        self.access_history.push(*key);
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }
        if self.access_history.len() >= 2 {
            let current = *key as i64;
            for i in 1..self.access_history.len() {
                let prev = self.access_history[self.access_history.len() - 1 - i] as i64;
                let stride = current - prev;
                let pattern = self.stride_patterns.entry(stride).or_insert(StridePattern {
                    confidence: 0.3,
                    occurrences: 0,
                });
                pattern.occurrences += 1;
                if pattern.occurrences > 2 {
                    pattern.confidence = (pattern.confidence + 0.05).min(1.0);
                }
            }
        }
        self.update_dominant_stride();
        if self.stride_patterns.len() > 10 {
            self.cleanup_patterns();
        }
    }

    fn reset(&mut self) {
        self.access_history.clear();
        self.stride_patterns.clear();
        self.dominant_stride = None;
    }
}

/// Implement `PrefetchStrategy<i64>` for `StridePrefetch<i64>`
impl PrefetchStrategy<i64> for StridePrefetch<i64> {
    fn predict_next(&mut self, accessed_key: &i64) -> Vec<i64> {
        let mut predictions = Vec::with_capacity(self.max_predictions);

        if let Some(dominant) = self.dominant_stride {
            if let Some(pattern) = self.stride_patterns.get(&dominant) {
                if pattern.confidence >= self.min_confidence {
                    for i in 1..=self.prefetch_distance {
                        if predictions.len() >= self.max_predictions {
                            break;
                        }
                        predictions.push(accessed_key + dominant * i as i64);
                    }
                }
            }
        }

        if predictions.len() < self.max_predictions {
            let mut other_strides: Vec<_> = self
                .stride_patterns
                .iter()
                .filter(|(stride, pattern)| {
                    pattern.confidence >= self.min_confidence && Some(**stride) != self.dominant_stride
                })
                .collect();

            other_strides.sort_by(|a, b| b.1.confidence.partial_cmp(&a.1.confidence).unwrap());

            for (stride, _) in other_strides.iter().take(2) {
                if predictions.len() >= self.max_predictions {
                    break;
                }
                let candidate = accessed_key + **stride;
                if !predictions.contains(&candidate) {
                    predictions.push(candidate);
                }
            }
        }

        predictions
    }

    fn update_access_pattern(&mut self, key: &i64) {
        self.access_history.push(*key);
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }
        if self.access_history.len() >= 2 {
            let current = *key;
            for i in 1..self.access_history.len() {
                let prev = self.access_history[self.access_history.len() - 1 - i];
                let stride = current - prev;
                let pattern = self.stride_patterns.entry(stride).or_insert(StridePattern {
                    confidence: 0.3,
                    occurrences: 0,
                });
                pattern.occurrences += 1;
                if pattern.occurrences > 2 {
                    pattern.confidence = (pattern.confidence + 0.05).min(1.0);
                }
            }
        }
        self.update_dominant_stride();
        if self.stride_patterns.len() > 10 {
            self.cleanup_patterns();
        }
    }

    fn reset(&mut self) {
        self.access_history.clear();
        self.stride_patterns.clear();
        self.dominant_stride = None;
    }
}

/// Implement `PrefetchStrategy<usize>` for `StridePrefetch<usize>`
impl PrefetchStrategy<usize> for StridePrefetch<usize> {
    fn predict_next(&mut self, accessed_key: &usize) -> Vec<usize> {
        let mut predictions = Vec::with_capacity(self.max_predictions);

        if let Some(dominant) = self.dominant_stride {
            if dominant > 0 {
                if let Some(pattern) = self.stride_patterns.get(&dominant) {
                    if pattern.confidence >= self.min_confidence {
                        for i in 1..=self.prefetch_distance {
                            if predictions.len() >= self.max_predictions {
                                break;
                            }
                            if let Some(next_key) = accessed_key.checked_add((dominant as usize) * i) {
                                predictions.push(next_key);
                            }
                        }
                    }
                }
            }
        }

        if predictions.len() < self.max_predictions {
            let mut other_strides: Vec<_> = self
                .stride_patterns
                .iter()
                .filter(|(stride, pattern)| {
                    **stride > 0
                        && pattern.confidence >= self.min_confidence
                        && Some(**stride) != self.dominant_stride
                })
                .collect();

            other_strides.sort_by(|a, b| b.1.confidence.partial_cmp(&a.1.confidence).unwrap());

            for (stride, _) in other_strides.iter().take(2) {
                if predictions.len() >= self.max_predictions {
                    break;
                }
                if let Some(next_key) = accessed_key.checked_add(**stride as usize) {
                    if !predictions.contains(&next_key) {
                        predictions.push(next_key);
                    }
                }
            }
        }

        predictions
    }

    fn update_access_pattern(&mut self, key: &usize) {
        self.access_history.push(*key);
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }
        if self.access_history.len() >= 2 {
            let current = *key as i64;
            for i in 1..self.access_history.len() {
                let prev = self.access_history[self.access_history.len() - 1 - i] as i64;
                let stride = current - prev;
                if stride > 0 {
                    let pattern = self.stride_patterns.entry(stride).or_insert(StridePattern {
                        confidence: 0.3,
                        occurrences: 0,
                    });
                    pattern.occurrences += 1;
                    if pattern.occurrences > 2 {
                        pattern.confidence = (pattern.confidence + 0.05).min(1.0);
                    }
                }
            }
        }
        self.update_dominant_stride();
        if self.stride_patterns.len() > 10 {
            self.cleanup_patterns();
        }
    }

    fn reset(&mut self) {
        self.access_history.clear();
        self.stride_patterns.clear();
        self.dominant_stride = None;
    }
}

/// Criterion benchmarks integration for i32, i64, usize
impl BenchmarkablePrefetch<i32> for StridePrefetch<i32> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Stride
    }
}
impl BenchmarkablePrefetch<i64> for StridePrefetch<i64> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Stride
    }
}
impl BenchmarkablePrefetch<usize> for StridePrefetch<usize> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Stride
    }
}


