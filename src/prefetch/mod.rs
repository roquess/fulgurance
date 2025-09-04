use crate::PrefetchStrategy;
pub mod sequential;
pub mod markov;

pub use sequential::SequentialPrefetch;
pub use markov::MarkovPrefetch;

/// Enumeration of available prefetch strategy types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefetchType {
    Sequential,
    Markov,
    None,
}

impl PrefetchType {
    pub fn name(&self) -> &'static str {
        match self {
            PrefetchType::Sequential => "Sequential",
            PrefetchType::Markov => "Markov",
            PrefetchType::None => "None",
        }
    }
    
    pub fn description(&self) -> &'static str {
        match self {
            PrefetchType::Sequential => "Predicts sequential access patterns",
            PrefetchType::Markov => "Predicts with Markov chain access patterns",
            PrefetchType::None => "No prefetching - baseline strategy",
        }
    }
    
    pub fn all() -> &'static [PrefetchType] {
        &[
            PrefetchType::Sequential,
            PrefetchType::Markov,
            PrefetchType::None,
        ]
    }
}

/// No-op prefetch strategy for baseline comparisons
#[derive(Debug, Clone, Default)]
pub struct NoPrefetch;

impl<K> PrefetchStrategy<K> for NoPrefetch {
    fn predict_next(&mut self, _accessed_key: &K) -> Vec<K> {
        Vec::new()
    }
    
    fn update_access_pattern(&mut self, _key: &K) {
        // No-op
    }
    
    fn reset(&mut self) {
        // No-op
    }
}

/// Factory function to create prefetch strategies dynamically
pub fn create_prefetch_strategy_i32(
    prefetch_type: PrefetchType
) -> Box<dyn PrefetchStrategy<i32>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<i32>::new()),
        PrefetchType::Markov => Box::new(MarkovPrefetch::<i32>::new()),
        PrefetchType::None => Box::new(NoPrefetch),
    }
}

pub fn create_prefetch_strategy_i64(
    prefetch_type: PrefetchType
) -> Box<dyn PrefetchStrategy<i64>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<i64>::new()),
        PrefetchType::Markov => Box::new(MarkovPrefetch::<i64>::new()),
        PrefetchType::None => Box::new(NoPrefetch),
    }
}

pub fn create_prefetch_strategy_usize(
    prefetch_type: PrefetchType
) -> Box<dyn PrefetchStrategy<usize>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<usize>::new()),
        PrefetchType::Markov => Box::new(MarkovPrefetch::<usize>::new()),
        PrefetchType::None => Box::new(NoPrefetch),
    }
}

/// Trait for prefetch strategies supporting benchmarking
pub trait BenchmarkablePrefetch<K>: PrefetchStrategy<K>
where
    K: Clone,
{
    fn prefetch_type(&self) -> PrefetchType;
    
    fn benchmark_name(&self) -> String {
        format!("{}_prefetch", self.prefetch_type().name())
    }
    
    fn characteristics(&self) -> PrefetchCharacteristics {
        match self.prefetch_type() {
            PrefetchType::Sequential => PrefetchCharacteristics {
                prediction_accuracy: "High for sequential patterns",
                memory_overhead: "Low",
                cpu_overhead: "Very Low",
                adaptability: "Low",
                best_use_case: "Sequential data access",
            },
            PrefetchType::Markov => PrefetchCharacteristics {
                prediction_accuracy: "High for complex patterns",
                memory_overhead: "Medium",
                cpu_overhead: "Medium",
                adaptability: "High",
                best_use_case: "Complex non-sequential patterns",
            },
            PrefetchType::None => PrefetchCharacteristics {
                prediction_accuracy: "N/A",
                memory_overhead: "None",
                cpu_overhead: "None",
                adaptability: "N/A",
                best_use_case: "Baseline comparison",
            },
        }
    }
}

impl BenchmarkablePrefetch<i32> for NoPrefetch {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::None
    }
}

impl BenchmarkablePrefetch<i64> for NoPrefetch {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::None
    }
}

