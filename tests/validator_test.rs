use crate::validator::{Validator, ValidationResult};
use crate::ir::module::{Module, Function, BasicBlock};
use crate::ir::value::{Instruction, ValueType, BinaryOp};
use std::collections::HashMap;

fn create_simple_sum_module(n: i32) -> Module {
    let mut module = Module::new("sum_module".to_string());
    let mut func = Function::new("sum_func".to_string(), ValueType::Int);

    let mut entry = BasicBlock::new("entry".to_string());
    entry.instructions.extend(vec![
        Instruction::Store { var_name: "i".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "acc".to_string(), value: Box::new(Instruction::Constant { value: 0 }) },
        Instruction::Store { var_name: "n".to_string(), value: Box::new(Instruction::Constant { value: n as i64 }) },
        Instruction::Jump { label: "loop_cond".to_string() },
    ]);
    func.basic_blocks.push(entry);

    let mut loop_cond = BasicBlock::new("loop_cond".to_string());
    loop_cond.instructions.extend(vec![
        Instruction::Branch {
            condition: Box::new(Instruction::Compare {
                condition: crate::ir::value::CompareCondition::Lt,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "n".to_string() }),
            }),
            then_label: "loop_body".to_string(),
            else_label: "exit".to_string(),
        },
    ]);
    func.basic_blocks.push(loop_cond);

    let mut loop_body = BasicBlock::new("loop_body".to_string());
    loop_body.instructions.extend(vec![
        Instruction::Store {
            var_name: "acc".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "acc".to_string() }),
                rhs: Box::new(Instruction::Variable { name: "i".to_string() }),
            }),
        },
        Instruction::Store {
            var_name: "i".to_string(),
            value: Box::new(Instruction::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Instruction::Variable { name: "i".to_string() }),
                rhs: Box::new(Instruction::Constant { value: 1 }),
            }),
        },
        Instruction::Jump { label: "loop_cond".to_string() },
    ]);
    func.basic_blocks.push(loop_body);

    let mut exit_bb = BasicBlock::new("exit".to_string());
    exit_bb.instructions.extend(vec![
        Instruction::Return { value: Some(Box::new(Instruction::Variable { name: "acc".to_string() })) },
    ]);
    func.basic_blocks.push(exit_bb);

    module.functions.push(func);
    module
}

#[test]
fn test_validator_pass_equivalent_modules() {
    let original = create_simple_sum_module(10);
    let optimized = create_simple_sum_module(10); // Identical module
    let validator = Validator::new();

    let result = validator.validate(&original, &optimized, "sum_func", 100);
    assert!(result.passed, "Validation should pass for identical modules");
    assert_eq!(result.failed_tests, 0);
    assert_eq!(result.corruption_probability, 0);
}

#[test]
fn test_validator_fail_different_behavior() {
    let original = create_simple_sum_module(10);
    let mut optimized_wrong = create_simple_sum_module(10);
    // Introduce a bug: change the constant 1 to 2 in the loop increment of the optimized module
    if let Some(func) = optimized_wrong.get_function_mut("sum_func") {
        if let Some(bb) = func.get_block_mut("loop_body") {
            if let Some(Instruction::Store { value, .. }) = bb.instructions.get_mut(1) { // Second instruction in loop_body is the increment
                if let Instruction::BinaryOp { rhs, .. } = &mut **value {
                    *rhs = Box::new(Instruction::Constant { value: 2 });
                }
            }
        }
    }

    let validator = Validator::new();
    let result = validator.validate(&original, &optimized_wrong, "sum_func", 100);
    assert!(!result.passed, "Validation should fail for modules with different behavior");
    assert!(result.failed_tests > 0);
    assert!(result.corruption_probability > 0);
}

#[test]
fn test_validator_function_not_found() {
    let original = create_simple_sum_module(10);
    let optimized = create_simple_sum_module(10);
    let validator = Validator::new();

    let result = validator.validate(&original, &optimized, "non_existent_func", 10);
    assert!(!result.passed, "Validation should fail if function is not found");
    assert_eq!(result.failure_details, "Function not found: non_existent_func");
}
