use crate::module_builders;
use crate::passes::loop_unroll::LoopUnrollPass;
use crate::interpreter::Interpreter;
use crate::profiler::RuntimeProfiler;
use crate::passes::Pass; // Required for .run()

#[test]
fn test_loop_unroll_factor_3() {
    let mut module = module_builders::build_sum_example(6); // Sum 0 to 5 = 15

    let mut lu_pass = LoopUnrollPass::new();
    lu_pass.set_param("factor", 3);
    lu_pass.run(&mut module).expect("LoopUnrollPass failed");

    let interpreter = Interpreter::new();
    let mut profiler = RuntimeProfiler::new();
    let result = interpreter.execute_function(&module.functions[0], &mut profiler).expect("Interpreter failed");

    assert_eq!(result, 15, "Expected 15 after loop unrolling");
}