impl BenchmarkablePrefetch<usize> for NoPrefetch {
    fn prefetch_type(&self) -> PrefetchType {
        PrefetchType::None
    }
}

#[derive(Debug, Clone)]
pub struct PrefetchCharacteristics {
    pub prediction_accuracy: &'static str,
    pub memory_overhead: &'static str,
    pub cpu_overhead: &'static str,
    pub adaptability: &'static str,
    pub best_use_case: &'static str,
}

impl Default for PrefetchCharacteristics {
    fn default() -> Self {
        Self {
            prediction_accuracy: "Unknown",
            memory_overhead: "Unknown",
            cpu_overhead: "Unknown",
            adaptability: "Unknown",
            best_use_case: "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prefetch_type_properties() {
        assert_eq!(PrefetchType::Sequential.name(), "Sequential");
        assert_eq!(PrefetchType::Markov.name(), "Markov");
        assert_eq!(PrefetchType::None.name(), "None");
        
        assert!(PrefetchType::Sequential.description().contains("sequential"));
        assert!(PrefetchType::Markov.description().contains("Markov"));
        assert!(PrefetchType::None.description().contains("baseline"));
    }
    
    #[test]
    fn test_all_prefetch_types_listed() {
        let all_types = PrefetchType::all();
        assert!(all_types.contains(&PrefetchType::Sequential));
        assert!(all_types.contains(&PrefetchType::Markov));
        assert!(all_types.contains(&PrefetchType::None));
        assert_eq!(all_types.len(), 3);
    }
    
    #[test]
    fn test_no_prefetch_strategy() {
        let mut strategy = NoPrefetch;
        strategy.update_access_pattern(&42);
        assert!(strategy.predict_next(&42).is_empty());
        
        // Test reset with explicit type annotation
        <NoPrefetch as PrefetchStrategy<i32>>::reset(&mut strategy);
    }
    
    #[test]
    fn test_factory_creation() {
        let _sequential_i32 = create_prefetch_strategy_i32(PrefetchType::Sequential);
        let _markov_i32 = create_prefetch_strategy_i32(PrefetchType::Markov);
        let _none_i32 = create_prefetch_strategy_i32(PrefetchType::None);
        
        let _sequential_i64 = create_prefetch_strategy_i64(PrefetchType::Sequential);
        let _markov_i64 = create_prefetch_strategy_i64(PrefetchType::Markov);
        let _none_i64 = create_prefetch_strategy_i64(PrefetchType::None);
        
        let _sequential_usize = create_prefetch_strategy_usize(PrefetchType::Sequential);
        let _markov_usize = create_prefetch_strategy_usize(PrefetchType::Markov);
        let _none_usize = create_prefetch_strategy_usize(PrefetchType::None);
    }
    
    #[test]
    fn test_benchmark_characteristics() {
        let sequential = SequentialPrefetch::<i32>::new();
        let markov = MarkovPrefetch::<i32>::new();
        let none = NoPrefetch;
        
        let seq_chars = sequential.characteristics();
        assert_eq!(seq_chars.adaptability, "Low");
        assert_eq!(seq_chars.cpu_overhead, "Very Low");
        
        let markov_chars = markov.characteristics();
        assert_eq!(markov_chars.adaptability, "High");
        assert_eq!(markov_chars.cpu_overhead, "Medium");
        
        let none_chars = none.characteristics();
        assert_eq!(none_chars.prediction_accuracy, "N/A");
        assert_eq!(none_chars.memory_overhead, "None");
    }
    
    #[test]
    fn test_benchmark_names() {
        let sequential = SequentialPrefetch::<i32>::new();
        let markov = MarkovPrefetch::<i32>::new();
        let none = NoPrefetch;
        
        assert_eq!(sequential.benchmark_name(), "Sequential_prefetch");
        assert_eq!(markov.benchmark_name(), "Markov_prefetch");
        assert_eq!(none.benchmark_name(), "None_prefetch");
    }
}
