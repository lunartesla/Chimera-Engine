use crate::module_builders;
use crate::interpreter::Interpreter;
use crate::profiler::RuntimeProfiler;
use crate::engine::OptimizationEngine;
use crate::passes::OptimizationLevel;
use std::collections::HashMap; // For args

#[test]
fn test_functional_n5() {
    let module = module_builders::build_sum_example(5);
    let interpreter = Interpreter::new();
    let mut profiler = RuntimeProfiler::new();
    let result = interpreter.execute_function(&module.functions[0], &mut profiler).expect("Interpreter failed");
    assert_eq!(result, 10, "Expected 10 for build_sum_example(5)");
}

#[test]
fn test_functional_n10() {
    let module = module_builders::build_sum_example(10);
    let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
    eng.load_module(module.clone());
    eng.profile("compute_sum").expect("Failed to profile in test");
    eng.identify_hot_paths(1);
    eng.optimize_hot_paths().expect("Failed to optimize in test");

    let interpreter = Interpreter::new();
    let mut profiler = RuntimeProfiler::new();
    let result = interpreter.execute_function(&eng.get_module().unwrap().functions[0], &mut profiler).expect("Interpreter failed");
    assert_eq!(result, 45, "Expected 45 for build_sum_example(10) after optimization");
}

#[test]
fn test_functional_n20() {
    let module = module_builders::build_sum_example(20);
    let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
    eng.load_module(module.clone());
    eng.profile("compute_sum").expect("Failed to profile in test");
    eng.identify_hot_paths(1);
    eng.optimize_hot_paths().expect("Failed to optimize in test");

    let interpreter = Interpreter::new();
    let mut profiler = RuntimeProfiler::new();
    let result = interpreter.execute_function(&eng.get_module().unwrap().functions[0], &mut profiler).expect("Interpreter failed");
    assert_eq!(result, 190, "Expected 190 for build_sum_example(20) after optimization");
}
