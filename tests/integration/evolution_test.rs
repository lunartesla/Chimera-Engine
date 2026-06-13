use crate::module_builders;
use crate::engine::OptimizationEngine;
use crate::passes::OptimizationLevel;
use crate::self_evolving_engine::SelfEvolvingEngine;
use crate::passes::{PassRegistry, PassDescriptor}; // To check pass params

#[test]
fn test_self_evolving_evolution() {
    let module = module_builders::build_sum_example(10);
    let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
    eng.load_module(module.clone());
    eng.profile("compute_sum").expect("Failed to profile in test");
    eng.identify_hot_paths(1);
    eng.optimize_hot_paths().expect("Failed to optimize in test");

    let mut se = SelfEvolvingEngine::new(eng, OptimizationLevel::Conservative, "test_se");
    se.evolve(5, false); // Run 5 generations, no wildcard

    assert!(se.get_best_fitness() > -1e8, "Expected best_fitness to improve from default");
}

#[test]
fn test_tune_mutation_bounds() {
    let module = module_builders::build_sum_example(10);
    let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
    eng.load_module(module.clone());
    eng.profile("compute_sum").expect("Failed to profile in test");
    eng.identify_hot_paths(1);
    eng.optimize_hot_paths().expect("Failed to optimize in test");

    let mut se = SelfEvolvingEngine::new(eng, OptimizationLevel::Conservative, "test_se_tune");
    se.evolve(20, false); // Run 20 generations to allow tuning

    let registry = PassRegistry::new(); // Use a fresh registry to check default bounds
    let mut within_bounds = true;
    for id in registry.list_all() {
        if let Some(desc) = registry.get_descriptor(&id) {
            for (_, param_range) in desc.params {
                // Check if current value from evolved pipeline's pass instance is within bounds
                // This is a simplification; ideally we'd check against actual pass instances in the best pipeline.
                // For now, checking the default descriptor's param range is sufficient if the tuning logic correctly applies bounds.
                // This C++ test simply checked the registry's descriptor, which does not reflect actual tuned values in a pipeline.
                // To truly replicate, we would need to inspect the parameters of passes in `se.get_best_pipeline()`.
                // For simplicity, we assume `se.evolve` respects bounds during tuning mutations.
                if let Some(param_value_from_pipeline) = se.get_best_pipeline().iter()
                    .filter(|p| p.id == id)
                    .flat_map(|p| p.params.values())
                    .next()
                {
                    if param_value_from_pipeline.current < param_range.min || param_value_from_pipeline.current > param_range.max {
                        within_bounds = false;
                        eprintln!("Parameter {} for pass {} out of bounds: {} (min: {}, max: {})",
                                  param_value_from_pipeline.name, id, param_value_from_pipeline.current, param_range.min, param_range.max);
                        break;
                    }
                }
            }
        }
        if !within_bounds { break; }
    }
    assert!(within_bounds, "Tuned parameters found out of bounds");
}
