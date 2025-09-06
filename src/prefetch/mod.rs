use crate::PrefetchStrategy;

pub mod sequential;
pub mod markov;
pub mod stride;
pub mod history_based;
pub mod adaptive;

pub use sequential::SequentialPrefetch;
pub use markov::MarkovPrefetch;
pub use stride::StridePrefetch;
pub use history_based::HistoryBasedPrefetch;
pub use adaptive::AdaptivePrefetch;

/// Enumeration of available prefetch strategy types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefetchType {
    Sequential,
    Markov,
    Stride,
    HistoryBased,
    Adaptive,
    None,
}

impl PrefetchType {
    pub fn name(&self) -> &'static str {
        match self {
            PrefetchType::Sequential => "Sequential",
            PrefetchType::Markov => "Markov",
            PrefetchType::Stride => "Stride",
            PrefetchType::HistoryBased => "HistoryBased",
            PrefetchType::Adaptive => "Adaptive",
            PrefetchType::None => "None",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PrefetchType::Sequential => "Predicts sequential access patterns with stride detection",
            PrefetchType::Markov => "Predicts with Markov chain access patterns",
            PrefetchType::Stride => "Detects and predicts multiple stride patterns simultaneously",
            PrefetchType::HistoryBased => "Learns from historical access sequences (n-grams)",
            PrefetchType::Adaptive => "Dynamically combines multiple strategies with performance weighting",
            PrefetchType::None => "No prefetching - baseline strategy",
        }
    }

    pub fn all() -> &'static [PrefetchType] {
        &[
            PrefetchType::Sequential,
            PrefetchType::Markov,
            PrefetchType::Stride,
            PrefetchType::HistoryBased,
            PrefetchType::Adaptive,
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

    fn update_access_pattern(&mut self, _key: &K) {}

    fn reset(&mut self) {}
}

/// Factory function to create prefetch strategies dynamically
pub fn create_prefetch_strategy_i32(prefetch_type: PrefetchType) -> Box<dyn PrefetchStrategy<i32>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<i32>::new()),
        PrefetchType::Markov => Box::new(MarkovPrefetch::<i32>::new()),
        PrefetchType::Stride => Box::new(StridePrefetch::<i32>::new()),
        PrefetchType::HistoryBased => Box::new(HistoryBasedPrefetch::<i32>::new()),
        PrefetchType::Adaptive => Box::new(AdaptivePrefetch::<i32>::new()),
        PrefetchType::None => Box::new(NoPrefetch),
    }
}

pub fn create_prefetch_strategy_i64(prefetch_type: PrefetchType) -> Box<dyn PrefetchStrategy<i64>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<i64>::new()),
        PrefetchType::Markov => Box::new(MarkovPrefetch::<i64>::new()),
        PrefetchType::Stride => Box::new(StridePrefetch::<i64>::new()),
        PrefetchType::HistoryBased => Box::new(HistoryBasedPrefetch::<i64>::new()),
        PrefetchType::Adaptive => Box::new(AdaptivePrefetch::<i64>::new()),
        PrefetchType::None => Box::new(NoPrefetch),
    }
}

