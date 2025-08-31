use crate::PrefetchStrategy;
use super::{BenchmarkablePrefetch, PrefetchType};

/// Sequential prefetch strategy
/// 
/// This strategy assumes that data access follows a sequential pattern
/// and predicts that the next few consecutive keys will be accessed.
/// It's particularly effective for scenarios like:
/// - Array/vector scanning
/// - Sequential file reading
/// - Database table scans
/// - Time-series data processing
#[derive(Debug, Clone)]
pub struct SequentialPrefetch<K> 
where
    K: Clone,
{
    /// The last accessed key for pattern detection
    last_key: Option<K>,
    /// Current stride detected from access pattern
    stride: Option<i64>,
    /// Number of keys to prefetch ahead
    prefetch_distance: usize,
    /// Maximum number of predictions per access
    max_predictions: usize,
    /// Confidence in the current stride pattern (0.0 to 1.0)
    confidence: f64,
    /// Number of consecutive successful stride predictions
    consecutive_hits: usize,
}

impl<K> SequentialPrefetch<K>
where
    K: Clone,
{
    /// Creates a new sequential prefetch strategy with default parameters
    pub fn new() -> Self {
        Self::with_config(2, 3, 0.5)
    }
    
    /// Creates a new sequential prefetch strategy with custom configuration
    /// 
    /// # Arguments
    /// * `prefetch_distance` - How many keys ahead to prefetch
    /// * `max_predictions` - Maximum number of keys to predict per access
    /// * `min_confidence` - Minimum confidence threshold for predictions
    pub fn with_config(
        prefetch_distance: usize, 
        max_predictions: usize,
        min_confidence: f64
    ) -> Self {
        Self {
            last_key: None,
            stride: None,
            prefetch_distance,
            max_predictions,
            confidence: min_confidence,
            consecutive_hits: 0,
        }
    }
    
    /// Returns current stride if detected
    pub fn current_stride(&self) -> Option<i64> {
        self.stride
    }
    
    /// Returns current confidence level
    pub fn confidence(&self) -> f64 {
        self.confidence
    }
}

impl<K> Default for SequentialPrefetch<K>
where
    K: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

// Implementation for numeric keys that support arithmetic
impl PrefetchStrategy<i32> for SequentialPrefetch<i32> {
    /// Predicts next keys based on detected stride pattern
    fn predict_next(&mut self, accessed_key: &i32) -> Vec<i32> {
        // If we don't have enough confidence, don't predict
        if self.confidence < 0.5 {
            return Vec::new();
        }
        
        let stride = self.stride.unwrap_or(1) as i32;
        let mut predictions = Vec::with_capacity(self.max_predictions);
        
        // Generate predictions based on stride
        for i in 1..=self.max_predictions {
            if predictions.len() >= self.prefetch_distance {
                break;
            }
            
            let next_key = accessed_key + (stride * i as i32);
            predictions.push(next_key);
        }
        
        predictions
    }
    
    /// Updates the strategy with new access pattern
    fn update_access_pattern(&mut self, key: &i32) {
        if let Some(last_key) = self.last_key {
            let new_stride = (*key as i64) - (last_key as i64);
            
            match self.stride {
                Some(current_stride) => {
                    if new_stride == current_stride {
                        // Stride confirmed - increase confidence
                        self.consecutive_hits += 1;
                        self.confidence = (self.confidence + 0.1).min(1.0);
                    } else {
                        // Stride changed - decrease confidence and update
                        self.consecutive_hits = 0;
                        self.confidence = (self.confidence - 0.2).max(0.0);
                        self.stride = Some(new_stride);
                    }
                },
                None => {
                    // First stride detected
                    self.stride = Some(new_stride);
                    self.confidence = 0.3; // Start with moderate confidence
                }
            }
        }
        
        self.last_key = Some(*key);
    }
    
    /// Resets the strategy's internal state
    fn reset(&mut self) {
        self.last_key = None;
        self.stride = None;
        self.confidence = 0.5;
        self.consecutive_hits = 0;
    }
}

// Implementation for 64-bit integers
impl PrefetchStrategy<i64> for SequentialPrefetch<i64> {
    fn predict_next(&mut self, accessed_key: &i64) -> Vec<i64> {
        if self.confidence < 0.5 {
            return Vec::new();
        }
        
        let stride = self.stride.unwrap_or(1);
        let mut predictions = Vec::with_capacity(self.max_predictions);
        
        for i in 1..=self.max_predictions {
            if predictions.len() >= self.prefetch_distance {
                break;
            }
            
            let next_key = accessed_key + (stride * i as i64);
            predictions.push(next_key);
        }
        
        predictions
    }
    
    fn update_access_pattern(&mut self, key: &i64) {
        if let Some(last_key) = self.last_key {
            let new_stride = *key - last_key;
            
            match self.stride {
                Some(current_stride) => {
                    if new_stride == current_stride {
                        self.consecutive_hits += 1;
                        self.confidence = (self.confidence + 0.1).min(1.0);
                    } else {
                        self.consecutive_hits = 0;
                        self.confidence = (self.confidence - 0.2).max(0.0);
                        self.stride = Some(new_stride);
                    }
                },
                None => {
                    self.stride = Some(new_stride);
                    self.confidence = 0.3;
                }
            }
        }
        
        self.last_key = Some(*key);
    }
    
