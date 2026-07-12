// Diagnostic: does Phase 2's array parser work on REAL clang -O0 output
// (not just hand-built modules)? Run with: cargo run --release --example diagnose_arrays

use std::path::Path;
use metamorphic_engine::llvm_frontend;
use metamorphic_engine::interpreter::Interpreter;

fn main() {
    let (module, ceiling) = llvm_frontend::load_target_module(
        Path::new(r"C:\Temp\llvmtest2\arraytest.c"),
        10,
        None,
    ).expect("failed to load target module");

    println!("Module '{}' — {} function(s), baseline instrs: {}, ceiling: {}",
        module.name, module.functions.len(), module.instruction_count(), ceiling);
    for f in &module.functions {
        println!("  fn '{}' params={:?} blocks={}", f.name, f.params, f.basic_blocks.len());
    }

    let interpreter = Interpreter::new();
    let entry_fn = &module.functions[0];
    let result = interpreter.execute_function(&module, entry_fn, &[], None);
    println!("Interpreted result: {:?}", result);
    println!("Expected: Ok(150)  (10+20+30+40+50)");

    // Also run through the real validator machinery against all 7 passes,
    // same shape as diagnose_loop.rs / diagnose_calls.rs earlier this session.
    use metamorphic_engine::engine::OptimizationEngine;
    use metamorphic_engine::passes::{PassRegistry, PassManager, OptimizationLevel};
    let registry = PassRegistry::new();
    println!("\nTesting all {} passes against the array-containing module:", registry.list_all().len());
    for pass_id in registry.list_all() {
        let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
        eng.load_module(module.clone());
        let mut pm = PassManager::new();
        if let Some(pass) = registry.create_pass(&pass_id) {
            pm.add(pass);
        } else { continue; }
        *eng.pass_manager_mut() = pm;
        let changed = eng.run_passes_on_module();
        let result = eng.validate_optimization_result(&entry_fn.name);
        let after = eng.get_module().map(|m| m.instruction_count()).unwrap_or(0);
        println!("  [{}] changed={} instrs_after={} passed={} details={}",
            pass_id, changed, after, result.passed, result.failure_details);
    }
}
