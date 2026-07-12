// Verifies all 7 hand-built modules (5 modified/original + 2 new) are
// well-formed, execute without error, and reports what each of the 7
// registered passes finds against them — the actual proof that the hidden
// opportunities designed into simple_sum/fib_loop/branch_heavy/array_reduce/
// recursive_calls are real and discoverable, not just present-but-invisible
// (see the CSE/strength_reduction findings from this session — this harness
// is what surfaced them, and will surface it again honestly here).
//
// Run with: cargo run --release --example verify_modules

use metamorphic_engine::module_builders;
use metamorphic_engine::interpreter::Interpreter;
use metamorphic_engine::engine::OptimizationEngine;
use metamorphic_engine::passes::{PassRegistry, PassManager, OptimizationLevel};

fn check_module(module: metamorphic_engine::ir::module::Module) {
    println!("=== {} ===", module.name);
    let entry_fn_name = module.functions[0].name.clone();
    println!("  baseline instrs: {}, functions: {}", module.instruction_count(), module.functions.len());

    let interpreter = Interpreter::new();
    let result = interpreter.execute_function(&module, &module.functions[0], &[], None);
    println!("  interpreted result: {:?}", result);

    let registry = PassRegistry::new();
    for pass_id in registry.list_all() {
        let mut eng = OptimizationEngine::new(OptimizationLevel::Conservative);
        eng.load_module(module.clone());
        let mut pm = PassManager::new();
        if let Some(pass) = registry.create_pass(&pass_id) { pm.add(pass); } else { continue; }
        *eng.pass_manager_mut() = pm;
        let changed = eng.run_passes_on_module();
        let validation = eng.validate_optimization_result(&entry_fn_name);
        let after = eng.get_module().map(|m| m.instruction_count()).unwrap_or(0);
        println!("    [{}] changed={} instrs_after={} passed={}",
            pass_id, changed, after, validation.passed);
    }
    println!();
}

fn main() {
    check_module(module_builders::build_simple_sum(20));
    check_module(module_builders::build_fib_loop(15));
    check_module(module_builders::build_nested_loop(8));
    check_module(module_builders::build_branch_heavy(16));
    check_module(module_builders::build_entropy_loop(24));
    check_module(module_builders::build_array_reduce(3));
    check_module(module_builders::build_recursive_calls(10));
}