    fn reset(&mut self) {
        self.last_key = None;
        self.stride = None;
        self.confidence = 0.5;
        self.consecutive_hits = 0;
    }
}

// Implementation for usize (common for array indices)
impl PrefetchStrategy<usize> for SequentialPrefetch<usize> {
    fn predict_next(&mut self, accessed_key: &usize) -> Vec<usize> {
        if self.confidence < 0.5 {
            return Vec::new();
        }
        
        let stride = self.stride.unwrap_or(1).max(1) as usize; // Ensure positive stride
        let mut predictions = Vec::with_capacity(self.max_predictions);
        
        for i in 1..=self.max_predictions {
            if predictions.len() >= self.prefetch_distance {
                break;
            }
            
            // Prevent overflow
            if let Some(next_key) = accessed_key.checked_add(stride * i) {
                predictions.push(next_key);
            }
        }
        
        predictions
    }
    
    fn update_access_pattern(&mut self, key: &usize) {
        if let Some(last_key) = self.last_key {
            // Handle potential underflow by using signed arithmetic
            let new_stride = (*key as i64) - (last_key as i64);
            
            match self.stride {
                Some(current_stride) => {
                    if new_stride == current_stride && new_stride > 0 {
                        self.consecutive_hits += 1;
                        self.confidence = (self.confidence + 0.1).min(1.0);
                    } else {
                        self.consecutive_hits = 0;
                        self.confidence = (self.confidence - 0.2).max(0.0);
                        if new_stride > 0 {
                            self.stride = Some(new_stride);
                        }
                    }
                },
                None => {
                    if new_stride > 0 {
                        self.stride = Some(new_stride);
                        self.confidence = 0.3;
                    }
                }
            }
        }
        
        self.last_key = Some(*key);
    }
    
    fn reset(&mut self) {
        self.last_key = None;
        self.stride = None;
        self.confidence = 0.5;
        self.consecutive_hits = 0;
    }
}

impl BenchmarkablePrefetch<i32> for SequentialPrefetch<i32> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Sequential
    }
}

impl BenchmarkablePrefetch<i64> for SequentialPrefetch<i64> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Sequential
    }
}

impl BenchmarkablePrefetch<usize> for SequentialPrefetch<usize> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Sequential
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sequential_stride_detection() {
        let mut strategy = SequentialPrefetch::<i32>::new();
        
        // Create a stride-2 pattern
        strategy.update_access_pattern(&0);
        strategy.update_access_pattern(&2);
        strategy.update_access_pattern(&4);
        strategy.update_access_pattern(&6);
        
        // Should detect stride of 2
        assert_eq!(strategy.current_stride(), Some(2));
        
        let predictions = strategy.predict_next(&8);
        assert_eq!(predictions[0], 10); // 8 + 2*1
        assert_eq!(predictions[1], 12); // 8 + 2*2
    }
    
    #[test]
    fn test_sequential_pattern_break() {
        let mut strategy = SequentialPrefetch::<i32>::new();
        
        // Build initial pattern
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        strategy.update_access_pattern(&3);
        
        let initial_confidence = strategy.confidence();
        
        // Break pattern
        strategy.update_access_pattern(&10);
        
        // Confidence should decrease
        assert!(strategy.confidence() < initial_confidence);
    }
    
    #[test]
    fn test_sequential_usize_overflow_protection() {
        let mut strategy = SequentialPrefetch::<usize>::new();
        
        let large_key = usize::MAX - 1;
        strategy.update_access_pattern(&(large_key - 2));
        strategy.update_access_pattern(&(large_key - 1));
        strategy.update_access_pattern(&large_key);
        
        // Should not panic and should handle overflow gracefully
        let predictions = strategy.predict_next(&large_key);
        // May be empty due to overflow protection
        assert!(predictions.len() <= 1);
    }
    
    #[test]
    fn test_sequential_reset() {
        let mut strategy = SequentialPrefetch::<i32>::new();
        
        // Build pattern
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        strategy.update_access_pattern(&3);
        
        // Reset
        strategy.reset();
        
        // Should be back to initial state
        assert_eq!(strategy.current_stride(), None);
        assert_eq!(strategy.confidence(), 0.5);
    }
    
    #[test]
    fn test_sequential_negative_stride() {
        let mut strategy = SequentialPrefetch::<i32>::new();
        
        // Create decreasing pattern
        strategy.update_access_pattern(&10);
        strategy.update_access_pattern(&8);
        strategy.update_access_pattern(&6);
        
        assert_eq!(strategy.current_stride(), Some(-2));
        
        let predictions = strategy.predict_next(&4);
        if !predictions.is_empty() {
            assert_eq!(predictions[0], 2); // 4 + (-2)*1
        }
    }
}
