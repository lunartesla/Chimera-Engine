use criterion::{criterion_group, criterion_main, Criterion};
use crate::module_builders;
use crate::engine::OptimizationEngine;
use crate::passes::OptimizationLevel;
use crate::self_evolving_engine::SelfEvolvingEngine;
use crate::interpreter::Interpreter;
use crate::profiler::RuntimeProfiler;
use crate::passes::Pass; // Needed for Pass::run

// Benchmark for running the full optimization pipeline on a sample module
fn bench_full_pipeline(c: &mut Criterion) {
    let mut module = module_builders::build_sum_example(100); // Larger input for meaningful benchmark
    let mut engine = OptimizationEngine::new(OptimizationLevel::Balanced); // Balanced uses all passes

    c.bench_function("full_optimize_module", |b| {
        b.iter(|| {
            let mut cloned_module = module.clone(); // Clone module for each iteration
            engine.load_module(cloned_module);
            engine.profile("compute_sum").expect("Failed to profile in bench");
            engine.identify_hot_paths(1);
            engine.optimize_hot_paths().expect("Failed to optimize in bench");
        });
    });
}

// Benchmark for the constant folding pass specifically
fn bench_constant_folding(c: &mut Criterion) {
    let mut module = module_builders::build_sum_example(100); // Module with binary ops
    let mut cf_pass = crate::passes::constant_folding::ConstantFoldingPass::new(); // Direct pass instance

    c.bench_function("pass_constant_folding", |b| {
        b.iter(|| {
            let mut cloned_module = module.clone();
            cf_pass.run(&mut cloned_module).expect("Constant folding failed in bench");
        });
    });
}

// Benchmark for a short evolution run
fn bench_evolution_10_gens(c: &mut Criterion) {
    let module = module_builders::build_sum_example(20);
    let eng = OptimizationEngine::new(OptimizationLevel::Conservative);

    c.bench_function("evolution_10_generations", |b| {
        b.iter(|| {
            let mut se = SelfEvolvingEngine::new(eng.clone(), OptimizationLevel::Conservative, "bench_se");
            se.evolve(10, false); // 10 generations, no wildcard
        });
    });
}

criterion_group!(benches, bench_full_pipeline, bench_constant_folding, bench_evolution_10_gens);
criterion_main!(benches);
