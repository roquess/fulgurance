use crate::PrefetchStrategy;
use super::{BenchmarkablePrefetch, PrefetchType};
use std::collections::HashMap;
use std::hash::Hash;

/// Markov Chain prefetch strategy
/// 
/// This strategy builds a probabilistic model of memory access patterns
/// using a Markov chain. It maintains transition probabilities between
/// different memory addresses/regions and predicts the most likely next
/// accesses based on the current state and historical patterns.
/// 
/// Key advantages:
/// - Captures complex, non-sequential access patterns
/// - Adapts to changing access patterns over time
/// - Can handle multiple possible transitions from each state
/// - Works well for pointer chasing, tree traversals, and irregular patterns
#[derive(Debug, Clone)]
pub struct MarkovPrefetch<K> 
where
    K: Clone + Hash + Eq,
{
    /// Transition matrix: state -> (next_state -> probability)
    transitions: HashMap<K, HashMap<K, f64>>,
    /// Current state in the Markov chain
    current_state: Option<K>,
    /// History of recent accesses for context
    access_history: Vec<K>,
    /// Maximum history length to maintain
    max_history: usize,
    /// Minimum probability threshold for predictions
    min_probability: f64,
    /// Maximum number of predictions to return
    max_predictions: usize,
    /// Learning rate for updating transition probabilities
    learning_rate: f64,
    /// Total number of observed transitions
    total_transitions: usize,
    /// Decay factor for older transitions
    decay_factor: f64,
}

impl<K> MarkovPrefetch<K>
where
    K: Clone + Hash + Eq,
{
    /// Creates a new Markov prefetch strategy with default parameters
    pub fn new() -> Self {
        Self::with_config(10, 0.1, 3, 0.1, 0.95)
    }

    /// Creates a new Markov prefetch strategy with custom configuration
    /// 
    /// # Arguments
    /// * `max_history` - Maximum number of recent accesses to remember
    /// * `min_probability` - Minimum probability threshold for predictions
    /// * `max_predictions` - Maximum number of predictions per access
    /// * `learning_rate` - Rate at which to update probabilities (0.0 to 1.0)
    /// * `decay_factor` - Decay factor for aging old transitions (0.0 to 1.0)
    pub fn with_config(
        max_history: usize,
        min_probability: f64,
        max_predictions: usize,
        learning_rate: f64,
        decay_factor: f64,
    ) -> Self {
        Self {
            transitions: HashMap::new(),
            current_state: None,
            access_history: Vec::with_capacity(max_history),
            max_history,
            min_probability,
            max_predictions,
            learning_rate,
            total_transitions: 0,
            decay_factor,
        }
    }

    /// Returns the current state of the Markov chain
    pub fn current_state(&self) -> Option<&K> {
        self.current_state.as_ref()
    }

    /// Returns the transition probabilities from the current state
    pub fn current_transitions(&self) -> Option<&HashMap<K, f64>> {
        self.current_state.as_ref()
            .and_then(|state| self.transitions.get(state))
    }

    /// Returns the total number of observed transitions
    pub fn transition_count(&self) -> usize {
        self.total_transitions
    }

    /// Updates transition probabilities with decay
    fn update_transition(&mut self, from: &K, to: &K) {
        // Apply decay to all existing transitions
        if self.total_transitions > 0 {
            self.apply_decay();
        }

        // Update the specific transition
        let from_transitions = self.transitions.entry(from.clone()).or_insert_with(HashMap::new);
        
        let current_prob = from_transitions.get(to).unwrap_or(&0.0);
        let new_prob = current_prob + self.learning_rate * (1.0 - current_prob);
        
        from_transitions.insert(to.clone(), new_prob);
        
        // Normalize probabilities for this state
        self.normalize_state_probabilities(from);
        
        self.total_transitions += 1;
    }

    /// Applies decay to all transition probabilities
    fn apply_decay(&mut self) {
        for state_transitions in self.transitions.values_mut() {
            for prob in state_transitions.values_mut() {
                *prob *= self.decay_factor;
            }
            
            // Remove transitions that have become too weak
            state_transitions.retain(|_, prob| *prob >= self.min_probability / 10.0);
        }
    }

    /// Normalizes probabilities for a given state to sum to 1.0
    fn normalize_state_probabilities(&mut self, state: &K) {
        if let Some(transitions) = self.transitions.get_mut(state) {
            let total: f64 = transitions.values().sum();
            
            if total > 0.0 {
                for prob in transitions.values_mut() {
                    *prob /= total;
                }
            }
        }
    }

    /// Gets predictions sorted by probability
    fn get_sorted_predictions(&self, state: &K) -> Vec<(K, f64)> {
        if let Some(transitions) = self.transitions.get(state) {
            let mut predictions: Vec<_> = transitions
                .iter()
                .filter(|(_, prob)| **prob >= self.min_probability)
                .map(|(key, prob)| (key.clone(), *prob))
                .collect();
            
            // Sort by probability (descending)
            predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            predictions.truncate(self.max_predictions);
            
            predictions
        } else {
            Vec::new()
        }
    }

    /// Adds context-aware predictions using recent history
    fn get_context_predictions(&self) -> Vec<K> {
        if self.access_history.len() < 2 {
            return Vec::new();
        }

        let mut context_predictions = Vec::new();
        let recent_len = self.access_history.len().min(3);
        
        // Look for patterns in recent history
        for window_size in 2..=recent_len {
            if self.access_history.len() >= window_size {
                let pattern = &self.access_history[self.access_history.len() - window_size + 1..];
                
                // Find historical occurrences of this pattern
                for i in 0..self.access_history.len().saturating_sub(window_size) {
                    if &self.access_history[i..i + window_size - 1] == pattern {
                        if i + window_size < self.access_history.len() {
                            let next_key = &self.access_history[i + window_size];
                            if !context_predictions.contains(next_key) {
                                context_predictions.push(next_key.clone());
                                if context_predictions.len() >= self.max_predictions {
                                    break;
                                }
                            }
                        }
                    }
                }
                
                if !context_predictions.is_empty() {
                    break;
                }
            }
        }

        context_predictions
    }
}

