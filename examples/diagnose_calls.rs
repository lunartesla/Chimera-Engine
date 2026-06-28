// Diagnostic: does Phase 1 (intra-module calls) actually work end-to-end?
// Run with: cargo run --release --example diagnose_calls

use std::path::Path;
use metamorphic_engine::llvm_frontend;

fn main() {
    let (module, ceiling) = llvm_frontend::load_target_module(
        Path::new(r"C:\Temp\llvmtest2\calltest.c"),
        10, // baked param for the top-level target function ("compute")
        Some("compute"),
    ).expect("failed to load target module");

    println!("Module '{}' — {} function(s), baseline instrs: {}, ceiling: {}",
        module.name, module.functions.len(), module.instruction_count(), ceiling);

    for f in &module.functions {
        println!("  fn '{}' params={:?} blocks={}", f.name, f.params, f.basic_blocks.len());
    }

    // Run it through the interpreter directly (this is what validate_optimization
    // does under the hood) and print the actual returned value, to confirm the
    // call chain executes correctly end to end, not just parses without erroring.
    let interpreter = metamorphic_engine::interpreter::Interpreter::new();
    let entry_fn = module.functions.iter().find(|f| f.name == "target_calltest" || f.name.contains("compute"))
        .unwrap_or(&module.functions[0]);
    println!("  Using entry fn: '{}'", entry_fn.name);
    let result = interpreter.execute_function(&module, entry_fn, &[], None);
    println!("  Interpreted result of '{}': {:?}", entry_fn.name, result);

    // compute(10) by hand: sum over i=0..9 of (i*2+1) = 1+3+5+7+9+11+13+15+17+19 = 100
    println!("  Expected (hand-computed compute(10)): 100");

    // Now run it through the SAME machinery actually used during real
    // evolution scoring (OptimizationEngine + Validator), not just a raw
    // interpreter call, with each of the 7 registered passes individually —
    // same diagnostic shape as diagnose_loop.rs earlier this session.
    use metamorphic_engine::engine::OptimizationEngine;
    use metamorphic_engine::passes::{PassManager, PassRegistry, OptimizationLevel};

    let registry = PassRegistry::new();
    println!("\n  Testing all {} passes against the call-containing module:", registry.list_all().len());
    for pass_id in registry.list_all() {
        let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
        eng.load_module(module.clone());
        let mut pm = PassManager::new();
        if let Some(pass) = registry.create_pass(&pass_id) {
            pm.add(pass);
        } else {
            continue;
        }
        *eng.pass_manager_mut() = pm;
        let changed = eng.run_passes_on_module();
        let result = eng.validate_optimization_result(&entry_fn.name);
        let after = eng.get_module().map(|m| m.instruction_count()).unwrap_or(0);
        println!("    [{}] changed={} instrs_after={} passed={} details={}",
            pass_id, changed, after, result.passed, result.failure_details);
    }
}
