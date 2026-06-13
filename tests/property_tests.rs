use proptest::prelude::*;
use crate::ir::module::{Module, Function, BasicBlock};
use crate::ir::value::{Instruction, BinaryOp, CompareCondition, ValueType};
use crate::passes::constant_folding::ConstantFoldingPass;
use crate::passes::dead_code_elimination::DeadCodeEliminationPass;
use crate::passes::{Pass, PassError}; // Ensure PassError is available if needed

// Helper strategy for generating Instruction
fn arb_instruction(depth: u32) -> BoxedStrategy<Instruction> {
    let leaf = prop_oneof![
        any::<i64>().prop_map(|v| Instruction::Constant { value: v }),
        any::<String>().prop_map(|s| Instruction::Variable { name: s }),
    ];
    if depth == 0 { return leaf.boxed(); }

    let recursive = prop_oneof![
        (any::<BinaryOp>(), arb_instruction(depth - 1), arb_instruction(depth - 1))
            .prop_map(|(op, lhs, rhs)| Instruction::BinaryOp { op, lhs, rhs }),
        (any::<CompareCondition>(), arb_instruction(depth - 1), arb_instruction(depth - 1))
            .prop_map(|(condition, lhs, rhs)| Instruction::Compare { condition, lhs, rhs }),
        (any::<String>(), arb_instruction(depth - 1))
            .prop_map(|(var_name, value)| Instruction::Store { var_name, value }),
        (arb_instruction(depth - 1), any::<String>(), any::<String>())
            .prop_map(|(condition, then_label, else_label)| Instruction::Branch { condition, then_label, else_label }),
        any::<String>().prop_map(|s| Instruction::Jump { label: s }),
        arb_instruction(depth - 1).prop_map(|value| Instruction::Return { value: Some(value) }),
        Just(Instruction::Return { value: None }),
    ];
    recursive.boxed()
}

// Helper strategy for generating BasicBlock
fn arb_basic_block(depth: u32) -> BoxedStrategy<BasicBlock> {
    (any::<String>(), prop::collection::vec(arb_instruction(depth), 1..10))
        .prop_map(|(name, instructions)| BasicBlock { name, instructions })
        .boxed()
}

// Helper strategy for generating Function
fn arb_function(depth: u32) -> BoxedStrategy<Function> {
    (any::<String>(), any::<ValueType>(), prop::collection::vec(arb_basic_block(depth), 1..5))
        .prop_map(|(name, return_type, basic_blocks)| Function { name, return_type, basic_blocks })
        .boxed()
}

// Helper strategy for generating Module
fn arb_module(depth: u32) -> BoxedStrategy<Module> {
    (any::<String>(), prop::collection::vec(arb_function(depth), 1..3))
        .prop_map(|(name, functions)| Module { name, functions })
        .boxed()
}

proptest! {
    #[test]
    fn constant_folding_never_panics(module in arb_module(3)) {
        let mut module_cloned = module.clone(); // Work on a clone
        let cf_pass = ConstantFoldingPass::new();
        let _ = cf_pass.run(&mut module_cloned); // Should not panic
    }

    #[test]
    fn dce_never_removes_terminators(module in arb_module(3)) {
        let mut module_cloned = module.clone();
        let dce_pass = DeadCodeEliminationPass::new();

        let initial_terminators: HashMap<String, Instruction> = module_cloned.functions.iter()
            .flat_map(|f| f.basic_blocks.iter())
            .filter_map(|bb| bb.terminator().map(|t| (bb.name.clone(), t.clone())))
            .collect();

        let _ = dce_pass.run(&mut module_cloned); // Should not panic

        // After running DCE, check that original terminators are still present for their blocks
        for func in &module_cloned.functions {
            for bb in &func.basic_blocks {
                if let Some(original_terminator) = initial_terminators.get(&bb.name) {
                    assert!(bb.terminator().is_some(), "Block {} lost its terminator", bb.name);
                    assert_eq!(bb.terminator().unwrap(), original_terminator, "Block {} terminator changed unexpectedly", bb.name);
                }
            }
        }
    }

    // Additional property tests can be added here
    // For example: constant_propagation_never_changes_control_flow
}