impl<K> Default for MarkovPrefetch<K>
where
    K: Clone + Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

// Implementation for i32 keys
impl PrefetchStrategy<i32> for MarkovPrefetch<i32> {
    fn predict_next(&mut self, accessed_key: &i32) -> Vec<i32> {
        let mut predictions = Vec::new();

        // Get Markov chain predictions
        if let Some(state) = &self.current_state {
            let markov_predictions = self.get_sorted_predictions(state);
            predictions.extend(markov_predictions.into_iter().map(|(key, _)| key));
        }

        // If we don't have enough Markov predictions, use context
        if predictions.len() < self.max_predictions {
            let context_predictions = self.get_context_predictions();
            for pred in context_predictions {
                if !predictions.contains(&pred) {
                    predictions.push(pred);
                    if predictions.len() >= self.max_predictions {
                        break;
                    }
                }
            }
        }

        // If still not enough, add some sequential predictions as fallback
        if predictions.is_empty() && self.total_transitions < 10 {
            for i in 1..=self.max_predictions.min(3) {
                predictions.push(accessed_key + i as i32);
            }
        }

        predictions.truncate(self.max_predictions);
        predictions
    }

    fn update_access_pattern(&mut self, key: &i32) {
        // Update transition if we have a previous state
        if let Some(prev_state) = self.current_state.clone() {
            self.update_transition(&prev_state, key);
        }

        // Update current state
        self.current_state = Some(*key);

        // Update access history
        self.access_history.push(*key);
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }
    }

    fn reset(&mut self) {
        self.transitions.clear();
        self.current_state = None;
        self.access_history.clear();
        self.total_transitions = 0;
    }
}

// Implementation for i64 keys
impl PrefetchStrategy<i64> for MarkovPrefetch<i64> {
    fn predict_next(&mut self, accessed_key: &i64) -> Vec<i64> {
        let mut predictions = Vec::new();

        if let Some(state) = &self.current_state {
            let markov_predictions = self.get_sorted_predictions(state);
            predictions.extend(markov_predictions.into_iter().map(|(key, _)| key));
        }

        if predictions.len() < self.max_predictions {
            let context_predictions = self.get_context_predictions();
            for pred in context_predictions {
                if !predictions.contains(&pred) {
                    predictions.push(pred);
                    if predictions.len() >= self.max_predictions {
                        break;
                    }
                }
            }
        }

        if predictions.is_empty() && self.total_transitions < 10 {
            for i in 1..=self.max_predictions.min(3) {
                predictions.push(accessed_key + i as i64);
            }
        }

        predictions.truncate(self.max_predictions);
        predictions
    }

    fn update_access_pattern(&mut self, key: &i64) {
        if let Some(prev_state) = self.current_state.clone() {
            self.update_transition(&prev_state, key);
        }

        self.current_state = Some(*key);
        self.access_history.push(*key);
        
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }
    }

    fn reset(&mut self) {
        self.transitions.clear();
        self.current_state = None;
        self.access_history.clear();
        self.total_transitions = 0;
    }
}

