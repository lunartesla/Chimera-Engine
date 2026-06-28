// Diagnostic: which single pass(es) break our real ingested loop module?
// Run with: cargo run --release --example diagnose_loop

use std::path::Path;
use metamorphic_engine::engine::OptimizationEngine;
use metamorphic_engine::passes::{PassManager, PassRegistry, OptimizationLevel};
use metamorphic_engine::llvm_frontend;

fn main() {
    let (module, _ceiling) = llvm_frontend::load_target_module(
        Path::new(r"C:\Temp\llvmtest\test.c"),
        10,
        None,
    ).expect("failed to load target module");

    let fn_name = module.functions[0].name.clone();
    println!("Module '{}' fn '{}' baseline instrs: {}", module.name, fn_name, module.instruction_count());

    let registry = PassRegistry::new();
    let all_pass_ids = registry.list_all();
    println!("Testing {} passes individually...\n", all_pass_ids.len());

    for pass_id in &all_pass_ids {
        let mut engine = OptimizationEngine::new(OptimizationLevel::Conservative);
        engine.load_module(module.clone());

        let mut pm = PassManager::new();
        if let Some(pass) = registry.create_pass(pass_id) {
            pm.add(pass);
        } else {
            println!("[{}] could not instantiate", pass_id);
            continue;
        }
        *engine.pass_manager_mut() = pm;

        let changed = engine.run_passes_on_module();
        let result = engine.validate_optimization_result(&fn_name);
        let after_count = engine.get_module().map(|m| m.instruction_count()).unwrap_or(0);

        println!("[{}] changed={} instrs_after={} -> passed={} details={}",
            pass_id, changed, after_count, result.passed, result.failure_details);
    }
}
