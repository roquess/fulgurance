//! Prefetch strategy implementations
use crate::PrefetchStrategy;
pub mod sequential;
// pub mod stride;    // Will be ajouté plus tard
// pub mod history;   // Will be ajouté plus tard
// pub mod adaptive;  // Will be ajouté plus tard

pub use sequential::SequentialPrefetch;

/// Enumeration of available prefetch strategy types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefetchType {
    Sequential,
    Stride,
    History,
    Adaptive,
    None,
}

impl PrefetchType {
    pub fn name(&self) -> &'static str {
        match self {
            PrefetchType::Sequential => "Sequential",
            PrefetchType::Stride => "Stride",
            PrefetchType::History => "History",
            PrefetchType::Adaptive => "Adaptive",
            PrefetchType::None => "None",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            PrefetchType::Sequential => "Predicts sequential access patterns",
            PrefetchType::Stride => "Detects and follows stride-based access patterns",
            PrefetchType::History => "Uses access history for intelligent prediction",
            PrefetchType::Adaptive => "Dynamically adapts between multiple strategies",
            PrefetchType::None => "No prefetching - baseline strategy",
        }
    }
    pub fn all() -> &'static [PrefetchType] {
        &[
            PrefetchType::Sequential,
            PrefetchType::Stride,
            PrefetchType::History,
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
        PrefetchType::None => Box::new(NoPrefetch),
        PrefetchType::Stride => {
            unimplemented!("Stride prefetch strategy not yet implemented")
        },
        PrefetchType::History => {
            unimplemented!("History prefetch strategy not yet implemented")
        },
        PrefetchType::Adaptive => {
            unimplemented!("Adaptive prefetch strategy not yet implemented")
        },
    }
}

pub fn create_prefetch_strategy_usize(
    prefetch_type: PrefetchType
) -> Box<dyn PrefetchStrategy<usize>> {
    match prefetch_type {
        PrefetchType::Sequential => Box::new(SequentialPrefetch::<usize>::new()),
        PrefetchType::None => Box::new(NoPrefetch),
        PrefetchType::Stride => {
            unimplemented!("Stride prefetch strategy not yet implemented")
        },
        PrefetchType::History => {
            unimplemented!("History prefetch strategy not yet implemented")
        },
        PrefetchType::Adaptive => {
            unimplemented!("Adaptive prefetch strategy not yet implemented")
        },
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
            PrefetchType::None => PrefetchCharacteristics {
                prediction_accuracy: "N/A",
                memory_overhead: "None",
                cpu_overhead: "None",
                adaptability: "N/A",
                best_use_case: "Baseline comparison",
            },
            _ => PrefetchCharacteristics::default(),
        }
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
        assert_eq!(PrefetchType::None.name(), "None");
        assert!(PrefetchType::Sequential.description().contains("sequential"));
        assert!(PrefetchType::None.description().contains("baseline"));
    }
    #[test]
    fn test_all_prefetch_types_listed() {
        let all_types = PrefetchType::all();
        assert!(all_types.contains(&PrefetchType::Sequential));
        assert!(all_types.contains(&PrefetchType::None));
        assert!(all_types.len() >= 2);
    }
    #[test]
    fn test_no_prefetch_strategy() {
        let mut strategy = NoPrefetch;
        strategy.update_access_pattern(&42);
        assert!(strategy.predict_next(&42).is_empty());
        // Correction E0282 : appel pleinement qualifié avec annotation de type générique
        <NoPrefetch as PrefetchStrategy<i32>>::reset(&mut strategy);
    }
    #[test]
    fn test_sequential_factory_creation() {
        let _strategy = create_prefetch_strategy_i32(PrefetchType::Sequential);
        let _strategy2 = create_prefetch_strategy_usize(PrefetchType::Sequential);
    }
}