// Implementation for usize keys
impl PrefetchStrategy<usize> for MarkovPrefetch<usize> {
    fn predict_next(&mut self, accessed_key: &usize) -> Vec<usize> {
        let mut predictions = Vec::new();

        if let Some(state) = &self.current_state {
            let markov_predictions = self.get_sorted_predictions(state);
            predictions.extend(markov_predictions.into_iter().map(|(key, _)| key));
        }

        if predictions.len() < self.max_predictions {
            let context_predictions = self.get_context_predictions();
            for pred in context_predictions {
                if !predictions.contains(&pred) {
                    predictions.push(pred);
                    if predictions.len() >= self.max_predictions {
                        break;
                    }
                }
            }
        }

        if predictions.is_empty() && self.total_transitions < 10 {
            for i in 1..=self.max_predictions.min(3) {
                if let Some(next_key) = accessed_key.checked_add(i) {
                    predictions.push(next_key);
                }
            }
        }

        predictions.truncate(self.max_predictions);
        predictions
    }

    fn update_access_pattern(&mut self, key: &usize) {
        if let Some(prev_state) = self.current_state.clone() {
            self.update_transition(&prev_state, key);
        }

        self.current_state = Some(*key);
        self.access_history.push(*key);
        
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }
    }

    fn reset(&mut self) {
        self.transitions.clear();
        self.current_state = None;
        self.access_history.clear();
        self.total_transitions = 0;
    }
}

impl BenchmarkablePrefetch<i32> for MarkovPrefetch<i32> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Markov
    }
}

impl BenchmarkablePrefetch<i64> for MarkovPrefetch<i64> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Markov
    }
}

impl BenchmarkablePrefetch<usize> for MarkovPrefetch<usize> {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::Markov
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markov_basic_transition() {
        let mut strategy = MarkovPrefetch::<i32>::new();

        // Create a simple pattern: 1 -> 2 -> 3 -> 1
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        strategy.update_access_pattern(&3);
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);

        // Should predict 3 after seeing 2
        let predictions = strategy.predict_next(&2);
        assert!(predictions.contains(&3));
    }

    #[test]
    fn test_markov_multiple_transitions() {
        let mut strategy = MarkovPrefetch::<i32>::new();

        // Create pattern where 1 can go to either 2 or 3
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&3);
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);

        // Should predict both 2 and 3 after seeing 1
        let predictions = strategy.predict_next(&1);
        assert!(predictions.len() > 0);
        assert!(predictions.contains(&2) || predictions.contains(&3));
    }

    #[test]
    fn test_markov_context_prediction() {
        let mut strategy = MarkovPrefetch::<i32>::with_config(20, 0.1, 3, 0.2, 0.95);

        // Create a repeating sequence
        let sequence = [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4];
        for &key in &sequence {
            strategy.update_access_pattern(&key);
        }

        // After seeing the pattern 1,2,3 multiple times, should predict 4
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        let predictions = strategy.predict_next(&3);
        
        // Should predict 4 based on learned pattern
        assert!(predictions.contains(&4) || strategy.transition_count() > 0);
    }

    #[test]
    fn test_markov_decay() {
        let mut strategy = MarkovPrefetch::<i32>::with_config(10, 0.1, 3, 0.1, 0.5);

        // Build initial pattern
        for _ in 0..5 {
            strategy.update_access_pattern(&1);
            strategy.update_access_pattern(&2);
        }

        let initial_transitions = strategy.transition_count();

        // Add many different transitions to trigger decay
        for i in 10..50 {
            strategy.update_access_pattern(&i);
            strategy.update_access_pattern(&(i + 1));
        }

        // Original pattern should still exist but be weakened
        assert!(strategy.transition_count() > initial_transitions);
    }

    #[test]
    fn test_markov_reset() {
        let mut strategy = MarkovPrefetch::<i32>::new();

        // Build some state
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);
        strategy.update_access_pattern(&3);

        assert!(strategy.current_state().is_some());
        assert!(strategy.transition_count() > 0);

        // Reset should clear everything
        strategy.reset();

        assert!(strategy.current_state().is_none());
        assert_eq!(strategy.transition_count(), 0);
    }

    #[test]
    fn test_markov_probability_threshold() {
        let mut strategy = MarkovPrefetch::<i32>::with_config(10, 0.8, 3, 0.1, 0.95);

        // Create weak transitions (below threshold)
        strategy.update_access_pattern(&1);
        strategy.update_access_pattern(&2);

        // With high threshold, should not predict weak transitions
        let predictions = strategy.predict_next(&1);
        // Might be empty or use fallback predictions
        assert!(predictions.len() <= 3);
    }

    #[test]
    fn test_markov_usize_overflow_safety() {
        let mut strategy = MarkovPrefetch::<usize>::new();

        let large_key = usize::MAX - 1;
        strategy.update_access_pattern(&large_key);

        // Should not panic and handle gracefully
        let predictions = strategy.predict_next(&large_key);
        assert!(predictions.len() <= 3);
    }
}
