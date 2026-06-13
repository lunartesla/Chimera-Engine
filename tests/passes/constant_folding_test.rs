use crate::ir::module::{Module, Function, BasicBlock};
use crate::ir::value::{Instruction, BinaryOp, ValueType};
use crate::passes::constant_folding::ConstantFoldingPass;
use crate::interpreter::Interpreter;
use crate::profiler::RuntimeProfiler;
use std::collections::HashMap;

#[test]
fn test_constant_folding_iterations_1() {
    let mut module = Module::new("cf_test".to_string());
    let mut func = Function::new("f".to_string(), ValueType::Int);
    let mut bb = BasicBlock::new("entry".to_string());
    bb.instructions.extend(vec![
        Instruction::Store {
            var_name: "x".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Constant { value: 2 }),
                rhs: Box::new(Instruction::Constant { value: 3 }),
            }),
        },
        Instruction::Return {
            value: Some(Box::new(Instruction::Variable { name: "x".to_string() })),
        },
    ]);
    func.basic_blocks.push(bb);
    module.functions.push(func);

    let mut cf_pass = ConstantFoldingPass::new();
    cf_pass.set_param("iterations", 1);
    cf_pass.run(&mut module).expect("ConstantFoldingPass failed");

    let interpreter = Interpreter::new();
    let mut profiler = RuntimeProfiler::new();
    let result = interpreter.execute_function(&module.functions[0], &mut profiler).expect("Interpreter failed");

    assert_eq!(result, 5, "Expected 5 after constant folding");
}