pub fn create_prefetch_strategy_usize(prefetch_type: PrefetchType) -> Box<dyn PrefetchStrategy<usize>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<usize>::new()),
        PrefetchType::Markov => Box::new(MarkovPrefetch::<usize>::new()),
        PrefetchType::Stride => Box::new(StridePrefetch::<usize>::new()),
        PrefetchType::HistoryBased => Box::new(HistoryBasedPrefetch::<usize>::new()),
        PrefetchType::Adaptive => Box::new(AdaptivePrefetch::<usize>::new()),
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
                best_use_case: "Sequential data access with consistent strides",
            },
            PrefetchType::Markov => PrefetchCharacteristics {
                prediction_accuracy: "High for complex patterns",
                memory_overhead: "Medium",
                cpu_overhead: "Medium",
                adaptability: "High",
                best_use_case: "Complex non-sequential patterns with state dependencies",
            },
            PrefetchType::Stride => PrefetchCharacteristics {
                prediction_accuracy: "High for multi-stride patterns",
                memory_overhead: "Low-Medium",
                cpu_overhead: "Low",
                adaptability: "Medium",
                best_use_case: "Multiple concurrent stride patterns",
            },
            PrefetchType::HistoryBased => PrefetchCharacteristics {
                prediction_accuracy: "High for repeating sequences",
                memory_overhead: "Medium-High",
                cpu_overhead: "Medium",
                adaptability: "High",
                best_use_case: "Complex repeating access patterns",
            },
            PrefetchType::Adaptive => PrefetchCharacteristics {
                prediction_accuracy: "High across various patterns",
                memory_overhead: "Medium-High",
                cpu_overhead: "Medium-High",
                adaptability: "Very High",
                best_use_case: "Mixed or changing access patterns",
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
        assert_eq!(PrefetchType::Stride.name(), "Stride");
        assert_eq!(PrefetchType::HistoryBased.name(), "HistoryBased");
        assert_eq!(PrefetchType::Adaptive.name(), "Adaptive");
        assert_eq!(PrefetchType::None.name(), "None");
        
        assert!(PrefetchType::Sequential.description().contains("sequential"));
        assert!(PrefetchType::Markov.description().contains("Markov"));
        assert!(PrefetchType::Stride.description().contains("stride"));
        assert!(PrefetchType::HistoryBased.description().contains("historical"));
        assert!(PrefetchType::Adaptive.description().contains("combines"));
        assert!(PrefetchType::None.description().contains("baseline"));
    }

    #[test]
    fn test_all_prefetch_types_listed() {
        let all_types = PrefetchType::all();
        assert!(all_types.contains(&PrefetchType::Sequential));
        assert!(all_types.contains(&PrefetchType::Markov));
        assert!(all_types.contains(&PrefetchType::Stride));
        assert!(all_types.contains(&PrefetchType::HistoryBased));
        assert!(all_types.contains(&PrefetchType::Adaptive));
        assert!(all_types.contains(&PrefetchType::None));
        assert_eq!(all_types.len(), 6);
    }

    #[test]
    fn test_no_prefetch_strategy() {
        let mut strategy = NoPrefetch;
        strategy.update_access_pattern(&42);
        assert!(strategy.predict_next(&42).is_empty());
        <NoPrefetch as PrefetchStrategy<i32>>::reset(&mut strategy);
    }

    #[test]
    fn test_factory_creation() {
        // Test i32 factories
        let _sequential_i32 = create_prefetch_strategy_i32(PrefetchType::Sequential);
        let _markov_i32 = create_prefetch_strategy_i32(PrefetchType::Markov);
        let _stride_i32 = create_prefetch_strategy_i32(PrefetchType::Stride);
        let _history_i32 = create_prefetch_strategy_i32(PrefetchType::HistoryBased);
        let _adaptive_i32 = create_prefetch_strategy_i32(PrefetchType::Adaptive);
        let _none_i32 = create_prefetch_strategy_i32(PrefetchType::None);
        
        // Test i64 factories
        let _sequential_i64 = create_prefetch_strategy_i64(PrefetchType::Sequential);
        let _markov_i64 = create_prefetch_strategy_i64(PrefetchType::Markov);
        let _stride_i64 = create_prefetch_strategy_i64(PrefetchType::Stride);
        let _history_i64 = create_prefetch_strategy_i64(PrefetchType::HistoryBased);
        let _adaptive_i64 = create_prefetch_strategy_i64(PrefetchType::Adaptive);
        let _none_i64 = create_prefetch_strategy_i64(PrefetchType::None);
        
        // Test usize factories
        let _sequential_usize = create_prefetch_strategy_usize(PrefetchType::Sequential);
        let _markov_usize = create_prefetch_strategy_usize(PrefetchType::Markov);
        let _stride_usize = create_prefetch_strategy_usize(PrefetchType::Stride);
        let _history_usize = create_prefetch_strategy_usize(PrefetchType::HistoryBased);
        let _adaptive_usize = create_prefetch_strategy_usize(PrefetchType::Adaptive);
        let _none_usize = create_prefetch_strategy_usize(PrefetchType::None);
    }

    #[test]
    fn test_benchmark_characteristics() {
        let sequential = SequentialPrefetch::<i32>::new();
        let markov = MarkovPrefetch::<i32>::new();
        let stride = StridePrefetch::<i32>::new();
        let history = HistoryBasedPrefetch::<i32>::new();
        let adaptive = AdaptivePrefetch::<i32>::new();
        let none = NoPrefetch;

        let seq_chars = sequential.characteristics();
        assert_eq!(seq_chars.adaptability, "Low");
        assert_eq!(seq_chars.cpu_overhead, "Very Low");

        let markov_chars = markov.characteristics();
        assert_eq!(markov_chars.adaptability, "High");
        assert_eq!(markov_chars.cpu_overhead, "Medium");

        let stride_chars = stride.characteristics();
        assert_eq!(stride_chars.adaptability, "Medium");
        assert_eq!(stride_chars.cpu_overhead, "Low");

        let history_chars = history.characteristics();
        assert_eq!(history_chars.adaptability, "High");
        assert_eq!(history_chars.memory_overhead, "Medium-High");

        let adaptive_chars = adaptive.characteristics();
        assert_eq!(adaptive_chars.adaptability, "Very High");
        assert_eq!(adaptive_chars.cpu_overhead, "Medium-High");

        let none_chars = <NoPrefetch as BenchmarkablePrefetch<i32>>::characteristics(&none);
        assert_eq!(none_chars.prediction_accuracy, "N/A");
        assert_eq!(none_chars.memory_overhead, "None");
    }

    #[test]
    fn test_benchmark_names() {
        let sequential = SequentialPrefetch::<i32>::new();
        let markov = MarkovPrefetch::<i32>::new();
        let stride = StridePrefetch::<i32>::new();
        let history = HistoryBasedPrefetch::<i32>::new();
        let adaptive = AdaptivePrefetch::<i32>::new();
        let none = NoPrefetch;

        assert_eq!(sequential.benchmark_name(), "Sequential_prefetch");
        assert_eq!(markov.benchmark_name(), "Markov_prefetch");
        assert_eq!(stride.benchmark_name(), "Stride_prefetch");
        assert_eq!(history.benchmark_name(), "HistoryBased_prefetch");
        assert_eq!(adaptive.benchmark_name(), "Adaptive_prefetch");
        assert_eq!(<NoPrefetch as BenchmarkablePrefetch<i32>>::benchmark_name(&none), "None_prefetch");
    }
}
